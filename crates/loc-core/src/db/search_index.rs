use anyhow::Result;
use rusqlite::{Connection, Transaction, TransactionBehavior};

/// Rebuild the derived index once; segment IDs are also FTS rowids thereafter.
/// Identity-only updates do not touch the index, and indexed text updates delete
/// by rowid rather than scanning the unindexed segment_id column.
pub(super) fn migrate_search_index(conn: &Connection) -> Result<()> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 0 {
        tx.execute_batch(
            "DROP TRIGGER IF EXISTS segments_ai;
            DROP TRIGGER IF EXISTS segments_ad;
            DROP TRIGGER IF EXISTS segments_au;
            DELETE FROM segments_fts;
            INSERT INTO segments_fts(rowid, segment_id, file_id, source_kr, target_zh, speaker, key_path)
                SELECT id, id, file_id, source_kr, COALESCE(target_zh, ''), COALESCE(speaker, ''), key_path
                FROM segments;
            CREATE TRIGGER segments_ai AFTER INSERT ON segments BEGIN
                INSERT INTO segments_fts(rowid, segment_id, file_id, source_kr, target_zh, speaker, key_path)
                VALUES (new.id, new.id, new.file_id, new.source_kr, COALESCE(new.target_zh, ''), COALESCE(new.speaker, ''), new.key_path);
            END;
            CREATE TRIGGER segments_ad AFTER DELETE ON segments BEGIN
                DELETE FROM segments_fts WHERE rowid = old.id;
            END;
            CREATE TRIGGER segments_au AFTER UPDATE OF source_kr, target_zh, speaker, key_path, file_id ON segments BEGIN
                DELETE FROM segments_fts WHERE rowid = old.id;
                INSERT INTO segments_fts(rowid, segment_id, file_id, source_kr, target_zh, speaker, key_path)
                VALUES (new.id, new.id, new.file_id, new.source_kr, COALESCE(new.target_zh, ''), COALESCE(new.speaker, ''), new.key_path);
            END;
            PRAGMA user_version = 1;",
        )?;
    }
    tx.commit()?;
    Ok(())
}
