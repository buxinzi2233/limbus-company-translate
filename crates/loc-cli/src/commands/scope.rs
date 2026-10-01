//! Import membership only after validating the complete source snapshot and LCT identities.
use anyhow::{ensure, Context, Result};
use loc_core::{ScopeManifest, Store};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

pub fn import_scope(store: &Store, project_id: &str, name: &str, input: &Path) -> Result<()> {
    ensure!(
        fs::metadata(input)?.len() <= 128 * 1024 * 1024,
        "Scope manifest exceeds 128 MiB: {}",
        input.display()
    );
    let bytes =
        fs::read(input).with_context(|| format!("Read scope manifest {}", input.display()))?;
    let manifest: ScopeManifest = serde_json::from_slice(&bytes).context(
        "Invalid scope manifest; require project_id, files, segments and source identity fields",
    )?;
    ensure!(
        manifest.project_id == project_id,
        "Scope project {} does not match explicit project {}",
        manifest.project_id,
        project_id
    );
    let project = store
        .get_project(project_id)?
        .context("Scope project does not exist")?;
    let source = project
        .live_client_kr_dir
        .context("Project has no recorded Korean source snapshot")?;
    let import_id = store.latest_source_import_id(project_id)?;
    let inventory = loc_parser::inventory_korean_assets(Path::new(&source))?;
    let mut documents = BTreeMap::new();
    for file in &manifest.files {
        let path = inventory.get(&file.path).with_context(|| {
            format!(
                "Scope file {} absent from full Korean snapshot {}",
                file.path, source
            )
        })?;
        let raw = fs::read(path)?;
        let actual = format!("{:x}", Sha256::digest(&raw));
        ensure!(
            actual == file.sha256,
            "Source hash mismatch for {}: expected {}, actual {}; rebuild the scope",
            file.path,
            file.sha256,
            actual
        );
        let clean = raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&raw);
        let document: serde_json::Value = serde_json::from_slice(clean)
            .with_context(|| format!("Invalid source JSON {}", path.display()))?;
        documents.insert(file.path.as_str(), document);
    }
    for item in &manifest.segments {
        let value = documents
            .get(item.path.as_str())
            .and_then(|doc| doc.pointer(&item.pointer))
            .and_then(|v| v.as_str());
        ensure!(
            value == Some(item.source_kr.as_str()),
            "Scope source pointer mismatch: segment {} at {}:{}",
            item.segment_id,
            item.path,
            item.pointer
        );
    }
    store.import_task_scope(
        &manifest,
        name,
        &format!("{:x}", Sha256::digest(&bytes)),
        import_id,
    )
}
