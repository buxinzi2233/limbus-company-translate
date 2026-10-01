use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, OptionalExtension, Row};
use thiserror::Error;

use super::store::{SegmentWithFile, Store};
use crate::Stage;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(
        "Database mutex poisoned; restart the service after investigating the preceding failure"
    )]
    Poisoned,
    #[error("Segment {0} does not exist in the selected project")]
    NotFound(i64),
    #[error("Segment {0} or its file is locked; editing is prohibited")]
    Locked(i64),
    #[error("Segment {0} changed since it was loaded; refresh and reconcile your draft")]
    Conflict(i64),
    #[error("Segment {0} has been reviewed; AI draft submission cannot overwrite a reviewed translation")]
    Reviewed(i64),
}

pub struct SaveTranslation<'a> {
    pub project_id: &'a str,
    pub id: i64,
    pub expected_updated_at: DateTime<Utc>,
    pub target_zh: &'a str,
}

pub(super) fn segment_from_row(row: &Row<'_>) -> rusqlite::Result<SegmentWithFile> {
    let stage: i32 = row.get(10)?;
    if !matches!(stage, 0..=5 | 9) {
        return Err(rusqlite::Error::IntegralValueOutOfRange(10, stage.into()));
    }
    let timestamp: String = row.get(13)?;
    let updated_at = parse_timestamp(&timestamp, 13)?;
    Ok(SegmentWithFile {
        id: row.get(0)?,
        project_id: row.get(1)?,
        file_id: row.get(2)?,
        relative_path: row.get(3)?,
        key_path: row.get(4)?,
        source_kr: row.get(5)?,
        previous_kr: row.get(6)?,
        target_zh: row.get(7)?,
        speaker: row.get(8)?,
        context_info: row.get(9)?,
        stage: Stage::from(stage),
        qa_passed: row.get(11)?,
        qa_issues: row.get(12)?,
        updated_at,
        is_locked: row.get(14)?,
        review_confirmed: row.get(15)?,
    })
}

pub(super) fn parse_timestamp(timestamp: &str, column: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(timestamp)
        .map(|date| date.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
        })
}

const SELECT_SEGMENT: &str = "SELECT s.id, s.project_id, s.file_id, f.relative_path,
    s.key_path, s.source_kr, s.previous_kr, s.target_zh, s.speaker, s.context_info,
    s.stage, s.qa_passed, s.qa_issues, s.updated_at, f.is_locked, s.review_confirmed
    FROM segments s JOIN files f ON f.id = s.file_id
    WHERE s.active = 1 AND f.active = 1 AND f.project_id = s.project_id AND s.project_id = ?1 AND s.id = ?2";

impl Store {
    pub fn get_segment(&self, project_id: &str, id: i64) -> Result<SegmentWithFile> {
        let conn = self.connection()?;
        conn.query_row(SELECT_SEGMENT, params![project_id, id], segment_from_row)
            .optional()?
            .ok_or_else(|| StoreError::NotFound(id).into())
    }

    /// Persist only a source-validated translation, using a compare-and-swap version.
    pub fn save_reviewed_translation(
        &self,
        change: &SaveTranslation<'_>,
    ) -> Result<SegmentWithFile> {
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = tx
            .query_row(
                SELECT_SEGMENT,
                params![change.project_id, change.id],
                segment_from_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound(change.id))?;
        if current.is_locked || current.stage == Stage::UpstreamFrozen {
            return Err(StoreError::Locked(change.id).into());
        }
        if current.updated_at != change.expected_updated_at {
            return Err(StoreError::Conflict(change.id).into());
        }
        let now = std::cmp::max(
            Utc::now(),
            current.updated_at + chrono::Duration::nanoseconds(1),
        );
        tx.execute(
            "UPDATE segments SET target_zh = ?1, stage = 4, review_confirmed = 1, qa_passed = 1,
            qa_issues = NULL, updated_at = ?2 WHERE project_id = ?3 AND id = ?4",
            params![
                change.target_zh,
                now.to_rfc3339(),
                change.project_id,
                change.id
            ],
        )?;
        let saved = tx.query_row(
            SELECT_SEGMENT,
            params![change.project_id, change.id],
            segment_from_row,
        )?;
        super::audit::record_translation(
            &tx,
            &current,
            &saved,
            "review",
            "local-review-interface",
        )?;
        tx.commit()?;
        Ok(saved)
    }
    /// Persist a QA-validated AI draft without granting reviewed status.
    pub fn save_ai_draft(
        &self,
        change: &SaveTranslation<'_>,
        producer: &str,
        permit: Option<&super::DraftPermit>,
    ) -> Result<SegmentWithFile> {
        anyhow::ensure!(
            !producer.trim().is_empty() && producer.len() <= 128,
            "Producer must contain 1 to 128 bytes"
        );
        anyhow::ensure!(
            !change.target_zh.trim().is_empty(),
            "Draft translation cannot be empty"
        );
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = tx
            .query_row(
                SELECT_SEGMENT,
                params![change.project_id, change.id],
                segment_from_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound(change.id))?;
        if current.is_locked || current.stage == Stage::UpstreamFrozen {
            return Err(StoreError::Locked(change.id).into());
        }
        if current.updated_at != change.expected_updated_at {
            return Err(StoreError::Conflict(change.id).into());
        }
        if current.stage == Stage::Reviewed {
            return Err(StoreError::Reviewed(change.id).into());
        }
        if current.stage != Stage::Untranslated
            || current.review_confirmed
            || current
                .target_zh
                .as_ref()
                .is_some_and(|text| !text.is_empty())
        {
            return Err(super::DraftAccessError::Existing(change.id).into());
        }
        let grant_id = super::draft_grants::authorize_segment(
            &tx,
            change.project_id,
            change.id,
            &current.updated_at.to_rfc3339(),
            permit,
        )?;
        let now = std::cmp::max(
            Utc::now(),
            current.updated_at + chrono::Duration::nanoseconds(1),
        );
        tx.execute(
            "INSERT INTO draft_write_guards(segment_id,project_id,previous_version,target_zh,grant_id)
            SELECT id,project_id,updated_at,?1,?2 FROM segments WHERE project_id=?3 AND id=?4",
            params![change.target_zh, grant_id, change.project_id, change.id],
        )?;
        tx.execute(
            "UPDATE segments SET target_zh = ?1, stage = 3, review_confirmed = 0, qa_passed = 1,
            qa_issues = NULL, updated_at = ?2 WHERE project_id = ?3 AND id = ?4",
            params![
                change.target_zh,
                now.to_rfc3339(),
                change.project_id,
                change.id
            ],
        )?;
        let saved = tx.query_row(
            SELECT_SEGMENT,
            params![change.project_id, change.id],
            segment_from_row,
        )?;
        tx.execute(
            "DELETE FROM draft_write_guards WHERE segment_id=?1",
            [change.id],
        )?;
        super::audit::record_translation(
            &tx,
            &current,
            &saved,
            "ai_draft",
            &format!("grant:{grant_id}/{producer}"),
        )?;
        tx.commit()?;
        Ok(saved)
    }
}
