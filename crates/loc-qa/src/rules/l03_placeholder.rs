use crate::issue::{Issue, IssueSeverity};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub(super) static PLACEHOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{|\}\}|\{([^{}]+)\}|[{}]").unwrap());

pub fn extract_placeholders(text: &str) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for cap in PLACEHOLDER_RE.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            *map.entry(m.as_str().to_string()).or_insert(0) += 1;
        }
    }
    map
}

pub fn check_l03_placeholder(source_kr: &str, target_zh: &str) -> Option<Issue> {
    let kr_map = extract_placeholders(source_kr);
    let zh_map = extract_placeholders(target_zh);

    let malformed_target = PLACEHOLDER_RE
        .find_iter(target_zh)
        .any(|item| matches!(item.as_str(), "{" | "}"));
    if kr_map != zh_map || malformed_target {
        Some(Issue {
            rule_id: "L03".to_string(),
            severity: IssueSeverity::Fatal,
            message: format!("动态占位符不匹配: 原文={:?}, 译文={:?}", kr_map, zh_map),
            pointer: None,
            auto_fixable: false,
            suggested_fix: None,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l03() {
        let kr = "{0}에게 {1}의 피해를 입힘";
        let zh_ok = "对{0}造成{1}点伤害";
        let zh_bad = "对{0}造成伤害"; // 缺少 {1}

        assert!(check_l03_placeholder(kr, zh_ok).is_none());
        assert!(check_l03_placeholder(kr, zh_bad).is_some());
    }
}
