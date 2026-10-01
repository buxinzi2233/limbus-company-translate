use anyhow::{Context, Result};
use rusqlite::{Connection, Transaction, TransactionBehavior};

/// Upgrade derived file categories without touching sources, drafts, or scope identities.
pub(super) fn migrate_categories(conn: &Connection) -> Result<()> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 2 {
        let files = {
            let mut stmt = tx.prepare("SELECT id, relative_path, category FROM files")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (id, path, previous) in files {
            let category = crate::resource_category(&path);
            if matches!(previous.as_str(), "story" | "battle" | "system") && previous != category {
                tx.execute(
                    "UPDATE files SET category = ?1 WHERE id = ?2",
                    (category, id),
                )
                .with_context(|| format!("Cannot reclassify resource {path:?} (file_id={id})"))?;
            }
        }
        tx.pragma_update(None, "user_version", 2)?;
    }
    tx.commit()?;
    Ok(())
}
