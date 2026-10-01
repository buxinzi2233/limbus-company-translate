use super::{segments::parse_timestamp, store::Store};
use anyhow::{ensure, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TranslationEvent {
    pub id: i64,
    pub segment_id: i64,
    pub kind: String,
    pub producer: String,
    pub source_kr: String,
    pub previous_target: Option<String>,
    pub target_zh: String,
    pub previous_stage: i32,
    pub resulting_stage: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SourceImport {
    pub id: i64,
    pub source_directory: String,
    pub translation_directory: String,
    pub active_segments: i64,
    pub added_or_reactivated: i64,
    pub changed: i64,
    pub retired: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CorpusSource {
    pub source_origin: String,
    pub entries: i64,
}

pub(super) fn record_translation(
    conn: &Connection,
    before: &super::store::SegmentWithFile,
    after: &super::store::SegmentWithFile,
    kind: &str,
    producer: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO translation_events
        (project_id, segment_id, kind, producer, source_kr, previous_target, target_zh,
        previous_stage, resulting_stage, previous_version, created_at)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            after.project_id,
            after.id,
            kind,
            producer,
            after.source_kr,
            before.target_zh,
            after.target_zh,
            before.stage as i32,
            after.stage as i32,
            before.updated_at.to_rfc3339(),
            after.updated_at.to_rfc3339()
        ],
    )?;
    Ok(())
}

impl Store {
    pub fn translation_events(
        &self,
        project_id: &str,
        before_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<TranslationEvent>> {
        self.translation_events_by_category(project_id, before_id, limit, None)
    }

    pub fn translation_events_by_category(
        &self,
        project_id: &str,
        before_id: Option<i64>,
        limit: usize,
        category: Option<crate::ResourceCategory>,
    ) -> Result<Vec<TranslationEvent>> {
        ensure!(
            (1..=100).contains(&limit),
            "Audit limit must be between 1 and 100"
        );
        ensure!(
            before_id.is_none_or(|id| id > 0),
            "Audit cursor must be positive"
        );
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT e.id, e.segment_id, e.kind, e.producer, e.source_kr, e.previous_target,
            e.target_zh, e.previous_stage, e.resulting_stage, e.created_at FROM translation_events e
            LEFT JOIN segments s ON s.id=e.segment_id AND s.project_id=e.project_id
            LEFT JOIN files f ON f.id=s.file_id AND f.project_id=e.project_id
            WHERE e.project_id = ?1 AND (?2 IS NULL OR e.id < ?2)
            AND (?4 IS NULL OR f.category=?4) ORDER BY e.id DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![
                project_id,
                before_id,
                limit,
                category.map(crate::ResourceCategory::as_str)
            ],
            |row| {
                let timestamp: String = row.get(9)?;
                Ok(TranslationEvent {
                    id: row.get(0)?,
                    segment_id: row.get(1)?,
                    kind: row.get(2)?,
                    producer: row.get(3)?,
                    source_kr: row.get(4)?,
                    previous_target: row.get(5)?,
                    target_zh: row.get(6)?,
                    previous_stage: row.get(7)?,
                    resulting_stage: row.get(8)?,
                    created_at: parse_timestamp(&timestamp, 9)?,
                })
            },
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn source_imports(
        &self,
        project_id: &str,
        before_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<SourceImport>> {
        ensure!(
            (1..=100).contains(&limit),
            "Import history limit must be between 1 and 100"
        );
        ensure!(
            before_id.is_none_or(|id| id > 0),
            "Import history cursor must be positive"
        );
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_directory, translation_directory,
            active_segments, added_or_reactivated, changed, retired, created_at FROM source_imports
            WHERE project_id = ?1 AND (?2 IS NULL OR id < ?2) ORDER BY id DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![project_id, before_id, limit], |row| {
            Ok(SourceImport {
                id: row.get(0)?,
                source_directory: row.get(1)?,
                translation_directory: row.get(2)?,
                active_segments: row.get(3)?,
                added_or_reactivated: row.get(4)?,
                changed: row.get(5)?,
                retired: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn corpus_sources(&self, offset: usize, limit: usize) -> Result<Vec<CorpusSource>> {
        ensure!(
            offset <= 1_000_000 && (1..=100).contains(&limit),
            "Corpus source pagination is outside supported bounds"
        );
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT source_origin, COUNT(*) FROM tm_entries
            GROUP BY source_origin ORDER BY source_origin LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(CorpusSource {
                source_origin: row.get(0)?,
                entries: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
