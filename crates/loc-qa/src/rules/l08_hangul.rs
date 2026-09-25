use crate::issue::{Issue, IssueSeverity};
use regex::Regex;
use std::sync::LazyLock;

static HANGUL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\u{ac00}-\u{d7af}\u{1100}-\u{11ff}\u{3130}-\u{318f}]").unwrap()
});

pub fn check_l08_hangul_residual(text: &str) -> Option<Issue> {
    if let Some(mat) = HANGUL_RE.find(text) {
        Some(Issue {
            rule_id: "L08".to_string(),
            severity: IssueSeverity::Error,
            message: format!("检测到未翻译的韩文残留字符: \"{}\"", mat.as_str()),
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
    fn test_l08() {
        assert!(check_l08_hangul_residual("裂隙终于彻底撕裂……").is_none());
        assert!(check_l08_hangul_residual("裂隙终于균열彻底撕裂……").is_some());
    }
}
