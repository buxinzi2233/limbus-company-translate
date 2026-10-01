use anyhow::{ensure, Context, Result};
use loc_core::{FileRecord, ImportFile, Project, Segment, SegmentWithFile, Stage, Store};
use loc_parser::inventory_korean_assets;
use loc_parser::{contains_hangul, extract_segments_from_value, parse_game_json};
use loc_tm::MatchTier;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

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

struct TmSuggestion {
    translation: Option<String>,
    tier: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ExtractSummary {
    pub scanned_files: usize,
    pub total_segments: usize,
    pub already_translated: usize,
    pub tm_exact_filled: usize,
    pub pending_untranslated: usize,
    pub needs_review: usize,
    pub snapshot_bound_segments: usize,
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
    let assets = inventory_korean_assets(kr_root)?;

    let project = match project_id_opt {
        Some(id) => store
            .get_project(&id)?
            .unwrap_or_else(|| Project::new(&id, &id)),
        None => store
            .get_latest_project()?
            .unwrap_or_else(|| Project::new("limbus-active", "边狱公司当前活跃汉化")),
    };
    let project_id = project.id.clone();

    let zh_root_effective = if zh_root.join("LimbusCompany_Data/Lang/LLC_zh-CN").exists() {
        zh_root.join("LimbusCompany_Data/Lang/LLC_zh-CN")
    } else {
        zh_root.to_path_buf()
    };

    let project = Project {
        live_client_kr_dir: Some(
            kr_root
                .canonicalize()?
                .to_str()
                .context("Korean directory is not UTF-8")?
                .to_string(),
        ),
        workspace_zh_dir: Some(
            zh_root_effective
                .canonicalize()?
                .to_str()
                .context("Chinese directory is not UTF-8")?
                .to_string(),
        ),
        ..project
    };

    let mut prepared_files = Vec::new();
    let mut snapshot_bound_segments = 0;
    let mut all_suggestions = HashMap::new();
    let ticket_destination = ticket_path
        .map(|path| validate_ticket_destination(store, path, kr_root, &zh_root_effective))
        .transpose()?;
    let mut ticket_writer = ticket_destination
        .as_ref()
        .map(|path| {
            let parent = path
                .parent()
                .context("Ticket destination has no parent directory")?;
            Ok::<_, anyhow::Error>(BufWriter::new(tempfile::NamedTempFile::new_in(parent)?))
        })
        .transpose()?;

    for (normalized_rel_str, path) in assets {
        let zh_file = zh_root_effective.join(&normalized_rel_str);

        let kr_content = fs::read_to_string(&path)
            .with_context(|| format!("Read Korean JSON {}", path.display()))?;
        let kr_val: Value = parse_game_json(kr_content.as_bytes())
            .with_context(|| format!("Parse Korean JSON {}", path.display()))?;

        // Extract all schema-defined display text, including non-Hangul strings.
        let kr_items = extract_segments_from_value(&kr_val)
            .with_context(|| format!("Extract Korean fields from {}", path.display()))?;

        let snapshot_count = kr_items.iter().filter(|item| item.snapshot_bound).count();
        snapshot_bound_segments += snapshot_count;
        if snapshot_count > 0 {
            tracing::warn!(file = %normalized_rel_str, segments = snapshot_count,
                "Ambiguous array identifiers: records are bound to this source snapshot; no automatic bilingual alignment or TM fill");
        }

        // 若中文目标文件存在，提取其中已有内容进行对照
        let zh_map: HashMap<String, String> = if zh_file.try_exists()? {
            let content = fs::read_to_string(&zh_file)
                .with_context(|| format!("Read Chinese JSON {}", zh_file.display()))?;
            let value: Value = parse_game_json(content.as_bytes())
                .with_context(|| format!("Parse Chinese JSON {}", zh_file.display()))?;
            let zh_items = extract_segments_from_value(&value)
                .with_context(|| format!("Extract Chinese fields from {}", zh_file.display()))?;
            ensure!(snapshot_count == 0 && !zh_items.iter().any(|item| item.snapshot_bound),
                "Cannot align snapshot-bound arrays with Chinese input in {normalized_rel_str}; use an empty translation input for source-only extraction and review these records independently");
            zh_items
                .into_iter()
                .map(|item| (item.identity, item.source_kr))
                .collect()
        } else {
            HashMap::new()
        };

        let mut file_segments = Vec::with_capacity(kr_items.len());
        let mut file_translated_count = 0;
        let mut suggestions = HashMap::new();

        for kr_item in &kr_items {
            let existing_zh = zh_map.get(&kr_item.identity);

            let (target_zh, stage, tm_suggest, tm_tier_str) = match existing_zh {
                Some(zh_str)
                    if !zh_str.trim().is_empty()
                        && zh_str.trim() != kr_item.source_kr.trim()
                        && !contains_hangul(zh_str) =>
                {
                    // 已有稳定中文翻译

                    file_translated_count += 1;
                    (Some(zh_str.clone()), Stage::Imported, None, None)
                }
                _ => {
                    // 尚未翻译或仍为韩文生肉，查询 TM 记忆库
                    let tm_candidate = loc_tm::match_segment(
                        &kr_item.source_kr,
                        Some(&normalized_rel_str),
                        store,
                    )?;

                    match tm_candidate {
                        Some(candidate)
                            if auto_fill_tm
                                && !kr_item.snapshot_bound
                                && (candidate.tier == MatchTier::FileExact
                                    || candidate.tier == MatchTier::Exact100) =>
                        {
                            file_translated_count += 1;
                            let tier_name = format!("{:?}", candidate.tier);
                            let target = candidate.target_zh.clone();
                            (
                                Some(target),
                                Stage::TmExact,
                                Some(candidate.target_zh),
                                Some(tier_name),
                            )
                        }
                        Some(candidate) => {
                            let tier_name = format!("{:?}", candidate.tier);
                            (
                                None,
                                Stage::Untranslated,
                                Some(candidate.target_zh),
                                Some(tier_name),
                            )
                        }
                        None => (None, Stage::Untranslated, None, None),
                    }
                }
            };

            suggestions.insert(
                kr_item.pointer.clone(),
                TmSuggestion {
                    translation: tm_suggest,
                    tier: tm_tier_str,
                },
            );
            let issues = target_zh
                .as_deref()
                .map(|text| loc_qa::check_segment_pair(&kr_item.source_kr, text));
            let qa_passed = issues.as_ref().is_some_and(|issues| issues.is_empty());
            let qa_issues = match issues.filter(|issues| !issues.is_empty()) {
                Some(issues) => Some(serde_json::to_string(&issues)?),
                None => None,
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
                qa_passed,
                qa_issues,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            file_segments.push(segment);
        }

        // 登记文件资产
        let file_record = FileRecord {
            id: 0,
            project_id: project_id.clone(),
            relative_path: normalized_rel_str.clone(),
            category: loc_core::resource_category(&normalized_rel_str).to_string(),
            total_segments: kr_items.len() as i32,
            translated_segments: file_translated_count,
            reviewed_segments: file_translated_count,
            is_locked: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        all_suggestions.insert(normalized_rel_str, suggestions);
        prepared_files.push(ImportFile {
            identities: kr_items
                .iter()
                .map(|item| (item.pointer.clone(), item.identity.clone()))
                .collect(),
            file: file_record,
            segments: file_segments,
        });
    }

    let imported = store.import_files(&project, &prepared_files)?;
    if let (Some(writer), Some(path)) = (ticket_writer.take(), ticket_destination.as_deref()) {
        publish_ticket(writer, path, &imported, &all_suggestions).with_context(|| format!(
            "Database import committed in full, but ticket {} was not published; previous ticket is unchanged. Fix the output error before exporting again",
            path.display()))?;
    }
    Ok(ExtractSummary {
        snapshot_bound_segments,
        ..summarize_import(&imported, prepared_files.len(), start.elapsed().as_millis())
    })
}

fn summarize_import(
    items: &[SegmentWithFile],
    scanned_files: usize,
    elapsed_ms: u128,
) -> ExtractSummary {
    let mut summary = ExtractSummary {
        scanned_files,
        total_segments: items.len(),
        elapsed_ms,
        ..ExtractSummary::default()
    };
    for item in items {
        if item
            .target_zh
            .as_deref()
            .is_none_or(|text| text.trim().is_empty())
        {
            summary.pending_untranslated += 1;
        } else if (item.stage == Stage::UpstreamFrozen
            || (item.stage == Stage::Reviewed && item.review_confirmed))
            && item.qa_passed
        {
            summary.already_translated += 1;
        } else if item.stage == Stage::TmExact && item.qa_passed {
            summary.tm_exact_filled += 1;
        } else {
            summary.needs_review += 1;
        }
    }
    summary
}

fn publish_ticket(
    mut writer: BufWriter<tempfile::NamedTempFile>,
    path: &Path,
    items: &[SegmentWithFile],
    suggestions: &HashMap<String, HashMap<String, TmSuggestion>>,
) -> Result<()> {
    for item in items.iter().filter(|item| {
        matches!(
            item.stage,
            Stage::Untranslated
                | Stage::TmExact
                | Stage::OutdatedDiff
                | Stage::AiDrafted
                | Stage::Imported
        )
    }) {
        let suggestion = suggestions
            .get(&item.relative_path)
            .and_then(|file| file.get(&item.key_path))
            .context("Missing TM metadata for imported pointer")?;
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&TicketItem {
                file: item.relative_path.clone(),
                pointer: item.key_path.clone(),
                speaker: item.speaker.clone(),
                source_kr: item.source_kr.clone(),
                target_zh: item.target_zh.clone(),
                tm_match: suggestion.translation.clone(),
                tm_tier: suggestion.tier.clone(),
                stage: format!("{:?}", item.stage),
            })?
        )?;
    }
    writer.flush()?;
    let temporary = writer.into_inner().map_err(|error| error.into_error())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .with_context(|| format!("Publish completed ticket {}", path.display()))?;
    Ok(())
}

fn validate_ticket_destination(
    store: &Store,
    path: &Path,
    kr_root: &Path,
    zh_root: &Path,
) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let destination = parent
        .canonicalize()?
        .join(path.file_name().context("Ticket path must name a file")?);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) => ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Ticket destination must be a regular file, not a directory or symlink: {}",
            destination.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Inspect ticket destination {}", destination.display()))
        }
    }
    for input in [kr_root, zh_root] {
        if input.try_exists()? {
            ensure!(
                !destination.starts_with(input.canonicalize()?),
                "Ticket must be outside source and translation input directories: {}",
                destination.display()
            );
        }
    }
    if let Some(database) = store.database_path()? {
        let database = database.canonicalize()?;
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut reserved = database.as_os_str().to_os_string();
            reserved.push(suffix);
            ensure!(
                destination != Path::new(&reserved),
                "Ticket must not overwrite the database or its journal: {}",
                destination.display()
            );
        }
    }
    Ok(destination)
}
