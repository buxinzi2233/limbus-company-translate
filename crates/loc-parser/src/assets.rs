use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

/// Strip one locale prefix at component boundaries, never inside asset names.
pub fn normalize_asset_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            bail!("Expected a relative asset path: {}", path.display());
        };
        let part = part.to_str().context("Asset path must be valid UTF-8")?;
        let normalized = part.strip_prefix("KR_").unwrap_or(part);
        if normalized.is_empty()
            || matches!(normalized, "." | "..")
            || normalized.contains(['\\', ':'])
        {
            bail!("Invalid normalized asset component: {part:?}");
        }
        parts.push(normalized);
    }
    if parts.is_empty() {
        bail!("Asset path must not be empty");
    }
    Ok(parts.join("/"))
}

/// Inventory all source paths before consumers modify any database or output.
pub fn inventory_korean_assets(root: &Path) -> Result<BTreeMap<String, PathBuf>> {
    if !root.is_dir() {
        bail!(
            "Source directory does not exist or is not a directory: {}",
            root.display()
        );
    }
    let mut assets = BTreeMap::new();
    for entry in WalkDir::new(root).sort_by_file_name() {
        let entry =
            entry.with_context(|| format!("Traverse source directory {}", root.display()))?;
        if entry.file_type().is_symlink() {
            bail!(
                "Source symlinks are not supported: {}",
                entry.path().display()
            );
        }
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|part| part.to_str()) != Some("json")
        {
            continue;
        }
        let normalized = normalize_asset_path(entry.path().strip_prefix(root)?)?;
        if let Some(previous) = assets.insert(normalized.clone(), entry.path().to_path_buf()) {
            bail!(
                "Asset path collision for {normalized:?}: {} and {}",
                previous.display(),
                entry.path().display()
            );
        }
    }
    Ok(assets)
}
