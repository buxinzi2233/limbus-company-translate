//! Advisory-only terminology checks; they never change hard QA or review eligibility.
use anyhow::Result;
use loc_core::{terminology::TermLink, Store};
use serde::Serialize;

#[derive(Serialize, schemars::JsonSchema)]
pub struct TermAdvice {
    pub rule_id: &'static str,
    pub message: String,
    pub evidence: TermLink,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TermAdviceReport {
    pub applicable: bool,
    pub available_links: usize,
    pub matched_terms: usize,
    pub blocking: bool,
    pub advice: Vec<TermAdvice>,
}

fn battle_description(path: &str, pointer: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let combat_field = ["/desc", "/description", "/summary"]
        .iter()
        .any(|field| pointer.ends_with(field));
    let combat_file = ["Skills", "Passives", "BattleKeywords", "Bufs", "EGOgift"]
        .iter()
        .any(|prefix| name.starts_with(prefix));
    (combat_file && combat_field)
        || (path.starts_with("RPGSystem/rpg-loc-item-") && pointer.ends_with("/statText"))
}

fn missing_space(text: &str, name: &str) -> bool {
    let mut square_depth = 0usize;
    let mut in_tag = false;
    for (index, ch) in text.char_indices() {
        match ch {
            '[' => {
                square_depth += 1;
                continue;
            }
            ']' => {
                square_depth = square_depth.saturating_sub(1);
                continue;
            }
            '<' => {
                in_tag = true;
                continue;
            }
            '>' => {
                in_tag = false;
                continue;
            }
            _ => {}
        }
        if square_depth == 0
            && !in_tag
            && text[index..].starts_with(name)
            && !text[index + name.len()..].starts_with(' ')
        {
            return true;
        }
    }
    false
}

pub fn check_term_advice(
    store: &Store,
    project: &str,
    id: i64,
    target: &str,
) -> Result<TermAdviceReport> {
    let row = store.get_segment(project, id)?;
    let applicable = battle_description(&row.relative_path, &row.key_path);
    let links = store.term_links(project)?;
    let mut report = TermAdviceReport {
        applicable,
        available_links: links.len(),
        matched_terms: 0,
        blocking: false,
        advice: Vec::new(),
    };
    if !applicable {
        return Ok(report);
    }
    for link in links {
        let token = format!("[{}]", link.keyword_id);
        if !row.source_kr.contains(&token) && !row.source_kr.contains(&link.source_kr) {
            continue;
        }
        report.matched_terms += 1;
        let issue = if !target.contains(&token) && !target.contains(&link.target_zh) {
            Some(("T01", format!("原文涉及 {} / {}，参考用名为“{}”；请核对是否漏译或使用了其他译名，不能仅据此判错。",link.source_kr,link.keyword_id,link.target_zh)))
        } else if missing_space(target, &link.target_zh) {
            Some(("T02", format!("战斗描述中的名称“{}”后未保留半角空格；零协会名称格式要求句尾也保留空格，键值格式 [键值] 不适用此规则，请人工核对。",link.target_zh)))
        } else {
            None
        };
        if let Some((rule_id, message)) = issue {
            report.advice.push(TermAdvice {
                rule_id,
                message,
                evidence: link,
            });
        }
    }
    Ok(report)
}
