use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::models::{FileRecord, Project, Segment, Stage, TMEntry};
use super::schema::migrate;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentSearchResult {
    pub segment_id: i64,
    pub file_id: i64,
    pub relative_path: String,
    pub key_path: String,
    pub source_kr: String,
    pub target_zh: Option<String>,
    pub speaker: Option<String>,
    pub stage: Stage,
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn upsert_project(&self, project: &Project) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO projects (id, name, live_client_kr_dir, upstream_llc_dir, workspace_zh_dir, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                live_client_kr_dir = excluded.live_client_kr_dir,
                upstream_llc_dir = excluded.upstream_llc_dir,
                workspace_zh_dir = excluded.workspace_zh_dir,
                updated_at = excluded.updated_at
            "#,
            params![
                project.id,
                project.name,
                project.live_client_kr_dir,
                project.upstream_llc_dir,
                project.workspace_zh_dir,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, live_client_kr_dir, upstream_llc_dir, workspace_zh_dir, created_at, updated_at FROM projects WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let created_str: String = row.get(5)?;
            let updated_str: String = row.get(6)?;
            Ok(Some(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                live_client_kr_dir: row.get(2)?,
                upstream_llc_dir: row.get(3)?,
                workspace_zh_dir: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&created_str)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_str)?.with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_latest_project(&self) -> Result<Option<Project>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, live_client_kr_dir, upstream_llc_dir, workspace_zh_dir, created_at, updated_at FROM projects ORDER BY updated_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let created_str: String = row.get(5)?;
            let updated_str: String = row.get(6)?;
            Ok(Some(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                live_client_kr_dir: row.get(2)?,
                upstream_llc_dir: row.get(3)?,
                workspace_zh_dir: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&created_str)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_str)?.with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_file(&self, file: &FileRecord) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO files (project_id, relative_path, category, total_segments, translated_segments, reviewed_segments, is_locked, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(project_id, relative_path) DO UPDATE SET
                category = excluded.category,
                total_segments = excluded.total_segments,
                translated_segments = excluded.translated_segments,
                reviewed_segments = excluded.reviewed_segments,
                is_locked = excluded.is_locked,
                updated_at = excluded.updated_at
            "#,
            params![
                file.project_id,
                file.relative_path,
                file.category,
                file.total_segments,
                file.translated_segments,
                file.reviewed_segments,
                if file.is_locked { 1 } else { 0 },
                file.created_at.to_rfc3339(),
                file.updated_at.to_rfc3339(),
            ],
        )?;

        let id = conn.query_row(
            "SELECT id FROM files WHERE project_id = ?1 AND relative_path = ?2",
            params![file.project_id, file.relative_path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn get_files(&self, project_id: &str) -> Result<Vec<FileRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, relative_path, category, total_segments, translated_segments, reviewed_segments, is_locked, created_at, updated_at FROM files WHERE project_id = ?1 ORDER BY relative_path ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let created_str: String = row.get(8)?;
            let updated_str: String = row.get(9)?;
            let is_locked_int: i32 = row.get(7)?;
            Ok(FileRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                relative_path: row.get(2)?,
                category: row.get(3)?,
                total_segments: row.get(4)?,
                translated_segments: row.get(5)?,
                reviewed_segments: row.get(6)?,
                is_locked: is_locked_int != 0,
                created_at: DateTime::parse_from_rfc3339(&created_str).unwrap().with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_str).unwrap().with_timezone(&Utc),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn batch_upsert_segments(&self, segments: &[Segment]) -> Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO segments (
                    project_id, file_id, key_path, source_kr, previous_kr,
                    target_zh, speaker, context_info, stage, qa_passed,
                    qa_issues, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(file_id, key_path) DO UPDATE SET
                    source_kr = excluded.source_kr,
                    previous_kr = excluded.previous_kr,
                    target_zh = COALESCE(excluded.target_zh, segments.target_zh),
                    speaker = excluded.speaker,
                    context_info = excluded.context_info,
                    stage = excluded.stage,
                    qa_passed = excluded.qa_passed,
                    qa_issues = excluded.qa_issues,
                    updated_at = excluded.updated_at
                "#,
            )?;

            for seg in segments {
                stmt.execute(params![
                    seg.project_id,
                    seg.file_id,
                    seg.key_path,
                    seg.source_kr,
                    seg.previous_kr,
                    seg.target_zh,
                    seg.speaker,
                    seg.context_info,
                    seg.stage as i32,
                    if seg.qa_passed { 1 } else { 0 },
                    seg.qa_issues,
                    seg.created_at.to_rfc3339(),
                    seg.updated_at.to_rfc3339(),
                ])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// 毫秒级反向全文搜索台词 (短词 < 3字自动走高速 LIKE，长句 >= 3字走 FTS5 trigram 索引)
    pub fn search_segments_fts(&self, query: &str, limit: usize) -> Result<Vec<SegmentSearchResult>> {
        let conn = self.conn.lock().unwrap();
        let char_count = query.chars().count();
        if char_count == 0 {
            return Ok(Vec::new());
        }

        if char_count < 3 {
            let pattern = format!("%{}%", query);
            let mut stmt = conn.prepare(
                r#"
                SELECT s.id, s.file_id, f.relative_path, s.key_path, s.source_kr, s.target_zh, s.speaker, s.stage
                FROM segments s
                JOIN files f ON f.id = s.file_id
                WHERE s.source_kr LIKE ?1 OR s.target_zh LIKE ?1 OR s.speaker LIKE ?1 OR s.key_path LIKE ?1
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![pattern, limit], |row| {
                let stage_int: i32 = row.get(7)?;
                Ok(SegmentSearchResult {
                    segment_id: row.get(0)?,
                    file_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    key_path: row.get(3)?,
                    source_kr: row.get(4)?,
                    target_zh: row.get(5)?,
                    speaker: row.get(6)?,
                    stage: Stage::from(stage_int),
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        } else {
            let escaped_query = format!("\"{}\"", query.replace('"', "\"\""));
            let mut stmt = conn.prepare(
                r#"
                SELECT s.id, s.file_id, f.relative_path, s.key_path, s.source_kr, s.target_zh, s.speaker, s.stage
                FROM segments_fts fts
                JOIN segments s ON s.id = fts.segment_id
                JOIN files f ON f.id = s.file_id
                WHERE segments_fts MATCH ?1
                LIMIT ?2
                "#,
            )?;

            let rows = stmt.query_map(params![escaped_query, limit], |row| {
                let stage_int: i32 = row.get(7)?;
                Ok(SegmentSearchResult {
                    segment_id: row.get(0)?,
                    file_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    key_path: row.get(3)?,
                    source_kr: row.get(4)?,
                    target_zh: row.get(5)?,
                    speaker: row.get(6)?,
                    stage: Stage::from(stage_int),
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        }
    }

    pub fn upsert_tm_entry(&self, entry: &TMEntry) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO tm_entries (hash_kr, source_kr, target_zh, file_scope, speaker, source_origin, priority, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                entry.hash_kr,
                entry.source_kr,
                entry.target_zh,
                entry.file_scope,
                entry.speaker,
                entry.source_origin,
                entry.priority,
                entry.created_at.to_rfc3339(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn query_tm_exact(&self, hash_kr: &str) -> Result<Option<TMEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, hash_kr, source_kr, target_zh, file_scope, speaker, source_origin, priority, created_at
            FROM tm_entries
            WHERE hash_kr = ?1
            ORDER BY priority DESC, id DESC
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![hash_kr])?;
        if let Some(row) = rows.next()? {
            let created_str: String = row.get(8)?;
            Ok(Some(TMEntry {
                id: row.get(0)?,
                hash_kr: row.get(1)?,
                source_kr: row.get(2)?,
                target_zh: row.get(3)?,
                file_scope: row.get(4)?,
                speaker: row.get(5)?,
                source_origin: row.get(6)?,
                priority: row.get(7)?,
                created_at: DateTime::parse_from_rfc3339(&created_str)?.with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }
}
