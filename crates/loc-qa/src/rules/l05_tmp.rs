use crate::issue::{Issue, IssueSeverity};
use regex::Regex;
use std::sync::LazyLock;

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"</?([a-zA-Z0-9]+)(=[^>]*)?>").unwrap());

pub fn check_l05_tmp_tags(text: &str) -> Option<Issue> {
    let mut stack: Vec<String> = Vec::new();

    for cap in TAG_RE.captures_iter(text) {
        let full_tag = cap.get(0).unwrap().as_str();
        let tag_name = cap.get(1).unwrap().as_str().to_lowercase();

        // 忽略自闭合/单标签
        if tag_name == "sprite" || tag_name == "br" {
            continue;
        }

        if full_tag.starts_with("</") {
            // 闭合标签
            if let Some(top) = stack.pop() {
                if top != tag_name {
                    return Some(Issue {
                        rule_id: "L05".to_string(),
                        severity: IssueSeverity::Fatal,
                        message: format!(
                            "TextMeshPro 标签交叉闭合错误: 期望 </{}> 但遇到了 </{}> (会导致 Unity 渲染崩溃)",
                            top, tag_name
                        ),
                        pointer: None,
                        auto_fixable: false,
                        suggested_fix: None,
                    });
                }
            } else {
                return Some(Issue {
                    rule_id: "L05".to_string(),
                    severity: IssueSeverity::Fatal,
                    message: format!("遇到多余的闭合标签: </{}>", tag_name),
                    pointer: None,
                    auto_fixable: false,
                    suggested_fix: None,
                });
            }
        } else {
            // 开标签
            stack.push(tag_name);
        }
    }

    if !stack.is_empty() {
        Some(Issue {
            rule_id: "L05".to_string(),
            severity: IssueSeverity::Fatal,
            message: format!("未闭合的 TextMeshPro 标签: {:?}", stack),
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
    fn test_l05() {
        let ok = "<color=#ff0000><b>暴击！</b></color><sprite name=\"dice\"><br>";
        assert!(check_l05_tmp_tags(ok).is_none());

        let unclosed = "<color=#ff0000>暴击！";
        assert!(check_l05_tmp_tags(unclosed).is_some());

        let crossed = "<b><color=#ff0000>错位</b></color>";
        assert!(check_l05_tmp_tags(crossed).is_some());
    }
}
