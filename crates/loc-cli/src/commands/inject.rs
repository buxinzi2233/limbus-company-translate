use super::recovery::{
    acquire_writer, digest, publish_manifest, publish_operation_lock, sync_ancestry,
    sync_directory, BackupEntry, BackupManifest,
};
use anyhow::{bail, ensure, Context, Result};
use loc_core::Store;
use loc_parser::{
    extract_segments_from_value, inject_translations_into_value, serialize_to_game_json,
};
use loc_parser::{inventory_korean_assets, parse_game_json};
use loc_qa::check_segment_pair;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct TicketInputItem {
    file: String,
    pointer: String,
    #[serde(alias = "kr")]
    source_kr: String,
    #[serde(alias = "zh")]
    target_zh: Option<String>,
}

#[derive(Debug)]
struct Patch {
    pointer: String,
    target: String,
    source: String,
    source_identity: Option<String>,
}
struct PreparedFile {
    relative_path: String,
    original: Option<Vec<u8>>,
    replacement: Vec<u8>,
    count: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct InjectSummary {
    pub modified_files: usize,
    pub injected_segments: usize,
    pub skipped_untranslated: usize,
    pub qa_violations: usize,
    pub backup_directory: Option<PathBuf>,
    pub elapsed_ms: u128,
}

/// Reject traversal, platform-specific absolute paths and symlink destinations.
pub(super) fn confined_path(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative
        .split('/')
        .any(|part| matches!(part, "" | "." | ".."))
        || relative.contains(['\\', ':'])
        || !relative.ends_with(".json")
    {
        bail!("Invalid relative JSON path: {relative:?}");
    }
    let mut result = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) if !part.to_string_lossy().starts_with(".lct-") => {
                result.push(part)
            }
            _ => bail!("Unsafe relative path: {relative:?}"),
        }
        match fs::symlink_metadata(&result) {
            Ok(metadata) if metadata.file_type().is_symlink() => bail!(
                "Symlink is not allowed in injection path: {}",
                result.display()
            ),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("Inspect path {}", result.display()))
            }
        }
    }
    Ok(result)
}

struct PatchBatch {
    patches: BTreeMap<String, Vec<Patch>>,
    skipped: usize,
    project_id: Option<String>,
}

fn collect_patches(
    store: &Store,
    ticket_path: Option<&Path>,
    selected_project: Option<&str>,
) -> Result<PatchBatch> {
    let mut patches: BTreeMap<String, Vec<Patch>> = BTreeMap::new();
    let mut skipped = 0;
    let mut project_id = None;
    if let Some(path) = ticket_path {
        for (index, line) in BufReader::new(File::open(path)?).lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let item: TicketInputItem = serde_json::from_str(&line)
                .with_context(|| format!("Invalid ticket {} line {}", path.display(), index + 1))?;
            if item.source_kr.trim().is_empty() {
                bail!(
                    "Ticket {} line {} requires a non-empty source_kr",
                    path.display(),
                    index + 1
                );
            }
            match item.target_zh {
                Some(target)
                    if !target.trim().is_empty() && target.trim() != item.source_kr.trim() =>
                {
                    patches.entry(item.file).or_default().push(Patch {
                        pointer: item.pointer,
                        target,
                        source: item.source_kr,
                        source_identity: None,
                    });
                }
                _ => skipped += 1,
            }
        }
    } else {
        let project = match selected_project {
            Some(id) => store
                .get_project(id)?
                .with_context(|| format!("Project {id} does not exist"))?,
            None => store
                .get_latest_project()?
                .context("No active project; run lct init first")?,
        };
        project_id = Some(project.id.clone());
        let connection = store.raw_connection();
        let conn = connection
            .lock()
            .map_err(|_| anyhow::anyhow!("Database mutex poisoned"))?;
        let mut statement = conn.prepare("SELECT f.relative_path, s.key_path, s.source_kr, s.target_zh, s.source_identity
            FROM segments s JOIN files f ON f.id = s.file_id
            WHERE s.active = 1 AND f.active = 1 AND s.project_id = ?1 AND f.project_id = ?1 AND f.is_locked = 0
            AND s.stage = 4 AND s.review_confirmed = 1 AND s.qa_passed = 1 AND TRIM(s.target_zh) != '' AND s.target_zh != s.source_kr")?;
        let rows = statement.query_map([project.id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                Patch {
                    pointer: row.get(1)?,
                    source: row.get(2)?,
                    source_identity: row.get(4)?,
                    target: row.get(3)?,
                },
            ))
        })?;
        for row in rows {
            let (path, patch) = row?;
            patches.entry(path).or_default().push(patch);
        }
    }
    Ok(PatchBatch {
        patches,
        skipped,
        project_id,
    })
}

/// Preflight every file before writes. Each replacement is atomic; the batch is not.
/// Originals and a manifest are retained for explicit recovery after I/O failure.
pub fn run_inject(
    store: &Store,
    workspace_root: &Path,
    kr_template_root: Option<&Path>,
    ticket_path: Option<&Path>,
    qa_check: bool,
    force: bool,
) -> Result<InjectSummary> {
    run_scoped_inject(
        store,
        workspace_root,
        kr_template_root,
        ticket_path,
        qa_check,
        force,
        None,
    )
}

pub fn run_scoped_inject(
    store: &Store,
    workspace_root: &Path,
    kr_template_root: Option<&Path>,
    ticket_path: Option<&Path>,
    qa_check: bool,
    force: bool,
    project_id: Option<&str>,
) -> Result<InjectSummary> {
    ensure!(
        ticket_path.is_none() || project_id.is_none(),
        "External tickets cannot declare database project ownership"
    );
    let start = Instant::now();
    let source_root = kr_template_root.context("Injection requires --kr with the current Korean source directory to verify ticket freshness")?.canonicalize()?;
    let workspace_effective = if workspace_root
        .join("LimbusCompany_Data/Lang/LLC_zh-CN")
        .is_dir()
    {
        workspace_root.join("LimbusCompany_Data/Lang/LLC_zh-CN")
    } else {
        workspace_root.to_path_buf()
    };
    let workspace = workspace_effective
        .canonicalize()
        .context("Workspace must already exist; create a dedicated output directory first")?;
    if workspace == source_root
        || workspace.starts_with(&source_root)
        || source_root.starts_with(&workspace)
    {
        bail!("Korean source and output workspace must be separate, non-nested directories");
    }
    if !qa_check && !force {
        bail!("Disabling QA requires explicit --force authorization");
    }
    let _writer = acquire_writer(&workspace)?;
    let source_assets = inventory_korean_assets(&source_root)?;
    let PatchBatch {
        patches,
        skipped: skipped_untranslated,
        project_id,
    } = collect_patches(store, ticket_path, project_id)?;
    let mut summary = InjectSummary {
        skipped_untranslated,
        ..InjectSummary::default()
    };
    let mut prepared = Vec::new();
    for (relative_path, entries) in patches {
        let target = confined_path(&workspace, &relative_path)?;
        let source = source_assets
            .get(&relative_path)
            .with_context(|| format!("No Korean source asset mapped to {relative_path:?}"))?;
        let source_value: Value = parse_game_json(
            &fs::read(source).with_context(|| format!("Read source {}", source.display()))?,
        )
        .with_context(|| format!("Parse source {}", source.display()))?;
        let allowed_pointers: BTreeSet<String> = extract_segments_from_value(&source_value)?
            .into_iter()
            .map(|item| item.pointer)
            .collect();
        let original = if target.try_exists()? {
            Some(fs::read(&target)?)
        } else {
            None
        };
        let mut value = match &original {
            Some(bytes) => parse_game_json(bytes)
                .with_context(|| format!("Parse target {}", target.display()))?,
            None => source_value.clone(),
        };
        let patch_pointers: Vec<&str> =
            entries.iter().map(|patch| patch.pointer.as_str()).collect();
        let source_identities = loc_parser::source_identities(&source_value, &patch_pointers)?;
        let target_identities = loc_parser::source_identities(&value, &patch_pointers)?;
        let mut pointers = BTreeSet::new();
        for patch in &entries {
            if !allowed_pointers.contains(&patch.pointer) {
                bail!(
                    "Pointer is not a translatable Korean field: {relative_path} {}",
                    patch.pointer
                );
            }
            if patch.pointer.is_empty() || !pointers.insert(&patch.pointer) {
                bail!(
                    "Empty or duplicate pointer in {relative_path}: {:?}",
                    patch.pointer
                );
            }
            let actual_source = source_value
                .pointer(&patch.pointer)
                .and_then(Value::as_str)
                .with_context(|| {
                    format!("Source string not found: {relative_path} {}", patch.pointer)
                })?;
            if actual_source != patch.source {
                bail!(
                    "Stale source in {relative_path} at {}; refresh the ticket before injection",
                    patch.pointer
                );
            }
            if value
                .pointer(&patch.pointer)
                .and_then(Value::as_str)
                .is_none()
            {
                bail!(
                    "Target must be an existing string leaf: {relative_path} {}",
                    patch.pointer
                );
            }
            let identity = &source_identities[&patch.pointer];
            if identity.snapshot_bound {
                ensure!(patch.source_identity.as_deref() == Some(identity.encoded.as_str()),
                    "Snapshot-bound source identity is missing or stale: {relative_path} {}; re-extract and review the current source, then inject from the reviewed database rather than a positional ticket",
                    patch.pointer);
            }
            ensure!(source_identities[&patch.pointer] == target_identities[&patch.pointer],
                "Target array identity differs from current source in {relative_path} {}; align the output structure before injection", patch.pointer);
            if qa_check {
                let issues = check_segment_pair(actual_source, &patch.target);
                summary.qa_violations += issues.len();
                if !issues.is_empty() {
                    if !force {
                        bail!(
                            "QA rejected {relative_path} {}: {}",
                            patch.pointer,
                            serde_json::to_string(&issues)?
                        );
                    }
                    tracing::warn!(file = relative_path, pointer = patch.pointer, issues = ?issues, "Explicitly bypassing QA");
                }
            }
        }
        let translations: Vec<(&str, &str)> = entries
            .iter()
            .map(|p| (p.pointer.as_str(), p.target.as_str()))
            .collect();
        let count = inject_translations_into_value(&mut value, &translations)?;
        prepared.push(PreparedFile {
            relative_path,
            original,
            replacement: serialize_to_game_json(&value)?.into_bytes(),
            count,
        });
    }
    if prepared.is_empty() {
        summary.elapsed_ms = start.elapsed().as_millis();
        return Ok(summary);
    }

    let lock_path = workspace.join(".lct-inject.lock");
    let backup = workspace
        .join(".lct-backups")
        .join(uuid::Uuid::new_v4().to_string());
    publish_operation_lock(&workspace, &backup)?;
    let result = write_prepared(&workspace, &backup, &prepared);
    // Keep the lock on failure so another writer cannot conceal partial output.
    result.with_context(|| {
        format!(
            "Injection stopped; inspect backup {} and lock {} before recovery",
            backup.display(),
            lock_path.display()
        )
    })?;
    fs::remove_file(&lock_path)?;
    sync_directory(&workspace)?;
    summary.modified_files = prepared.len();
    summary.injected_segments = prepared.iter().map(|file| file.count).sum();
    store.record_injection(project_id.as_deref(), &workspace, &backup, prepared.len())
        .with_context(|| format!("Files were injected successfully, but the database audit receipt failed; do not blindly retry. Inspect backup {}", backup.display()))?;
    summary.backup_directory = Some(backup);
    summary.elapsed_ms = start.elapsed().as_millis();
    Ok(summary)
}

fn write_prepared(workspace: &Path, backup: &Path, files: &[PreparedFile]) -> Result<()> {
    let backup_root = workspace.join(".lct-backups");
    if backup_root.try_exists()? && fs::symlink_metadata(&backup_root)?.file_type().is_symlink() {
        bail!(
            "Backup root must not be a symlink: {}",
            backup_root.display()
        );
    }
    // Recheck all snapshots before preparing any output.
    for file in files {
        let target = confined_path(workspace, &file.relative_path)?;
        let current = if target.try_exists()? {
            Some(fs::read(&target)?)
        } else {
            None
        };
        if current != file.original {
            bail!("Target changed during preflight: {}", target.display());
        }
    }
    fs::create_dir_all(backup)?;
    let mut manifest = Vec::new();
    let mut staged = Vec::new();
    for file in files {
        if let Some(original) = &file.original {
            let saved = backup.join("originals").join(&file.relative_path);
            fs::create_dir_all(saved.parent().context("Backup path lacks parent")?)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&saved)?;
            output.write_all(original)?;
            output.set_permissions(
                fs::metadata(confined_path(workspace, &file.relative_path)?)?.permissions(),
            )?;
            output.sync_all()?;
            sync_ancestry(
                saved.parent().context("Backup path lacks parent")?,
                workspace,
            )?;
        }
        manifest.push(BackupEntry {
            file: file.relative_path.clone(),
            existed: file.original.is_some(),
            original_sha256: file.original.as_ref().map(|bytes| digest(bytes)),
            replacement_sha256: digest(&file.replacement),
        });
        let target = confined_path(workspace, &file.relative_path)?;
        let parent = target.parent().context("Target path lacks parent")?;
        fs::create_dir_all(parent)?;
        sync_ancestry(parent, workspace)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&file.replacement)?;
        if target.try_exists()? {
            temporary
                .as_file()
                .set_permissions(fs::metadata(&target)?.permissions())?;
        }
        temporary.as_file().sync_all()?;
        staged.push((temporary, target));
    }
    publish_manifest(
        backup,
        &BackupManifest {
            version: 1,
            workspace: workspace.to_path_buf(),
            entries: manifest,
        },
    )?;
    for (temporary, target) in staged {
        temporary
            .persist(&target)
            .with_context(|| format!("Atomic replacement failed: {}", target.display()))?;
        sync_directory(target.parent().context("Target path lacks parent")?)?;
    }
    Ok(())
}
