use anyhow::Result;
use rusqlite::Connection;

/// 执行数据库表结构迁移与 FTS5 全文索引构建
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS translation_scopes (
        project_id TEXT PRIMARY KEY REFERENCES projects(id), name TEXT NOT NULL,
        manifest_sha256 TEXT NOT NULL, source_import_id INTEGER NOT NULL, created_at TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS translation_scope_files (
        project_id TEXT NOT NULL REFERENCES translation_scopes(project_id), file_id INTEGER NOT NULL REFERENCES files(id),
        sha256 TEXT NOT NULL, PRIMARY KEY(project_id,file_id));
        CREATE TABLE IF NOT EXISTS translation_scope_segments (
        project_id TEXT NOT NULL REFERENCES translation_scopes(project_id), segment_id INTEGER NOT NULL REFERENCES segments(id),
        source_identity TEXT NOT NULL, source_kr TEXT NOT NULL, key_path TEXT NOT NULL, PRIMARY KEY(project_id,segment_id));
        CREATE TABLE IF NOT EXISTS translation_scope_events (
        id INTEGER PRIMARY KEY, project_id TEXT NOT NULL, previous_sha256 TEXT, manifest_sha256 TEXT NOT NULL,
        source_import_id INTEGER NOT NULL, selected_segments INTEGER NOT NULL, created_at TEXT NOT NULL);")?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;

        -- 1. 项目与版本表
        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            live_client_kr_dir TEXT,
            upstream_llc_dir TEXT,
            workspace_zh_dir TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- 2. 文件资产表
        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            category TEXT NOT NULL,
            total_segments INTEGER NOT NULL DEFAULT 0,
            translated_segments INTEGER NOT NULL DEFAULT 0,
            reviewed_segments INTEGER NOT NULL DEFAULT 0,
            is_locked INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(project_id, relative_path),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        -- 3. 词条表 (Segments)
        CREATE TABLE IF NOT EXISTS segments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id TEXT NOT NULL,
            file_id INTEGER NOT NULL,
            key_path TEXT NOT NULL,
            source_kr TEXT NOT NULL,
            previous_kr TEXT,
            target_zh TEXT,
            speaker TEXT,
            context_info TEXT,
            stage INTEGER NOT NULL DEFAULT 0,
            qa_passed INTEGER NOT NULL DEFAULT 1,
            qa_issues TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(file_id, key_path),
            FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        -- 4. FTS5 全文索引虚拟表 (使用 trigram 分词器，原生毫秒级检索中韩文字符串)
        CREATE VIRTUAL TABLE IF NOT EXISTS segments_fts USING fts5(
            segment_id UNINDEXED,
            file_id UNINDEXED,
            source_kr,
            target_zh,
            speaker,
            key_path,
            tokenize = 'trigram'
        );

        -- 5. FTS5 自动同步触发器
        CREATE TRIGGER IF NOT EXISTS segments_ai AFTER INSERT ON segments BEGIN
            INSERT INTO segments_fts(segment_id, file_id, source_kr, target_zh, speaker, key_path)
            VALUES (new.id, new.file_id, new.source_kr, COALESCE(new.target_zh, ''), COALESCE(new.speaker, ''), new.key_path);
        END;

        CREATE TRIGGER IF NOT EXISTS segments_ad AFTER DELETE ON segments BEGIN
            DELETE FROM segments_fts WHERE segment_id = old.id;
        END;

        CREATE TRIGGER IF NOT EXISTS segments_au AFTER UPDATE ON segments BEGIN
            DELETE FROM segments_fts WHERE segment_id = old.id;
            INSERT INTO segments_fts(segment_id, file_id, source_kr, target_zh, speaker, key_path)
            VALUES (new.id, new.file_id, new.source_kr, COALESCE(new.target_zh, ''), COALESCE(new.speaker, ''), new.key_path);
        END;

        -- 6. TM 翻译记忆库表
        CREATE TABLE IF NOT EXISTS tm_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hash_kr TEXT NOT NULL,
            source_kr TEXT NOT NULL,
            target_zh TEXT NOT NULL,
            file_scope TEXT,
            speaker TEXT,
            source_origin TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 50,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tm_hash ON tm_entries(hash_kr);

        -- 7. 术语与禁忌词表
        CREATE TABLE IF NOT EXISTS terms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_kr TEXT NOT NULL UNIQUE,
            target_zh TEXT NOT NULL,
            category TEXT NOT NULL,
            forbidden_terms TEXT,
            description TEXT,
            created_at TEXT NOT NULL
        );

        -- 8. TM 翻译记忆库 FTS5 全文索引虚拟表
        CREATE VIRTUAL TABLE IF NOT EXISTS tm_fts USING fts5(
            tm_id UNINDEXED,
            source_kr,
            target_zh,
            speaker,
            file_scope,
            tokenize = 'trigram'
        );

        CREATE TRIGGER IF NOT EXISTS tm_ai AFTER INSERT ON tm_entries BEGIN
            INSERT INTO tm_fts(tm_id, source_kr, target_zh, speaker, file_scope)
            VALUES (new.id, new.source_kr, new.target_zh, COALESCE(new.speaker, ''), COALESCE(new.file_scope, ''));
        END;

        CREATE TRIGGER IF NOT EXISTS tm_ad AFTER DELETE ON tm_entries BEGIN
            DELETE FROM tm_fts WHERE tm_id = old.id;
        END;
        "#,
    )?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS translation_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT, project_id TEXT NOT NULL,
        segment_id INTEGER NOT NULL, kind TEXT NOT NULL, producer TEXT NOT NULL,
        source_kr TEXT NOT NULL, previous_target TEXT, target_zh TEXT NOT NULL,
        previous_stage INTEGER NOT NULL, resulting_stage INTEGER NOT NULL,
        previous_version TEXT NOT NULL, created_at TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS idx_translation_events_project ON translation_events(project_id,id);
        CREATE TABLE IF NOT EXISTS source_imports (
        id INTEGER PRIMARY KEY AUTOINCREMENT, project_id TEXT NOT NULL,
        source_directory TEXT NOT NULL, translation_directory TEXT NOT NULL,
        active_segments INTEGER NOT NULL, added_or_reactivated INTEGER NOT NULL,
        changed INTEGER NOT NULL, retired INTEGER NOT NULL, created_at TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS idx_source_imports_project ON source_imports(project_id,id);")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS file_operations (
        id INTEGER PRIMARY KEY AUTOINCREMENT, project_id TEXT, kind TEXT NOT NULL,
        workspace TEXT NOT NULL, backup_directory TEXT NOT NULL, affected_files INTEGER NOT NULL,
        created_at TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS idx_file_operations_project ON file_operations(project_id,id);",
    )?;
    // Additive migration preserves existing databases and historical translations.
    for (table, column, definition) in [
        ("files", "active", "INTEGER NOT NULL DEFAULT 1"),
        ("segments", "active", "INTEGER NOT NULL DEFAULT 1"),
        ("segments", "source_identity", "TEXT"),
        ("segments", "retired_key_path", "TEXT"),
        ("segments", "review_confirmed", "INTEGER NOT NULL DEFAULT 0"),
    ] {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2)",
            [table, column],
            |row| row.get(0),
        )?;
        if !exists {
            conn.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN {column} {definition}"
            ))?;
            if column == "review_confirmed" {
                conn.execute_batch(
                    "UPDATE segments SET review_confirmed = 1 WHERE stage = 4 AND EXISTS (
                    SELECT 1 FROM translation_events e WHERE e.project_id = segments.project_id
                    AND e.segment_id = segments.id AND e.kind = 'review'
                    AND e.source_kr = segments.source_kr AND e.target_zh = segments.target_zh);",
                )?;
            }
        }
    }
    conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_segment_identity ON segments(file_id, source_identity) WHERE source_identity IS NOT NULL;")?;
    super::draft_grants::migrate(conn)?;
    super::search_index::migrate_search_index(conn)?;
    super::categories::migrate_categories(conn)?;
    Ok(())
}
