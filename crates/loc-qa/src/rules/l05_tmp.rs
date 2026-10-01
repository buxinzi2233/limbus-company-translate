use crate::issue::{Issue, IssueSeverity};
use regex::Regex;
use std::sync::LazyLock;

pub(super) static TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)</?(align|allcaps|alpha|b|br|color|cspace|font|font-weight|gradient|i|indent|line-height|line-indent|link|lowercase|margin|margin-left|margin-right|mark|mspace|nobr|noparse|page|pos|rotate|s|size|smallcaps|space|sprite|strikethrough|style|sub|sup|u|uppercase|voffset|width)(?:[= ][^<>]*)?/?>").unwrap()
});

pub fn check_l05_tmp_tags(text: &str) -> Option<Issue> {
    let mut stack: Vec<String> = Vec::new();

    for cap in TAG_RE.captures_iter(text) {
        let full_tag = cap.get(0).unwrap().as_str();
        let tag_name = cap.get(1).unwrap().as_str().to_lowercase();

        if stack.last().is_some_and(|name| name == "noparse")
            && !(tag_name == "noparse" && full_tag.starts_with("</"))
        {
            continue;
        }
        // These tags change state or insert a glyph without an opening/closing pair.
        if matches!(
            tag_name.as_str(),
            "sprite" | "br" | "page" | "space" | "pos" | "alpha" | "margin-left" | "margin-right"
        ) {
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
                            "TextMeshPro 标签交叉闭合错误: 期望 </{}> 但遇到了 </{}> (会导致富文本显示异常)",
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

/// Preserve renderer control tags and their attributes across translation.
pub fn check_tmp_invariant(source: &str, target: &str) -> Option<Issue> {
    let tags = |text: &str| {
        let mut result: Vec<String> = TAG_RE
            .find_iter(text)
            .map(|item| item.as_str().to_string())
            .collect();
        result.sort();
        result
    };
    let source_tags = tags(source);
    let target_tags = tags(target);
    if source_tags == target_tags {
        None
    } else {
        Some(Issue {
            rule_id: "TMP_MISMATCH".into(),
            severity: IssueSeverity::Error,
            message: format!(
                "富文本控制标签或属性不匹配: 原文={source_tags:?}, 译文={target_tags:?}"
            ),
            pointer: None,
            auto_fixable: false,
            suggested_fix: None,
        })
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
