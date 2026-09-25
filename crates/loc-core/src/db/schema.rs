use anyhow::Result;
use rusqlite::Connection;

/// 执行数据库表结构迁移与 FTS5 全文索引构建
pub fn migrate(conn: &Connection) -> Result<()> {
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
        "#,
    )?;
    Ok(())
}
