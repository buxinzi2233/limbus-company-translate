use crate::issue::{Issue, IssueSeverity};
use regex::Regex;
use std::sync::LazyLock;

static NUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[-+−]?\d+(\.\d+)?%?").unwrap());

fn extract_numbers(text: &str) -> Vec<String> {
    let without_tags = super::l05_tmp::TAG_RE.replace_all(text, " ");
    let visible = super::l03_placeholder::PLACEHOLDER_RE.replace_all(&without_tags, " ");
    NUM_RE
        .find_iter(&visible)
        .map(|m| {
            let number = m.as_str().replace('−', "-");
            if number.starts_with('-')
                && visible[..m.start()]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_digit())
            {
                number[1..].to_string()
            } else {
                number
            }
        })
        .collect()
}

/// 跨语言数值一致性硬核稽核 (防止 AI/人工漏改、篡改技能数值与回合数)
pub fn check_numeric_invariant(source_kr: &str, target_zh: &str) -> Option<Issue> {
    let mut kr_nums = extract_numbers(source_kr);
    let mut zh_nums = extract_numbers(target_zh);

    kr_nums.sort();
    zh_nums.sort();

    if kr_nums != zh_nums {
        Some(Issue {
            rule_id: "NUMERIC_MISMATCH".to_string(),
            severity: IssueSeverity::Error,
            message: format!(
                "跨语言数值失配: 原文提取={:?}, 译文提取={:?} (可能漏译或写错数值/百分比)",
                kr_nums, zh_nums
            ),
            pointer: None,
            auto_fixable: false,
            suggested_fix: None,
        })
    } else {
        None
    }
}

/// 机制正反极性反转校验 (防止 '以下' 翻成 '以上'、'减少' 翻成 '增加')
pub fn check_polarity_conflict(source_kr: &str, target_zh: &str) -> Option<Issue> {
    // 1. 以下 vs 以上
    if source_kr.contains("이하") && target_zh.contains("以上") && !target_zh.contains("以下")
    {
        return Some(Issue {
            rule_id: "POLARITY_CONFLICT".to_string(),
            severity: IssueSeverity::Error,
            message: "机制极性严重反转: 原文为 '이하 (以下)'，译文误写为 '以上'".to_string(),
            pointer: None,
            auto_fixable: false,
            suggested_fix: None,
        });
    }

    if source_kr.contains("이상") && target_zh.contains("以下") && !target_zh.contains("以上")
    {
        return Some(Issue {
            rule_id: "POLARITY_CONFLICT".to_string(),
            severity: IssueSeverity::Error,
            message: "机制极性严重反转: 原文为 '이상 (以上)'，译文误写为 '以下'".to_string(),
            pointer: None,
            auto_fixable: false,
            suggested_fix: None,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeric_invariant() {
        let kr = "체력이 20% 이하일 때, 3턴 동안 피해량 50% 증가";
        let zh_ok = "体力在20%以下时，在3回合内伤害量增加50%";
        let zh_bad = "体力在20%以下时，在2回合内伤害量增加50%"; // 3 变 2

        assert!(check_numeric_invariant(kr, zh_ok).is_none());
        assert!(check_numeric_invariant(kr, zh_bad).is_some());
    }

    #[test]
    fn test_polarity_conflict() {
        let kr = "체력이 25% 이하일 때";
        let zh_inverted = "体力在25%以上时";
        let zh_correct = "体力在25%以下时";

        assert!(check_polarity_conflict(kr, zh_inverted).is_some());
        assert!(check_polarity_conflict(kr, zh_correct).is_none());
    }
}
