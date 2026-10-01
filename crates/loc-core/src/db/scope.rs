//! Persisted, source-version-bound task membership; never changes translations.
use super::{CategoryStats, Store};
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("Task scope for project {0} is stale; validate and import its work order against the current full source snapshot")]
    Stale(String),
    #[error("Task scope is not configured for project {0}; import a verified work order first")]
    Missing(String),
}

#[derive(Debug, Deserialize)]
pub struct ScopeFile {
    pub path: String,
    pub sha256: String,
    pub selected_segments: usize,
}
#[derive(Debug, Deserialize)]
pub struct ScopeSegment {
    pub segment_id: i64,
    pub path: String,
    pub pointer: String,
    pub source_identity: String,
    pub source_kr: String,
}
#[derive(Debug, Deserialize)]
pub struct ScopeManifest {
    pub project_id: String,
    pub files: Vec<ScopeFile>,
    pub segments: Vec<ScopeSegment>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct TaskScope {
    pub name: String,
    pub manifest_sha256: String,
    pub source_import_id: i64,
    pub total_files: i64,
    pub total_segments: i64,
    pub translated_segments: i64,
    pub reviewed_segments: i64,
    pub pending_review_segments: i64,
    pub categories: BTreeMap<String, CategoryStats>,
}
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSelection {
    Inventory,
    Task,
}

pub(super) fn validate_scope(conn: &Connection, project: &str) -> Result<()> {
    let pinned: Option<i64> = conn
        .query_row(
            "SELECT source_import_id FROM translation_scopes WHERE project_id=?1",
            [project],
            |r| r.get(0),
        )
        .optional()?;
    let pinned = pinned.ok_or_else(|| ScopeError::Missing(project.to_owned()))?;
    let latest: i64 = conn.query_row(
        "SELECT COALESCE(MAX(id),0) FROM source_imports WHERE project_id=?1",
        [project],
        |r| r.get(0),
    )?;
    let invalid: i64 = conn.query_row("SELECT COUNT(*) FROM translation_scope_segments t LEFT JOIN segments s ON s.id=t.segment_id LEFT JOIN files f ON f.id=s.file_id WHERE t.project_id=?1 AND (s.id IS NULL OR s.project_id<>t.project_id OR s.active<>1 OR f.active<>1 OR s.source_identity IS NULL OR s.source_identity<>t.source_identity OR s.source_kr<>t.source_kr OR s.key_path<>t.key_path)", [project], |r| r.get(0))?;
    let retired: i64 = conn.query_row("SELECT COUNT(*) FROM translation_scope_files t LEFT JOIN files f ON f.id=t.file_id WHERE t.project_id=?1 AND (f.id IS NULL OR f.project_id<>t.project_id OR f.active<>1)", [project], |r| r.get(0))?;
    if pinned != latest || invalid != 0 || retired != 0 {
        return Err(ScopeError::Stale(project.to_owned()).into());
    }
    Ok(())
}

impl Store {
    pub fn latest_source_import_id(&self, project: &str) -> Result<i64> {
        Ok(self.connection()?.query_row(
            "SELECT COALESCE(MAX(id),0) FROM source_imports WHERE project_id=?1",
            [project],
            |r| r.get(0),
        )?)
    }

    /// All membership checks and replacement happen atomically, without segment updates.
    pub fn import_task_scope(
        &self,
        manifest: &ScopeManifest,
        name: &str,
        digest: &str,
        source_import_id: i64,
    ) -> Result<()> {
        ensure!(
            !name.trim().is_empty() && name.len() <= 160,
            "Scope name must contain 1..160 bytes"
        );
        ensure!(
            !manifest.files.is_empty() && !manifest.segments.is_empty(),
            "Task scope must contain files and segments"
        );
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let latest: i64 = tx.query_row(
            "SELECT COALESCE(MAX(id),0) FROM source_imports WHERE project_id=?1",
            [&manifest.project_id],
            |r| r.get(0),
        )?;
        ensure!(
            latest == source_import_id && latest > 0,
            "Source import changed or is missing for project {}; revalidate scope",
            manifest.project_id
        );
        let mut file_ids = BTreeMap::new();
        for file in &manifest.files {
            ensure!(
                file.sha256.len() == 64 && file.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid SHA-256 for {}",
                file.path
            );
            let id: i64 = tx.query_row(
                "SELECT id FROM files WHERE project_id=?1 AND relative_path=?2 AND active=1",
                params![manifest.project_id, file.path],
                |r| r.get(0),
            )?;
            ensure!(
                file_ids.insert(file.path.as_str(), id).is_none(),
                "Duplicate scope file {}",
                file.path
            );
        }
        let mut seen = BTreeSet::new();
        let mut counts = BTreeMap::new();
        for item in &manifest.segments {
            ensure!(
                seen.insert(item.segment_id),
                "Duplicate scope segment {}",
                item.segment_id
            );
            let file_id = file_ids.get(item.path.as_str()).ok_or_else(|| {
                anyhow::anyhow!(
                    "Scope segment {} references undeclared file {}",
                    item.segment_id,
                    item.path
                )
            })?;
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM segments WHERE id=?1 AND project_id=?2 AND file_id=?3 AND key_path=?4 AND source_identity=?5 AND source_kr=?6 AND active=1)", params![item.segment_id, manifest.project_id, file_id, item.pointer, item.source_identity, item.source_kr], |r| r.get(0))?;
            ensure!(valid, "Scope identity/source mismatch for segment {} at {}:{}; re-read the current source", item.segment_id, item.path, item.pointer);
            *counts.entry(item.path.as_str()).or_insert(0_usize) += 1;
        }
        for file in &manifest.files {
            ensure!(
                counts.get(file.path.as_str()).copied().unwrap_or(0) == file.selected_segments,
                "Scope declared count mismatch for {}",
                file.path
            );
        }
        let previous: Option<String> = tx
            .query_row(
                "SELECT manifest_sha256 FROM translation_scopes WHERE project_id=?1",
                [&manifest.project_id],
                |r| r.get(0),
            )
            .optional()?;
        tx.execute(
            "DELETE FROM translation_scope_segments WHERE project_id=?1",
            [&manifest.project_id],
        )?;
        tx.execute(
            "DELETE FROM translation_scope_files WHERE project_id=?1",
            [&manifest.project_id],
        )?;
        tx.execute("INSERT INTO translation_scopes(project_id,name,manifest_sha256,source_import_id,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(project_id) DO UPDATE SET name=excluded.name,manifest_sha256=excluded.manifest_sha256,source_import_id=excluded.source_import_id,created_at=excluded.created_at", params![manifest.project_id, name, digest, source_import_id, chrono::Utc::now().to_rfc3339()])?;
        for file in &manifest.files {
            tx.execute(
                "INSERT INTO translation_scope_files(project_id,file_id,sha256) VALUES(?1,?2,?3)",
                params![
                    manifest.project_id,
                    file_ids[file.path.as_str()],
                    file.sha256
                ],
            )?;
        }
        {
            let mut stmt = tx.prepare("INSERT INTO translation_scope_segments(project_id,segment_id,source_identity,source_kr,key_path) VALUES(?1,?2,?3,?4,?5)")?;
            for item in &manifest.segments {
                stmt.execute(params![
                    manifest.project_id,
                    item.segment_id,
                    item.source_identity,
                    item.source_kr,
                    item.pointer
                ])?;
            }
        }
        tx.execute("INSERT INTO translation_scope_events(project_id,previous_sha256,manifest_sha256,source_import_id,selected_segments,created_at) VALUES(?1,?2,?3,?4,?5,?6)", params![manifest.project_id, previous, digest, source_import_id, manifest.segments.len(), chrono::Utc::now().to_rfc3339()])?;
        tx.commit()?;
        Ok(())
    }

    pub fn task_scope(&self, project: &str) -> Result<Option<TaskScope>> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let meta = tx.query_row("SELECT name,manifest_sha256,source_import_id FROM translation_scopes WHERE project_id=?1", [project], |r| Ok((r.get::<_, String>(0)?,r.get::<_, String>(1)?,r.get::<_, i64>(2)?))).optional()?;
        let Some((name, manifest_sha256, source_import_id)) = meta else {
            return Ok(None);
        };
        validate_scope(&tx, project)?;
        let total_files = tx.query_row(
            "SELECT COUNT(*) FROM translation_scope_files WHERE project_id=?1",
            [project],
            |r| r.get(0),
        )?;
        let (total_segments,translated_segments,reviewed_segments,pending_review_segments) = tx.query_row(&format!("SELECT COUNT(*),COALESCE(SUM(TRIM(s.target_zh)<>'' AND s.target_zh<>s.source_kr),0),COALESCE(SUM((s.stage=9 OR (s.stage=4 AND s.review_confirmed=1)) AND s.qa_passed=1),0),COALESCE(SUM({}),0) FROM translation_scope_segments t JOIN segments s ON s.id=t.segment_id JOIN files f ON f.id=s.file_id WHERE t.project_id=?1", super::review_queue::REVIEW_FILTER), [project], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let categories = {
            let mut stmt = tx.prepare("SELECT f.category,COUNT(*),COALESCE(SUM(TRIM(s.target_zh)<>'' AND s.target_zh<>s.source_kr),0) FROM translation_scope_segments t JOIN segments s ON s.id=t.segment_id JOIN files f ON f.id=s.file_id WHERE t.project_id=?1 GROUP BY f.category")?;
            let rows = stmt.query_map([project], |r| {
                Ok((
                    r.get(0)?,
                    CategoryStats {
                        total: r.get(1)?,
                        translated: r.get(2)?,
                    },
                ))
            })?;
            rows.collect::<rusqlite::Result<BTreeMap<_, _>>>()?
        };
        tx.commit()?;
        Ok(Some(TaskScope {
            name,
            manifest_sha256,
            source_import_id,
            total_files,
            total_segments,
            translated_segments,
            reviewed_segments,
            pending_review_segments,
            categories,
        }))
    }
}
