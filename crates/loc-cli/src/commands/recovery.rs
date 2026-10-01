use super::inject::confined_path;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub(super) struct BackupEntry {
    pub file: String,
    pub existed: bool,
    pub original_sha256: Option<String>,
    pub replacement_sha256: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct BackupManifest {
    pub version: u32,
    pub workspace: PathBuf,
    pub entries: Vec<BackupEntry>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct OperationLock {
    pub backup: PathBuf,
}

#[derive(Serialize)]
pub struct RestoreSummary {
    pub restored_files: usize,
    pub removed_files: usize,
    pub interrupted_before_publication: bool,
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn regular_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Expected a regular non-symlink file: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("Inspect {}", path.display())),
    }
    Ok(())
}

/// All cooperating writers hold this OS lock; the file is never unlinked.
pub(super) fn acquire_writer(workspace: &Path) -> Result<File> {
    let path = workspace.join(".lct-writer.lock");
    regular_path(&path)?;
    let guard = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    guard.try_lock().with_context(|| {
        format!(
            "Another injection or restore is running in {}",
            workspace.display()
        )
    })?;
    Ok(guard)
}

pub(super) fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Sync every newly created directory entry up to the existing workspace root.
pub(super) fn sync_ancestry(path: &Path, root: &Path) -> Result<()> {
    ensure!(
        path.starts_with(root),
        "Directory is outside durability boundary: {}",
        path.display()
    );
    for directory in path.ancestors() {
        sync_directory(directory)?;
        if directory == root {
            break;
        }
    }
    Ok(())
}

pub(super) fn publish_operation_lock(workspace: &Path, backup: &Path) -> Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(workspace)?;
    temporary.write_all(&serde_json::to_vec(&OperationLock {
        backup: backup.to_path_buf(),
    })?)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(workspace.join(".lct-inject.lock"))
        .context("Injection lock already exists or cannot be published; restore the recorded operation before writing")?;
    sync_directory(workspace)
}

pub(super) fn publish_manifest(backup: &Path, manifest: &BackupManifest) -> Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(backup)?;
    temporary.write_all(&serde_json::to_vec_pretty(manifest)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(backup.join("manifest.json"))?;
    sync_ancestry(backup, &manifest.workspace)
}

/// Restore a complete backup only when current bytes belong to that operation.
/// A failed or interrupted restore can be rerun; unrelated edits are never overwritten.
pub fn run_restore(workspace_root: &Path, backup_root: &Path) -> Result<RestoreSummary> {
    let workspace = workspace_root
        .canonicalize()
        .context("Restore workspace must exist")?;
    let _writer = acquire_writer(&workspace)?;
    let name = backup_root
        .file_name()
        .and_then(|name| name.to_str())
        .context("Backup path requires an operation UUID")?;
    let operation =
        uuid::Uuid::parse_str(name).context("Backup directory name must be an operation UUID")?;
    ensure!(
        operation.to_string() == name,
        "Backup UUID must use canonical form"
    );
    let backup = workspace.join(".lct-backups").join(name);
    ensure!(
        backup_root.is_absolute() && backup_root == backup,
        "Restore requires the exact backup path inside {}",
        workspace.display()
    );
    for directory in [
        workspace.join(".lct-backups"),
        backup.clone(),
        backup.join("originals"),
    ] {
        match fs::symlink_metadata(&directory) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Unsafe backup directory: {}",
                directory.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let sentinel = workspace.join(".lct-inject.lock");
    regular_path(&sentinel)?;
    let interrupted = if sentinel.try_exists()? {
        let lock: OperationLock = serde_json::from_slice(&fs::read(&sentinel)?)
            .context("Invalid operation lock; preserve it and inspect before recovery")?;
        ensure!(
            lock.backup == backup,
            "A different incomplete operation owns the workspace: {}",
            lock.backup.display()
        );
        true
    } else {
        false
    };
    let manifest_path = backup.join("manifest.json");
    regular_path(&manifest_path)?;
    if !manifest_path.try_exists()? {
        ensure!(
            interrupted,
            "Backup has no published manifest and no matching interrupted operation"
        );
        // No target replacement is permitted before the durable manifest publication.
        fs::remove_file(&sentinel)?;
        sync_directory(&workspace)?;
        return Ok(RestoreSummary {
            restored_files: 0,
            removed_files: 0,
            interrupted_before_publication: true,
        });
    }
    let manifest: BackupManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    ensure!(
        manifest.version == 1 && manifest.workspace == workspace,
        "Unsupported manifest version or workspace mismatch"
    );
    let mut seen = BTreeSet::new();
    let mut staged = Vec::new();
    let mut removed = Vec::new();
    for entry in &manifest.entries {
        ensure!(
            seen.insert(&entry.file),
            "Duplicate backup entry: {}",
            entry.file
        );
        let target = confined_path(&workspace, &entry.file)?;
        regular_path(&target)?;
        let current = if target.try_exists()? {
            Some(fs::read(&target)?)
        } else {
            None
        };
        let original = if entry.existed {
            let saved = confined_path(&backup.join("originals"), &entry.file)?;
            regular_path(&saved)?;
            let bytes = fs::read(&saved)
                .with_context(|| format!("Read backup original {}", saved.display()))?;
            ensure!(
                entry.original_sha256.as_deref() == Some(digest(&bytes).as_str()),
                "Backup checksum mismatch: {}",
                entry.file
            );
            Some(bytes)
        } else {
            ensure!(
                entry.original_sha256.is_none(),
                "Unexpected original checksum: {}",
                entry.file
            );
            None
        };
        if current == original {
            continue;
        }
        ensure!(
            current
                .as_ref()
                .is_some_and(|bytes| digest(bytes) == entry.replacement_sha256),
            "Target has unrelated edits or is missing: {}; preserve those edits before restoring",
            target.display()
        );
        if let Some(bytes) = original {
            let mut temporary =
                tempfile::NamedTempFile::new_in(target.parent().context("Target lacks parent")?)?;
            temporary.write_all(&bytes)?;
            temporary.as_file().set_permissions(
                fs::metadata(backup.join("originals").join(&entry.file))?.permissions(),
            )?;
            temporary.as_file().sync_all()?;
            staged.push((temporary, target));
        } else {
            removed.push(target);
        }
    }
    if !interrupted {
        publish_operation_lock(&workspace, &backup)?;
    }
    let summary = RestoreSummary {
        restored_files: staged.len(),
        removed_files: removed.len(),
        interrupted_before_publication: false,
    };
    for (temporary, target) in staged {
        temporary.persist(&target)?;
        sync_directory(target.parent().context("Target lacks parent")?)?;
    }
    for target in removed {
        fs::remove_file(&target)?;
        sync_directory(target.parent().context("Target lacks parent")?)?;
    }
    fs::remove_file(&sentinel)?;
    sync_directory(&workspace)?;
    Ok(summary)
}
