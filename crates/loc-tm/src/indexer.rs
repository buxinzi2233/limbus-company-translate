use anyhow::{ensure, Context, Result};
use loc_core::{Store, TMEntry};
use loc_parser::extract_segments_from_value;
use loc_parser::{inventory_korean_assets, parse_game_json};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::matcher::hash_text;

#[derive(Debug, Clone, Default)]
pub struct IndexSummary {
    pub scanned_files: usize,
    pub aligned_entries: usize,
    pub inserted_entries: usize,
    pub elapsed_ms: u128,
}

/// 将韩文原版目录与零协会中文汉化目录进行双语对齐，全量反哺吸纳进 TM 记忆库
pub fn index_parallel_corpora(
    kr_root: &Path,
    zh_root: &Path,
    origin: &str,
    priority: i32,
    store: &Store,
) -> Result<IndexSummary> {
    let start = Instant::now();
    let mut scanned_files = 0;
    let mut entries = Vec::new();
    let assets = inventory_korean_assets(kr_root)?;

    let zh_root_effective = if zh_root.join("LimbusCompany_Data/Lang/LLC_zh-CN").exists() {
        zh_root.join("LimbusCompany_Data/Lang/LLC_zh-CN")
    } else {
        zh_root.to_path_buf()
    };

    for (normalized_rel_str, path) in assets {
        let zh_file = zh_root_effective.join(&normalized_rel_str);

        if !zh_file.try_exists()? {
            continue;
        }

        let kr_content =
            fs::read_to_string(&path).with_context(|| format!("Read corpus {}", path.display()))?;
        let zh_content = fs::read_to_string(&zh_file)
            .with_context(|| format!("Read corpus {}", zh_file.display()))?;
        let kr_val: Value = parse_game_json(kr_content.as_bytes())
            .with_context(|| format!("Parse corpus {}", path.display()))?;
        let zh_val: Value = parse_game_json(zh_content.as_bytes())
            .with_context(|| format!("Parse corpus {}", zh_file.display()))?;

        let kr_items = extract_segments_from_value(&kr_val)
            .with_context(|| format!("Align Korean corpus {}", path.display()))?;
        let zh_items = extract_segments_from_value(&zh_val)
            .with_context(|| format!("Align Chinese corpus {}", zh_file.display()))?;

        ensure!(
            !kr_items.iter().chain(zh_items.iter()).any(|item| item.snapshot_bound),
            "Cannot index snapshot-bound arrays as bilingual TM: {normalized_rel_str}; record alignment requires independent review"
        );

        let zh_map: HashMap<&str, &str> = zh_items
            .iter()
            .map(|item| (item.identity.as_str(), item.source_kr.as_str()))
            .collect();

        let mut batch = Vec::with_capacity(kr_items.len());
        for kr_item in &kr_items {
            if let Some(&zh_trans) = zh_map.get(kr_item.identity.as_str()) {
                // 如果中文译文与韩文原文相同 (说明未翻译)，则跳过
                if zh_trans.trim() == kr_item.source_kr.trim() {
                    continue;
                }

                let h = hash_text(&kr_item.source_kr);
                let tm_entry = TMEntry {
                    id: 0,
                    hash_kr: h,
                    source_kr: kr_item.source_kr.clone(),
                    target_zh: zh_trans.to_string(),
                    file_scope: Some(normalized_rel_str.clone()),
                    speaker: kr_item.speaker.clone(),
                    source_origin: origin.to_string(),
                    priority,
                    created_at: chrono::Utc::now(),
                };

                batch.push(tm_entry);
            }
        }

        entries.extend(batch);

        scanned_files += 1;
    }

    let aligned_entries = entries.len();
    let inserted_entries = store.upsert_tm_entries_batch(&entries)?;
    Ok(IndexSummary {
        scanned_files,
        aligned_entries,
        inserted_entries,
        elapsed_ms: start.elapsed().as_millis(),
    })
}
