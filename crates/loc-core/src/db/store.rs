use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use super::schema::migrate;
use crate::models::{FileRecord, Project, Segment, Stage};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentSearchResult {
    pub review_confirmed: bool,
    pub segment_id: i64,
    pub file_id: i64,
    pub relative_path: String,
    pub key_path: String,
    pub source_kr: String,
    pub target_zh: Option<String>,
    pub speaker: Option<String>,
    pub stage: Stage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TMSearchResult {
    pub id: i64,
    pub hash_kr: String,
    pub source_kr: String,
    pub target_zh: String,
    pub file_scope: Option<String>,
    pub speaker: Option<String>,
    pub source_origin: String,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentWithFile {
    pub review_confirmed: bool,
    pub is_locked: bool,
    pub id: i64,
    pub project_id: String,
    pub file_id: i64,
    pub relative_path: String,
    pub key_path: String,
    pub source_kr: String,
    pub previous_kr: Option<String>,
    pub target_zh: Option<String>,
    pub speaker: Option<String>,
    pub context_info: Option<String>,
    pub stage: Stage,
    pub qa_passed: bool,
    pub qa_issues: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct Store {
    pub(crate) conn: Arc<Mutex<Connection>>,
    pub(crate) reference: Option<crate::references::ReferenceBinding>,
}

impl Store {
    pub(super) fn connection(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| super::segments::StoreError::Poisoned.into())
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            reference: None,
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            reference: None,
        })
    }

    pub fn database_path(&self) -> Result<Option<std::path::PathBuf>> {
        Ok(self
            .connection()?
            .path()
            .filter(|path| !path.is_empty())
            .map(std::path::PathBuf::from))
    }

    pub fn raw_connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub fn upsert_project(&self, project: &Project) -> Result<()> {
        let conn = self.connection()?;
        super::imports::upsert_project(&conn, project)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        let conn = self.connection()?;
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
        let conn = self.connection()?;
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
        let conn = self.connection()?;
        super::imports::upsert_file(&conn, file)
    }

    pub fn get_files(&self, project_id: &str) -> Result<Vec<FileRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT f.id, f.project_id, f.relative_path, f.category,
            (SELECT COUNT(*) FROM segments s WHERE s.active = 1 AND s.file_id = f.id),
            (SELECT COUNT(*) FROM segments s WHERE s.active = 1 AND s.file_id = f.id AND TRIM(s.target_zh) != '' AND s.target_zh != s.source_kr),
            (SELECT COUNT(*) FROM segments s WHERE s.active = 1 AND s.file_id = f.id AND (s.stage = 9 OR (s.stage = 4 AND s.review_confirmed = 1)) AND s.qa_passed = 1),
            f.is_locked, f.created_at, f.updated_at FROM files f WHERE active = 1 AND project_id = ?1 ORDER BY relative_path ASC",
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
                created_at: super::segments::parse_timestamp(&created_str, 8)?,
                updated_at: super::segments::parse_timestamp(&updated_str, 9)?,
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn batch_upsert_segments(&self, segments: &[Segment]) -> Result<usize> {
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let count = super::imports::upsert_segments(&tx, segments)?;
        tx.commit()?;
        Ok(count)
    }

    /// 毫秒级反向全文搜索台词 (短词 < 3字自动走高速 LIKE，长句 >= 3字走 FTS5 trigram 索引)
    pub fn search_segments_fts(
        &self,
        project_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SegmentSearchResult>> {
        let conn = self.connection()?;
        let char_count = query.chars().count();
        if char_count == 0 {
            return Ok(Vec::new());
        }

        if char_count < 3 {
            let pattern = format!("%{}%", query);
            let mut stmt = conn.prepare(
                r#"
                SELECT s.id, s.file_id, f.relative_path, s.key_path, s.source_kr, s.target_zh, s.speaker, s.stage, s.review_confirmed
                FROM segments s
                JOIN files f ON f.id = s.file_id
                WHERE s.active = 1 AND s.project_id = ?3 AND (s.source_kr LIKE ?1 OR s.target_zh LIKE ?1 OR s.speaker LIKE ?1 OR s.key_path LIKE ?1)
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![pattern, limit, project_id], |row| {
                let stage_int: i32 = row.get(7)?;
                Ok(SegmentSearchResult {
                    review_confirmed: row.get(8)?,
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
                SELECT s.id, s.file_id, f.relative_path, s.key_path, s.source_kr, s.target_zh, s.speaker, s.stage, s.review_confirmed
                FROM segments_fts fts
                JOIN segments s ON s.id = fts.segment_id
                JOIN files f ON f.id = s.file_id
                WHERE segments_fts MATCH ?1 AND s.active = 1 AND s.project_id = ?3
                LIMIT ?2
                "#,
            )?;

            let rows = stmt.query_map(params![escaped_query, limit, project_id], |row| {
                let stage_int: i32 = row.get(7)?;
                Ok(SegmentSearchResult {
                    review_confirmed: row.get(8)?,
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

    /// 毫秒级反向全文搜索 TM 记忆库 (短词 < 3字自动走高速 LIKE，长句 >= 3字走 FTS5 trigram 索引)
    pub fn search_tm_fts(&self, query: &str, limit: usize) -> Result<Vec<TMSearchResult>> {
        let conn = self.connection()?;
        let char_count = query.chars().count();
        if char_count == 0 {
            return Ok(Vec::new());
        }

        if char_count < 3 {
            let pattern = format!("%{}%", query);
            let mut stmt = conn.prepare(
                r#"
                SELECT id, hash_kr, source_kr, target_zh, file_scope, speaker, source_origin, priority
                FROM tm_entries
                WHERE source_kr LIKE ?1 OR target_zh LIKE ?1 OR speaker LIKE ?1 OR file_scope LIKE ?1
                ORDER BY priority DESC, id DESC
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![pattern, limit], |row| {
                Ok(TMSearchResult {
                    id: row.get(0)?,
                    hash_kr: row.get(1)?,
                    source_kr: row.get(2)?,
                    target_zh: row.get(3)?,
                    file_scope: row.get(4)?,
                    speaker: row.get(5)?,
                    source_origin: row.get(6)?,
                    priority: row.get(7)?,
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
                SELECT tm.id, tm.hash_kr, tm.source_kr, tm.target_zh, tm.file_scope, tm.speaker, tm.source_origin, tm.priority
                FROM tm_fts fts
                JOIN tm_entries tm ON tm.id = fts.tm_id
                WHERE tm_fts MATCH ?1
                ORDER BY tm.priority DESC, tm.id DESC
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![escaped_query, limit], |row| {
                Ok(TMSearchResult {
                    id: row.get(0)?,
                    hash_kr: row.get(1)?,
                    source_kr: row.get(2)?,
                    target_zh: row.get(3)?,
                    file_scope: row.get(4)?,
                    speaker: row.get(5)?,
                    source_origin: row.get(6)?,
                    priority: row.get(7)?,
                })
            })?;
            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        }
    }

    /// 分页且按条件检索词条（支持按文件、生命周期阶段、关键词筛选）
    pub fn query_segments_paged(
        &self,
        project_id: &str,
        file_id: Option<i64>,
        stage: Option<i32>,
        search: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<(usize, Vec<SegmentWithFile>)> {
        self.query_segments_by_stages(
            project_id,
            file_id,
            &stage.into_iter().collect::<Vec<_>>(),
            search,
            offset,
            limit,
        )
    }

    pub fn query_segments_by_stages(
        &self,
        project_id: &str,
        file_id: Option<i64>,
        stages: &[i32],
        search: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<(usize, Vec<SegmentWithFile>)> {
        self.query_selected_segments(
            project_id,
            super::scope::SegmentSelection::Inventory,
            (file_id, stages, search),
            offset,
            limit,
        )
    }

    pub fn query_selected_segments(
        &self,
        project_id: &str,
        selection: super::scope::SegmentSelection,
        filters: (Option<i64>, &[i32], Option<&str>),
        offset: usize,
        limit: usize,
    ) -> Result<(usize, Vec<SegmentWithFile>)> {
        self.query_categorized_segments(project_id, selection, filters, None, offset, limit)
    }

    pub fn query_categorized_segments(
        &self,
        project_id: &str,
        selection: super::scope::SegmentSelection,
        filters: (Option<i64>, &[i32], Option<&str>),
        category: Option<crate::ResourceCategory>,
        offset: usize,
        limit: usize,
    ) -> Result<(usize, Vec<SegmentWithFile>)> {
        let (file_id, stages, search) = filters;
        anyhow::ensure!(
            stages.iter().all(|stage| matches!(stage, 0..=5 | 9)),
            "Invalid stage filter"
        );
        let conn = self.connection()?;
        let tx = conn.unchecked_transaction()?;
        if matches!(selection, super::scope::SegmentSelection::Task) {
            super::scope::validate_scope(&tx, project_id)?;
        }
        let mut where_clauses =
            vec!["s.active = 1 AND f.active = 1 AND s.project_id = ?1".to_string()];
        if matches!(selection, super::scope::SegmentSelection::Task) {
            where_clauses.push("EXISTS(SELECT 1 FROM translation_scope_segments t WHERE t.project_id=s.project_id AND t.segment_id=s.id)".to_string());
        }
        let mut params: Vec<rusqlite::types::Value> = vec![project_id.to_string().into()];

        if let Some(category) = category {
            where_clauses.push(format!("f.category = ?{}", params.len() + 1));
            params.push(category.as_str().to_string().into());
        }

        if let Some(fid) = file_id {
            where_clauses.push(format!("s.file_id = ?{}", params.len() + 1));
            params.push(fid.into());
        }

        if !stages.is_empty() {
            let mut placeholders = Vec::new();
            for stage in stages {
                placeholders.push(format!("?{}", params.len() + 1));
                params.push((*stage).into());
            }
            where_clauses.push(format!("s.stage IN ({})", placeholders.join(",")));
        }

        if let Some(q) = search {
            if !q.trim().is_empty() {
                let pat = format!("%{}%", q.trim());
                where_clauses.push(format!(
                    "(s.source_kr LIKE ?{0} OR s.target_zh LIKE ?{0} OR s.speaker LIKE ?{0} OR f.relative_path LIKE ?{0})",
                    params.len() + 1
                ));
                params.push(pat.into());
            }
        }

        let where_sql = if where_clauses.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        // 1. 获取满足条件的总条数
        let count_sql = format!(
            "SELECT COUNT(*) FROM segments s JOIN files f ON f.id = s.file_id {}",
            where_sql
        );
        let mut count_stmt = conn.prepare(&count_sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        let total: usize = count_stmt.query_row(&param_refs[..], |r| r.get(0))?;

        // 2. 分页查数据
        let select_sql = format!(
            r#"
            SELECT s.id, s.project_id, s.file_id, f.relative_path, s.key_path,
                   s.source_kr, s.previous_kr, s.target_zh, s.speaker, s.context_info,
                   s.stage, s.qa_passed, s.qa_issues, s.updated_at, f.is_locked, s.review_confirmed
            FROM segments s
            JOIN files f ON f.id = s.file_id
            {}
            ORDER BY f.relative_path ASC, s.id ASC
            LIMIT ?{} OFFSET ?{}
            "#,
            where_sql,
            params.len() + 1,
            params.len() + 2
        );

        params.push((limit as i64).into());
        params.push((offset as i64).into());
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();

        let mut stmt = conn.prepare(&select_sql)?;
        let rows = stmt.query_map(&param_refs[..], super::segments::segment_from_row)?;

        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }

        Ok((total, items))
    }
}
