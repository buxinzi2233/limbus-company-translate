//! Revocable, exclusive, source-pinned capabilities for first-draft writes.
use anyhow::{ensure, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;

use super::{scope::validate_scope, Store};

#[derive(Debug, Error)]
pub enum DraftAccessError {
    #[error("Draft writes require a process-bound permit; this connection is read-only")]
    Missing,
    #[error("Draft permit is invalid, revoked, or bound to another project")]
    Invalid,
    #[error("Draft grant {0} no longer matches the active task scope/source import; revoke and reauthorize")]
    Stale(String),
    #[error("Segment {0} is outside this worker's authorized task membership")]
    Outside(i64),
    #[error("Segment {0} already has an active worker grant; revoke it before handing off")]
    Assigned(i64),
    #[error("Segment {0} is not an empty, unreviewed stage-0 record; existing translations require separate human handling")]
    Existing(i64),
}

/// Bearer capability; deliberately has no Debug implementation or tool output schema.
#[derive(Clone, Deserialize, Serialize)]
pub struct DraftPermit {
    token: String,
}

#[derive(Deserialize)]
pub struct DraftGrantRequest {
    pub project_id: String,
    pub worker: String,
    pub manifest_sha256: String,
    pub source_import_id: i64,
    pub segment_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct DraftGrantReceipt {
    pub id: String,
    pub worker: String,
    pub project_id: String,
    pub manifest_sha256: String,
    pub source_import_id: i64,
    pub segments: usize,
}

pub(super) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS draft_grants (
        id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id),
        worker TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
        manifest_sha256 TEXT NOT NULL, source_import_id INTEGER NOT NULL,
        created_at TEXT NOT NULL, revoked_at TEXT);
        CREATE TABLE IF NOT EXISTS draft_grant_segments (
        grant_id TEXT NOT NULL REFERENCES draft_grants(id),
        segment_id INTEGER NOT NULL REFERENCES segments(id),
        expected_updated_at TEXT NOT NULL, PRIMARY KEY(grant_id,segment_id));
        CREATE INDEX IF NOT EXISTS idx_draft_grant_segment ON draft_grant_segments(segment_id);",
    )?;
    // Old MCP processes still use UPDATE, so fence content-changing draft writes at the database boundary too.
    conn.execute_batch("CREATE TABLE IF NOT EXISTS draft_write_guards (
        segment_id INTEGER PRIMARY KEY, project_id TEXT NOT NULL,
        previous_version TEXT NOT NULL, target_zh TEXT NOT NULL, grant_id TEXT NOT NULL);
        CREATE TRIGGER IF NOT EXISTS require_draft_write_guard
        BEFORE UPDATE OF target_zh,stage ON segments
        WHEN NEW.stage=3 AND (NEW.target_zh IS NOT OLD.target_zh OR OLD.stage<>3)
        BEGIN
            SELECT CASE WHEN NOT EXISTS (
                SELECT 1 FROM draft_write_guards w
                JOIN draft_grants g ON g.id=w.grant_id AND g.project_id=w.project_id
                JOIN draft_grant_segments m ON m.grant_id=g.id AND m.segment_id=w.segment_id
                WHERE w.segment_id=OLD.id AND w.project_id=OLD.project_id
                AND w.previous_version=OLD.updated_at AND w.target_zh=NEW.target_zh
                AND g.revoked_at IS NULL AND OLD.stage=0
                AND COALESCE(OLD.target_zh,'')='' AND OLD.review_confirmed=0
            ) THEN RAISE(ABORT,'Draft write denied: use a current LCT process with an authorized first-draft permit') END;
        END;")?;
    Ok(())
}

fn permit_hash(permit: &DraftPermit) -> Result<String> {
    if permit.token.len() != 64 || !permit.token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DraftAccessError::Invalid.into());
    }
    Ok(format!("{:x}", Sha256::digest(permit.token.as_bytes())))
}

pub(super) fn validate_grant(
    conn: &Connection,
    project: &str,
    permit: &DraftPermit,
) -> Result<String> {
    let grant: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT id,manifest_sha256,source_import_id FROM draft_grants
        WHERE project_id=?1 AND token_hash=?2 AND revoked_at IS NULL",
            params![project, permit_hash(permit)?],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (id, digest, import_id) = grant.ok_or(DraftAccessError::Invalid)?;
    validate_scope(conn, project)?;
    let matches: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM translation_scopes WHERE project_id=?1
        AND manifest_sha256=?2 AND source_import_id=?3)",
        params![project, digest, import_id],
        |r| r.get(0),
    )?;
    ensure!(matches, DraftAccessError::Stale(id.clone()));
    Ok(id)
}

pub(super) fn authorize_segment(
    conn: &Connection,
    project: &str,
    segment_id: i64,
    version: &str,
    permit: Option<&DraftPermit>,
) -> Result<String> {
    let permit = permit.ok_or(DraftAccessError::Missing)?;
    let grant_id = validate_grant(conn, project, permit)?;
    let pinned: Option<String> = conn
        .query_row(
            "SELECT g.expected_updated_at FROM draft_grant_segments g
        JOIN translation_scope_segments t ON t.segment_id=g.segment_id
        WHERE g.grant_id=?1 AND g.segment_id=?2 AND t.project_id=?3",
            params![grant_id, segment_id, project],
            |r| r.get(0),
        )
        .optional()?;
    let pinned = pinned.ok_or(DraftAccessError::Outside(segment_id))?;
    ensure!(
        chrono::DateTime::parse_from_rfc3339(&pinned)?
            == chrono::DateTime::parse_from_rfc3339(version)?,
        super::StoreError::Conflict(segment_id)
    );
    Ok(grant_id)
}

impl Store {
    /// Issued by the trusted coordinator, never exposed as an MCP tool.
    pub fn create_draft_grant(
        &self,
        request: &DraftGrantRequest,
    ) -> Result<(DraftGrantReceipt, DraftPermit)> {
        ensure!(
            !request.worker.trim().is_empty() && request.worker.len() <= 128,
            "Worker label must contain 1..128 bytes"
        );
        let ids: BTreeSet<i64> = request.segment_ids.iter().copied().collect();
        ensure!(
            !ids.is_empty()
                && ids.len() <= 100_000
                && ids.len() == request.segment_ids.len()
                && ids.iter().all(|id| *id > 0),
            "Grant requires 1..100000 unique positive segment IDs"
        );
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        validate_scope(&tx, &request.project_id)?;
        let matches: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM translation_scopes WHERE project_id=?1 AND manifest_sha256=?2 AND source_import_id=?3)",
            params![request.project_id, request.manifest_sha256, request.source_import_id], |r| r.get(0),
        )?;
        ensure!(matches, DraftAccessError::Stale("requested scope".into()));
        let id = Uuid::new_v4().to_string();
        let permit = DraftPermit {
            token: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        };
        tx.execute("INSERT INTO draft_grants(id,project_id,worker,token_hash,manifest_sha256,source_import_id,created_at)
            VALUES(?1,?2,?3,?4,?5,?6,?7)", params![id,request.project_id,request.worker,permit_hash(&permit)?,request.manifest_sha256,request.source_import_id,Utc::now().to_rfc3339()])?;
        for segment_id in &ids {
            let version: Option<String> = tx
                .query_row(
                    "SELECT s.updated_at FROM segments s JOIN files f ON f.id=s.file_id
                JOIN translation_scope_segments t ON t.segment_id=s.id AND t.project_id=s.project_id
                WHERE s.id=?1 AND s.project_id=?2 AND s.active=1 AND f.active=1",
                    params![segment_id, request.project_id],
                    |r| r.get(0),
                )
                .optional()?;
            let version = version.ok_or(DraftAccessError::Outside(*segment_id))?;
            let eligible: bool = tx.query_row(
                "SELECT s.stage=0 AND s.review_confirmed=0 AND COALESCE(s.target_zh,'')='' AND f.is_locked=0
                FROM segments s JOIN files f ON f.id=s.file_id WHERE s.id=?1 AND s.project_id=?2",
                params![segment_id,request.project_id], |r| r.get(0),
            )?;
            ensure!(eligible, DraftAccessError::Existing(*segment_id));
            let assigned: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM draft_grant_segments m JOIN draft_grants g ON g.id=m.grant_id
                WHERE m.segment_id=?1 AND g.project_id=?2 AND g.revoked_at IS NULL)",
                params![segment_id,request.project_id], |r| r.get(0),
            )?;
            ensure!(!assigned, DraftAccessError::Assigned(*segment_id));
            tx.execute("INSERT INTO draft_grant_segments(grant_id,segment_id,expected_updated_at) VALUES(?1,?2,?3)", params![id,segment_id,version])?;
        }
        tx.commit()?;
        Ok((
            DraftGrantReceipt {
                id,
                worker: request.worker.clone(),
                project_id: request.project_id.clone(),
                manifest_sha256: request.manifest_sha256.clone(),
                source_import_id: request.source_import_id,
                segments: ids.len(),
            },
            permit,
        ))
    }

    pub fn check_draft_permit(&self, project: &str, permit: &DraftPermit) -> Result<String> {
        let conn = self.connection()?;
        validate_grant(&conn, project, permit)
    }

    pub fn revoke_draft_grant(&self, project: &str, id: &str) -> Result<()> {
        let conn = self.connection()?;
        let found: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM draft_grants WHERE id=?1 AND project_id=?2)",
            params![id, project],
            |r| r.get(0),
        )?;
        ensure!(found, DraftAccessError::Invalid);
        conn.execute("UPDATE draft_grants SET revoked_at=COALESCE(revoked_at,?1) WHERE id=?2 AND project_id=?3", params![Utc::now().to_rfc3339(),id,project])?;
        Ok(())
    }
}
