use super::store::Store;
use anyhow::Result;
use rusqlite::params;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CategoryStats {
    pub total: i64,
    pub translated: i64,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProjectStats {
    pub total_files: i64,
    pub total_segments: i64,
    pub translated_segments: i64,
    pub reviewed_segments: i64,
    pub tm_entries: i64,
    pub pending_review_segments: i64,
    pub categories: BTreeMap<String, CategoryStats>,
    pub stages: BTreeMap<String, i64>,
}

impl Store {
    /// Read a consistent, project-scoped snapshot from the source-of-truth rows.
    pub fn get_project_stats(&self, project_id: &str) -> Result<ProjectStats> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let total_files = tx.query_row(
            "SELECT COUNT(*) FROM files WHERE active = 1 AND project_id = ?1",
            [project_id],
            |row| row.get(0),
        )?;
        let (total_segments, translated_segments, reviewed_segments) = tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(target_zh IS NOT NULL AND TRIM(target_zh) != '' AND target_zh != source_kr), 0),
            COALESCE(SUM((stage = 9 OR (stage = 4 AND review_confirmed = 1)) AND qa_passed = 1), 0) FROM segments WHERE active = 1 AND project_id = ?1",
            [project_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        let pending_review_segments = tx.query_row(
            &format!(
                "SELECT COUNT(*) FROM segments s JOIN files f ON f.id=s.file_id
            WHERE s.project_id=?1 AND {}",
                super::review_queue::REVIEW_FILTER
            ),
            [project_id],
            |row| row.get(0),
        )?;
        let tm_entries = tx.query_row("SELECT COUNT(*) FROM tm_entries", [], |row| row.get(0))?;
        let categories = {
            let mut stmt = tx.prepare("SELECT f.category, COUNT(s.id),
                COALESCE(SUM(s.target_zh IS NOT NULL AND TRIM(s.target_zh) != '' AND s.target_zh != s.source_kr),0)
                FROM files f LEFT JOIN segments s ON s.file_id = f.id AND s.active = 1
                WHERE f.active = 1 AND f.project_id = ?1 GROUP BY f.category")?;
            let rows = stmt.query_map(params![project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    CategoryStats {
                        total: row.get(1)?,
                        translated: row.get(2)?,
                    },
                ))
            })?;
            rows.collect::<rusqlite::Result<BTreeMap<_, _>>>()?
        };
        let stages = {
            let mut stmt = tx.prepare(
                "SELECT stage, COUNT(*) FROM segments WHERE active = 1 AND project_id = ?1 GROUP BY stage",
            )?;
            let rows = stmt.query_map(params![project_id], |row| {
                Ok((row.get::<_, i32>(0)?.to_string(), row.get::<_, i64>(1)?))
            })?;
            rows.collect::<rusqlite::Result<BTreeMap<_, _>>>()?
        };
        tx.commit()?;
        Ok(ProjectStats {
            total_files,
            total_segments,
            translated_segments,
            reviewed_segments,
            tm_entries,
            pending_review_segments,
            categories,
            stages,
        })
    }
}
