//! Bounded, hash-verified research snapshots; reports are evidence, never authority or instructions.
use anyhow::{ensure, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    path::{Component, Path},
};

#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    #[error("No research library configured; restart MCP with --research-dir pointing to the bundled research directory")]
    NotConfigured,
    #[error("Research query must contain 1..120 characters, offset must be at most 1000000, and limit must be 1..50")]
    InvalidQuery,
    #[error("Research document {0:?} is not in the configured catalog; use list_research or search_research first")]
    NotFound(String),
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResearchStatus {
    HistoricalUnverified,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResearchMetadata {
    pub id: String,
    pub title: String,
    pub category: String,
    pub status: ResearchStatus,
    pub sha256: String,
    pub original_sha256: String,
    pub modifications: Vec<String>,
}

#[derive(Deserialize)]
struct Catalog {
    schema_version: u32,
    notice: String,
    documents: Vec<ResearchMetadata>,
}

#[derive(Clone, Serialize, JsonSchema)]
pub struct ResearchDocument {
    pub metadata: ResearchMetadata,
    pub content: String,
    pub warning: String,
}

pub struct ResearchLibrary {
    documents: Vec<ResearchDocument>,
    notice: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ResearchPage {
    pub total: usize,
    pub items: Vec<ResearchMetadata>,
    pub warning: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ResearchMatch {
    pub metadata: ResearchMetadata,
    pub excerpt: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ResearchResults {
    pub query: String,
    pub total: usize,
    pub items: Vec<ResearchMatch>,
    pub warning: String,
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .with_context(|| format!("Open research file {}", path.display()))?
        .take(262_145)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 262_144,
        "Research file exceeds 256 KiB: {}",
        path.display()
    );
    Ok(bytes)
}

pub fn load_research(path: &Path) -> Result<ResearchLibrary> {
    let root = path
        .canonicalize()
        .with_context(|| format!("Resolve research directory {}", path.display()))?;
    let catalog_path = root.join("catalog.json").canonicalize()?;
    ensure!(
        catalog_path.starts_with(&root),
        "Research catalog escapes configured directory"
    );
    let catalog: Catalog = serde_json::from_slice(&read_bounded(&catalog_path)?)
        .context("Research catalog requires schema_version, notice and typed document metadata")?;
    ensure!(
        catalog.schema_version == 1,
        "Unsupported research catalog schema {}",
        catalog.schema_version
    );
    ensure!(
        !catalog.notice.trim().is_empty(),
        "Research catalog notice must not be empty"
    );
    ensure!(
        !catalog.documents.is_empty() && catalog.documents.len() <= 256,
        "Research catalog must contain 1..256 documents"
    );
    let mut ids = BTreeSet::new();
    let documents = catalog.documents.into_iter().map(|metadata| {
        let relative = Path::new(&metadata.id);
        ensure!(!metadata.id.contains('\\') && relative.extension().is_some_and(|extension| extension == "md") && relative.components().all(|part| matches!(part, Component::Normal(_))), "Unsafe research document ID {:?}", metadata.id);
        ensure!(ids.insert(metadata.id.clone()), "Duplicate research document ID {:?}", metadata.id);
        ensure!(!metadata.title.trim().is_empty() && !metadata.category.trim().is_empty(), "Research title/category missing for {}", metadata.id);
        for hash in [&metadata.sha256, &metadata.original_sha256] {
            ensure!(hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "Invalid SHA-256 for research document {}", metadata.id);
        }
        let file = root.join(relative).canonicalize().with_context(|| format!("Resolve research document {}", metadata.id))?;
        ensure!(file.starts_with(&root), "Research document {} escapes configured directory", metadata.id);
        let bytes = read_bounded(&file)?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        ensure!(actual == metadata.sha256, "Research SHA-256 mismatch for {}: expected {}, got {}; restore the original bundle", metadata.id, metadata.sha256, actual);
        Ok(ResearchDocument {
            metadata,
            content: String::from_utf8(bytes).context("Research document must be UTF-8")?,
            warning: catalog.notice.clone(),
        })
    }).collect::<Result<Vec<_>>>()?;
    Ok(ResearchLibrary {
        documents,
        notice: catalog.notice,
    })
}

pub fn list_research(
    library: &ResearchLibrary,
    offset: usize,
    limit: usize,
) -> Result<ResearchPage> {
    if offset > 1_000_000 || !(1..=50).contains(&limit) {
        return Err(ResearchError::InvalidQuery.into());
    }
    Ok(ResearchPage {
        total: library.documents.len(),
        items: library
            .documents
            .iter()
            .skip(offset)
            .take(limit)
            .map(|item| item.metadata.clone())
            .collect(),
        warning: library.notice.clone(),
    })
}

pub fn read_research(library: &ResearchLibrary, id: &str) -> Result<ResearchDocument> {
    library
        .documents
        .iter()
        .find(|item| item.metadata.id == id)
        .cloned()
        .ok_or_else(|| ResearchError::NotFound(id.into()).into())
}

pub fn search_research(
    library: &ResearchLibrary,
    query: &str,
    limit: usize,
) -> Result<ResearchResults> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 120 || !(1..=50).contains(&limit) {
        return Err(ResearchError::InvalidQuery.into());
    }
    let matches: Vec<&ResearchDocument> = library
        .documents
        .iter()
        .filter(|document| {
            document.metadata.id.contains(query)
                || document.metadata.title.contains(query)
                || document.content.contains(query)
        })
        .collect();
    let items = matches
        .iter()
        .take(limit)
        .map(|document| {
            let start = document.content.find(query).unwrap_or(0);
            ResearchMatch {
                metadata: document.metadata.clone(),
                excerpt: document.content[start..].chars().take(480).collect(),
            }
        })
        .collect();
    Ok(ResearchResults {
        query: query.into(),
        total: matches.len(),
        items,
        warning: library.notice.clone(),
    })
}
