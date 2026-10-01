use super::store::Store;
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct FileOperation {
    pub id: i64,
    pub project_id: Option<String>,
    pub kind: String,
    pub workspace: String,
    pub backup_directory: String,
    pub affected_files: usize,
    pub created_at: String,
}

impl Store {
    /// A completed-file receipt is historical evidence, not current filesystem state.
    pub fn record_injection(
        &self,
        project_id: Option<&str>,
        workspace: &Path,
        backup: &Path,
        affected_files: usize,
    ) -> Result<()> {
        let conn = self.connection()?;
        conn.execute("INSERT INTO file_operations (project_id,kind,workspace,backup_directory,affected_files,created_at)
            VALUES (?1,'inject',?2,?3,?4,?5)", params![project_id, workspace.to_str().context("Workspace is not UTF-8")?,
            backup.to_str().context("Backup path is not UTF-8")?, affected_files, chrono::Utc::now().to_rfc3339()])?;
        Ok(())
    }

    pub fn record_restoration(
        &self,
        workspace: &Path,
        backup: &Path,
        affected_files: usize,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let workspace = workspace.to_str().context("Workspace is not UTF-8")?;
        let backup = backup.to_str().context("Backup path is not UTF-8")?;
        let project_id: Option<String> = tx
            .query_row(
                "SELECT project_id FROM file_operations WHERE kind='inject'
            AND workspace=?1 AND backup_directory=?2 ORDER BY id DESC LIMIT 1",
                params![workspace, backup],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        tx.execute("INSERT INTO file_operations (project_id,kind,workspace,backup_directory,affected_files,created_at)
            VALUES (?1,'restore',?2,?3,?4,?5)", params![project_id,workspace,backup,affected_files,chrono::Utc::now().to_rfc3339()])?;
        tx.commit()?;
        Ok(())
    }

    pub fn file_operations(
        &self,
        project_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<FileOperation>> {
        ensure!(
            (1..=100).contains(&limit) && before.is_none_or(|id| id > 0),
            "Invalid file operation pagination"
        );
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT id,project_id,kind,workspace,backup_directory,affected_files,created_at
            FROM file_operations WHERE (project_id=?1 OR project_id IS NULL) AND (?2 IS NULL OR id<?2) ORDER BY id DESC LIMIT ?3")?;
        let rows = stmt.query_map(params![project_id, before, limit], |row| {
            Ok(FileOperation {
                id: row.get(0)?,
                project_id: row.get(1)?,
                kind: row.get(2)?,
                workspace: row.get(3)?,
                backup_directory: row.get(4)?,
                affected_files: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
