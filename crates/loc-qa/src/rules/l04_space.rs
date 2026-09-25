use crate::issue::{Issue, IssueSeverity};
use regex::Regex;
use std::sync::LazyLock;

// 匹配 [关键词] 后面不是空格、不是行尾、不是标点、也不是另一个 '['
static BRACKET_MISSING_SPACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\[[^\]\n]+\])([^\s\]\)\.,;，。！？\n])").unwrap()
});

pub fn check_l04_space(text: &str) -> Option<Issue> {
    if let Some(mat) = BRACKET_MISSING_SPACE.find(text) {
        Some(Issue {
            rule_id: "L04".to_string(),
            severity: IssueSeverity::Error,
            message: format!("战斗关键词方括号后缺失半角空格，将导致悬浮窗无法触发: \"{}\"", mat.as_str()),
            pointer: None,
            auto_fixable: true,
            suggested_fix: Some(fix_l04_bracket_space(text)),
        })
    } else {
        None
    }
}

pub fn fix_l04_bracket_space(text: &str) -> String {
    BRACKET_MISSING_SPACE.replace_all(text, "$1 $2").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l04() {
        let bad = "自身获得2层[呼吸法]状态，并使目标的[流血]层数增加";
        assert!(check_l04_space(bad).is_some());

        let fixed = fix_l04_bracket_space(bad);
        assert_eq!(fixed, "自身获得2层[呼吸法] 状态，并使目标的[流血] 层数增加");
        assert!(check_l04_space(&fixed).is_none());
    }
}
