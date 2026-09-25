use anyhow::Result;
use loc_core::{Store, TMEntry};
use loc_parser::extract_segments_from_value;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;

use crate::matcher::hash_text;

#[derive(Debug, Clone, Default)]
pub struct IndexSummary {
    pub scanned_files: usize,
    pub aligned_entries: usize,
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
    let mut aligned_entries = 0;

    let zh_root_effective = if zh_root.join("LimbusCompany_Data/Lang/LLC_zh-CN").exists() {
        zh_root.join("LimbusCompany_Data/Lang/LLC_zh-CN")
    } else {
        zh_root.to_path_buf()
    };

    for entry in WalkDir::new(kr_root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let rel_path = path.strip_prefix(kr_root)?;
        // 处理可能存在的 KR_ 前缀 (如 KR_StoryData/ -> StoryData/)
        let normalized_rel_str = rel_path.to_string_lossy().replace("KR_", "");
        let zh_file = zh_root_effective.join(&normalized_rel_str);

        if !zh_file.exists() {
            continue;
        }

        let kr_content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let zh_content = match fs::read_to_string(&zh_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let kr_val: Value = match serde_json::from_str(&kr_content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let zh_val: Value = match serde_json::from_str(&zh_content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let kr_items = extract_segments_from_value(&kr_val, true)?;
        let zh_items = extract_segments_from_value(&zh_val, false)?;

        let zh_map: HashMap<&str, &str> = zh_items
            .iter()
            .map(|item| (item.pointer.as_str(), item.source_kr.as_str()))
            .collect();

        let mut batch = Vec::with_capacity(kr_items.len());
        for kr_item in &kr_items {
            if let Some(&zh_trans) = zh_map.get(kr_item.pointer.as_str()) {
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

        if !batch.is_empty() {
            let count = store.upsert_tm_entries_batch(&batch)?;
            aligned_entries += count;
        }

        scanned_files += 1;
    }

    Ok(IndexSummary {
        scanned_files,
        aligned_entries,
        elapsed_ms: start.elapsed().as_millis(),
    })
}
