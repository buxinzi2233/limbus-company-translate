use anyhow::{bail, Result};
use loc_core::{FileRecord, Project, Segment, Stage, Store};
use loc_parser::{contains_hangul, extract_segments_from_value};
use loc_tm::MatchTier;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
pub struct TicketItem {
    pub file: String,
    pub pointer: String,
    pub speaker: Option<String>,
    pub source_kr: String,
    pub target_zh: Option<String>,
    pub tm_match: Option<String>,
    pub tm_tier: Option<String>,
    pub stage: String,
}

#[derive(Debug, Default, Serialize)]
pub struct ExtractSummary {
    pub scanned_files: usize,
    pub total_segments: usize,
    pub already_translated: usize,
    pub tm_exact_filled: usize,
    pub pending_untranslated: usize,
    pub elapsed_ms: u128,
}

pub fn run_extract(
    store: &Store,
    project_id_opt: Option<String>,
    kr_root: &Path,
    zh_root: &Path,
    ticket_path: Option<&Path>,
    auto_fill_tm: bool,
) -> Result<ExtractSummary> {
    let start = Instant::now();

    if !kr_root.exists() {
        bail!("韩文原版目录不存在: {}", kr_root.display());
    }

    // 获取或创建活跃项目
    let project_id = if let Some(pid) = project_id_opt {
        let proj = Project::new(&pid, &pid);
        store.upsert_project(&proj)?;
        pid
    } else if let Some(p) = store.get_latest_project()? {
        p.id
    } else {
        let default_proj = Project::new("limbus-active", "边狱公司当前活跃汉化");
        store.upsert_project(&default_proj)?;
        "limbus-active".to_string()
    };

    let zh_root_effective = if zh_root.join("LimbusCompany_Data/Lang/LLC_zh-CN").exists() {
        zh_root.join("LimbusCompany_Data/Lang/LLC_zh-CN")
    } else {
        zh_root.to_path_buf()
    };

    let mut summary = ExtractSummary::default();
    let mut ticket_writer = if let Some(tp) = ticket_path {
        if let Some(parent) = tp.parent() {
            fs::create_dir_all(parent)?;
        }
        Some(BufWriter::new(File::create(tp)?))
    } else {
        None
    };

    for entry in WalkDir::new(kr_root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let rel_path = path.strip_prefix(kr_root)?;
        let normalized_rel_str = rel_path.to_string_lossy().replace("KR_", "");
        let zh_file = zh_root_effective.join(&normalized_rel_str);

        let kr_content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let kr_val: Value = match serde_json::from_str(&kr_content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // 仅提取包含韩文的待译条目
        let kr_items = extract_segments_from_value(&kr_val, true)?;
        if kr_items.is_empty() {
            continue;
        }

        // 若中文目标文件存在，提取其中已有内容进行对照
        let zh_map: HashMap<String, String> = if zh_file.exists() {
            if let Ok(zh_content) = fs::read_to_string(&zh_file) {
                if let Ok(zh_val) = serde_json::from_str::<Value>(&zh_content) {
                    if let Ok(zh_items) = extract_segments_from_value(&zh_val, false) {
                        zh_items
                            .into_iter()
                            .map(|i| (i.pointer, i.source_kr))
                            .collect()
                    } else {
                        HashMap::new()
                    }
                } else {
                    HashMap::new()
                }
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        let mut file_segments = Vec::with_capacity(kr_items.len());
        let mut file_translated_count = 0;

        for kr_item in &kr_items {
            summary.total_segments += 1;
            let existing_zh = zh_map.get(&kr_item.pointer);

            let (target_zh, stage, tm_suggest, tm_tier_str) = match existing_zh {
                Some(zh_str) if !zh_str.trim().is_empty() && zh_str.trim() != kr_item.source_kr.trim() && !contains_hangul(zh_str) => {
                    // 已有稳定中文翻译
                    summary.already_translated += 1;
                    file_translated_count += 1;
                    (Some(zh_str.clone()), Stage::Reviewed, None, None)
                }
                _ => {
                    // 尚未翻译或仍为韩文生肉，查询 TM 记忆库
                    let tm_candidate = loc_tm::match_segment(&kr_item.source_kr, Some(&normalized_rel_str), store).ok().flatten();

                    match tm_candidate {
                        Some(candidate) if auto_fill_tm && (candidate.tier == MatchTier::Context101 || candidate.tier == MatchTier::Exact100) => {
                            summary.tm_exact_filled += 1;
                            file_translated_count += 1;
                            let tier_name = format!("{:?}", candidate.tier);
                            let target = candidate.target_zh.clone();
                            (Some(target), Stage::TmExact, Some(candidate.target_zh), Some(tier_name))
                        }
                        Some(candidate) => {
                            summary.pending_untranslated += 1;
                            let tier_name = format!("{:?}", candidate.tier);
                            (None, Stage::Untranslated, Some(candidate.target_zh), Some(tier_name))
                        }
                        None => {
                            summary.pending_untranslated += 1;
                            (None, Stage::Untranslated, None, None)
                        }
                    }
                }
            };

            let segment = Segment {
                id: 0,
                project_id: project_id.clone(),
                file_id: 0, // 将在 upsert_file 后绑定
                key_path: kr_item.pointer.clone(),
                source_kr: kr_item.source_kr.clone(),
                previous_kr: None,
                target_zh: target_zh.clone(),
                speaker: kr_item.speaker.clone(),
                context_info: Some(kr_item.field_name.clone()),
                stage,
                qa_passed: true,
                qa_issues: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            // 如果需要导出工单，且本条目属于未翻译或 TM 预填待复核项
            if let Some(writer) = &mut ticket_writer {
                if stage == Stage::Untranslated || stage == Stage::TmExact {
                    let ticket_item = TicketItem {
                        file: normalized_rel_str.clone(),
                        pointer: kr_item.pointer.clone(),
                        speaker: kr_item.speaker.clone(),
                        source_kr: kr_item.source_kr.clone(),
                        target_zh,
                        tm_match: tm_suggest,
                        tm_tier: tm_tier_str,
                        stage: format!("{:?}", stage),
                    };
                    writeln!(writer, "{}", serde_json::to_string(&ticket_item)?)?;
                }
            }

            file_segments.push(segment);
        }

        // 登记文件资产
        let file_record = FileRecord {
            id: 0,
            project_id: project_id.clone(),
            relative_path: normalized_rel_str.clone(),
            category: if normalized_rel_str.starts_with("StoryData") {
                "story".to_string()
            } else if normalized_rel_str.starts_with("Skills") || normalized_rel_str.starts_with("Bufs") {
                "battle".to_string()
            } else {
                "system".to_string()
            },
            total_segments: kr_items.len() as i32,
            translated_segments: file_translated_count as i32,
            reviewed_segments: file_translated_count as i32,
            is_locked: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let file_id = store.upsert_file(&file_record)?;
        for seg in &mut file_segments {
            seg.file_id = file_id;
        }

        store.batch_upsert_segments(&file_segments)?;
        summary.scanned_files += 1;
    }

    if let Some(mut writer) = ticket_writer {
        writer.flush()?;
    }

    summary.elapsed_ms = start.elapsed().as_millis();
    Ok(summary)
}
