use super::{segments::parse_timestamp, store::Store};
use crate::TMEntry;
use anyhow::Result;
use rusqlite::{params, Connection};

fn insert_tm_entry(conn: &Connection, entry: &TMEntry) -> Result<(i64, bool)> {
    let mut insert = conn.prepare_cached(
        "INSERT INTO tm_entries (hash_kr, source_kr, target_zh, file_scope, speaker, source_origin, priority, created_at)
        SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
        WHERE NOT EXISTS (SELECT 1 FROM tm_entries WHERE hash_kr = ?1 AND source_kr = ?2
            AND target_zh = ?3 AND file_scope IS ?4 AND speaker IS ?5 AND source_origin = ?6 AND priority = ?7)",
    )?;
    let changed = insert.execute(params![
        entry.hash_kr,
        entry.source_kr,
        entry.target_zh,
        entry.file_scope,
        entry.speaker,
        entry.source_origin,
        entry.priority,
        entry.created_at.to_rfc3339()
    ])?;
    let mut select = conn.prepare_cached("SELECT MIN(id) FROM tm_entries WHERE hash_kr = ?1 AND source_kr = ?2
        AND target_zh = ?3 AND file_scope IS ?4 AND speaker IS ?5 AND source_origin = ?6 AND priority = ?7",
    )?;
    let id = select.query_row(
        params![
            entry.hash_kr,
            entry.source_kr,
            entry.target_zh,
            entry.file_scope,
            entry.speaker,
            entry.source_origin,
            entry.priority
        ],
        |row| row.get(0),
    )?;
    Ok((id, changed == 1))
}

impl Store {
    /// Serialize duplicate checks and writes across independent SQLite connections.
    pub fn upsert_tm_entry(&self, entry: &TMEntry) -> Result<i64> {
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (id, _) = insert_tm_entry(&tx, entry)?;
        tx.commit()?;
        Ok(id)
    }

    /// Commit an entire prepared corpus or leave all existing entries unchanged.
    pub fn upsert_tm_entries_batch(&self, entries: &[TMEntry]) -> Result<usize> {
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut inserted = 0;
        for entry in entries {
            let (_, changed) = insert_tm_entry(&tx, entry)?;
            inserted += usize::from(changed);
        }
        tx.commit()?;
        Ok(inserted)
    }

    pub fn get_tm_count(&self) -> Result<i64> {
        Ok(self
            .connection()?
            .query_row("SELECT COUNT(*) FROM tm_entries", [], |row| row.get(0))?)
    }

    /// A hash is only an index hint; callers must verify source text and ambiguity.
    pub fn query_tm_candidates(&self, hash_kr: &str) -> Result<Vec<TMEntry>> {
        let conn = self.connection()?;
        let mut statement = conn.prepare(
            "SELECT id, hash_kr, source_kr, target_zh, file_scope, speaker, source_origin,
            priority, created_at FROM tm_entries WHERE hash_kr = ?1 ORDER BY priority DESC, id ASC",
        )?;
        let rows = statement.query_map([hash_kr], |row| {
            Ok(TMEntry {
                id: row.get(0)?,
                hash_kr: row.get(1)?,
                source_kr: row.get(2)?,
                target_zh: row.get(3)?,
                file_scope: row.get(4)?,
                speaker: row.get(5)?,
                source_origin: row.get(6)?,
                priority: row.get(7)?,
                created_at: parse_timestamp(&row.get::<_, String>(8)?, 8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
