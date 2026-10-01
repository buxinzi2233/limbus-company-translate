use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize)]
enum IdentityPart {
    Field(String),
    Entry { key: String, value: Value },
    Position(usize),
    SnapshotEntry { sha256: String, index: usize },
}

/// A snapshot-bound identity must not be used to guess bilingual alignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceIdentity {
    pub encoded: String,
    pub snapshot_bound: bool,
}

/// Resolve all requested pointers while validating each array only once.
pub fn source_identities(
    root: &Value,
    pointers: &[&str],
) -> Result<BTreeMap<String, SourceIdentity>> {
    let mut arrays: BTreeMap<String, Vec<IdentityPart>> = BTreeMap::new();
    let mut identities = BTreeMap::new();
    for pointer in pointers {
        ensure!(
            pointer.starts_with('/'),
            "Expected a non-root JSON pointer: {pointer}"
        );
        let mut node = root;
        let mut parts = Vec::new();
        let mut path = String::new();
        for encoded in pointer[1..].split('/') {
            let token = encoded.replace("~1", "/").replace("~0", "~");
            match node {
                Value::Object(map) => {
                    node = map
                        .get(&token)
                        .with_context(|| format!("Missing field in {pointer}: {token}"))?;
                    parts.push(IdentityPart::Field(token));
                }
                Value::Array(items) => {
                    let index: usize = token.parse().context("Invalid array index")?;
                    node = items
                        .get(index)
                        .with_context(|| format!("Array index outside {pointer}"))?;
                    if !arrays.contains_key(&path) {
                        arrays.insert(path.clone(), array_parts(items, &path)?);
                    }
                    parts.push(arrays[&path][index].clone());
                }
                _ => bail!("Non-container encountered in JSON pointer {pointer}"),
            }
            path.push('/');
            path.push_str(encoded);
        }
        identities.insert(
            (*pointer).to_string(),
            SourceIdentity {
                encoded: serde_json::to_string(&parts)?,
                snapshot_bound: parts
                    .iter()
                    .any(|part| matches!(part, IdentityPart::SnapshotEntry { .. })),
            },
        );
    }
    Ok(identities)
}

fn array_parts(items: &[Value], path: &str) -> Result<Vec<IdentityPart>> {
    let key = ["id", "key", "index"]
        .into_iter()
        .find(|key| items.iter().any(|item| item.get(key).is_some()));
    let mut values = BTreeSet::new();
    if let Some(key) = key {
        let mut ambiguous = false;
        for item in items {
            let Some(value) = item.get(key) else {
                ambiguous = true;
                continue;
            };
            ensure!(
                value.is_number() || value.as_str().is_some_and(|text| !text.is_empty()),
                "Invalid {key} identifier in {path}: {value}"
            );
            if !values.insert(serde_json::to_string(value)?) {
                ambiguous = true;
            }
        }
        if ambiguous {
            // Bind every position to the complete array, never just a repeated ID.
            // Any content, ordering or metadata change creates new identities.
            let sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(items)?));
            return Ok((0..items.len())
                .map(|index| IdentityPart::SnapshotEntry {
                    sha256: sha256.clone(),
                    index,
                })
                .collect());
        }
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| match key {
            Some(key) => Ok(IdentityPart::Entry {
                key: key.into(),
                value: item
                    .get(key)
                    .with_context(|| format!("Missing validated {key} in {path}"))?
                    .clone(),
            }),
            None => Ok(IdentityPart::Position(index)),
        })
        .collect()
}
