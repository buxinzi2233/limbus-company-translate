use anyhow::{bail, Result};
use loc_core::Store;
use loc_parser::{inject_translations_into_value, serialize_to_game_json};
use loc_qa::{check_segment_pair, IssueSeverity};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct TicketInputItem {
    pub file: String,
    pub pointer: String,
    pub source_kr: Option<String>,
    pub target_zh: Option<String>,
    pub zh: Option<String>,
    pub kr: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct InjectSummary {
    pub modified_files: usize,
    pub injected_segments: usize,
    pub skipped_untranslated: usize,
    pub qa_violations: usize,
    pub elapsed_ms: u128,
}

pub fn run_inject(
    store: &Store,
    workspace_root: &Path,
    kr_template_root: Option<&Path>,
    ticket_path: Option<&Path>,
    qa_check: bool,
    force: bool,
) -> Result<InjectSummary> {
    let start = Instant::now();

    // 收集待注入的条目: key = relative_path, val = Vec<(pointer, target_zh, source_kr)>
    let mut file_patches: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
    let mut summary = InjectSummary::default();

    if let Some(tp) = ticket_path {
        if !tp.exists() {
            bail!("指定的工单文件不存在: {}", tp.display());
        }
        let file = File::open(tp)?;
        let reader = BufReader::new(file);

        for line_res in reader.lines() {
            let line = line_res?;
            if line.trim().is_empty() {
                continue;
            }

            let item: TicketInputItem = match serde_json::from_str(&line) {
                Ok(it) => it,
                Err(e) => {
                    tracing::warn!("跳过非法工单行: {} ({})", line, e);
                    continue;
                }
            };

            let zh_val = item.target_zh.or(item.zh);
            let kr_val = item.source_kr.or(item.kr).unwrap_or_default();

            if let Some(zh) = zh_val {
                if !zh.trim().is_empty() && zh.trim() != kr_val.trim() {
                    file_patches
                        .entry(item.file)
                        .or_default()
                        .push((item.pointer, zh, kr_val));
                } else {
                    summary.skipped_untranslated += 1;
                }
            } else {
                summary.skipped_untranslated += 1;
            }
        }
    } else {
        // 从数据库中提取已有译文进行回填
        // 查询所有有 target_zh 的条目
        let conn = store.raw_connection();
        let conn_guard = conn.lock().unwrap();
        let mut stmt = conn_guard.prepare(
            r#"
            SELECT f.relative_path, s.key_path, s.source_kr, s.target_zh
            FROM segments s
            JOIN files f ON f.id = s.file_id
            WHERE s.target_zh IS NOT NULL AND TRIM(s.target_zh) != '' AND s.target_zh != s.source_kr
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        for r in rows {
            let (rel_path, pointer, kr, zh) = r?;
            file_patches
                .entry(rel_path)
                .or_default()
                .push((pointer, zh, kr));
        }
    }

    let workspace_effective = if workspace_root.join("LimbusCompany_Data/Lang/LLC_zh-CN").exists() {
        workspace_root.join("LimbusCompany_Data/Lang/LLC_zh-CN")
    } else {
        workspace_root.to_path_buf()
    };

    fs::create_dir_all(&workspace_effective)?;

    for (rel_path, patches) in file_patches {
        let target_file = workspace_effective.join(&rel_path);

        // 获取原 JSON 树
        let mut root_val: Value = if target_file.exists() {
            let content = fs::read_to_string(&target_file)?;
            serde_json::from_str(&content)?
        } else if let Some(kr_root) = kr_template_root {
            // 如果目标文件不存在，从韩文原版目录找对应模板
            let template_file = if kr_root.join(&rel_path).exists() {
                kr_root.join(&rel_path)
            } else {
                let kr_prefixed = format!("KR_{}", rel_path);
                kr_root.join(&kr_prefixed)
            };

            if !template_file.exists() {
                bail!("目标文件不存在且韩文模板库中亦无此文件: {}", rel_path);
            }
            let content = fs::read_to_string(&template_file)?;
            serde_json::from_str(&content)?
        } else {
            bail!(
                "工作区不存在目标文件: {}，且未提供 --kr 模板目录作为底座初始化",
                rel_path
            );
        };

        // 门禁预检
        if qa_check {
            let mut fatal_count = 0;
            for (ptr, zh, kr) in &patches {
                let issues = check_segment_pair(kr, zh);
                for issue in issues {
                    if issue.severity == IssueSeverity::Fatal {
                        fatal_count += 1;
                        summary.qa_violations += 1;
                        tracing::error!(
                            "[{}] 指针 {} 存在致命门禁违规: {}",
                            rel_path,
                            ptr,
                            issue.message
                        );
                    }
                }
            }

            if fatal_count > 0 && !force {
                bail!(
                    "文件 {} 发现 {} 项致命门禁违规 (波浪号/占位符/未闭合标签)！注入已熔断。使用 --force 忽略或先执行 loc qa / fix 自愈。",
                    rel_path,
                    fatal_count
                );
            }
        }

        // 执行 RFC-6901 指针精准原地无损回填
        let patch_tuples: Vec<(&str, &str)> = patches
            .iter()
            .map(|(p, zh, _)| (p.as_str(), zh.as_str()))
            .collect();

        let injected = inject_translations_into_value(&mut root_val, &patch_tuples)?;
        let serialized = serialize_to_game_json(&root_val)?;

        if let Some(parent) = target_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target_file, serialized)?;

        summary.modified_files += 1;
        summary.injected_segments += injected;
    }

    summary.elapsed_ms = start.elapsed().as_millis();
    Ok(summary)
}
