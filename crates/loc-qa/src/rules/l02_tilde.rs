use crate::issue::{Issue, IssueSeverity};

pub fn check_l02_tilde(text: &str) -> Option<Issue> {
    if text.contains('\u{FF5E}') || text.contains('\u{301C}') {
        Some(Issue {
            rule_id: "L02".to_string(),
            severity: IssueSeverity::Fatal,
            message: "发现全角波浪号 (～ 或 〜)，会导致 Unity TextMeshPro 渲染乱码方框 □ 甚至崩溃".to_string(),
            pointer: None,
            auto_fixable: true,
            suggested_fix: Some(text.replace(['\u{FF5E}', '\u{301C}'], "~")),
        })
    } else {
        None
    }
}

pub fn fix_l02_tilde(text: &str) -> String {
    text.replace(['\u{FF5E}', '\u{301C}'], "~")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l02() {
        let bad = "从1层～3层生效〜";
        assert!(check_l02_tilde(bad).is_some());
        let fixed = fix_l02_tilde(bad);
        assert_eq!(fixed, "从1层~3层生效~");
        assert!(check_l02_tilde(&fixed).is_none());
    }
}
