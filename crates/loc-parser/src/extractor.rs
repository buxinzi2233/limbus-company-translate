use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::hangul::contains_hangul;
use crate::pointer::encode_token;

/// 从游戏 JSON 中提取出的待本地化单元
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedItem {
    /// RFC-6901 JSON 树中的绝对寻址路径 (如 /dataList/0/texts/1/text)
    pub pointer: String,
    /// 原始韩文文本
    pub source_kr: String,
    /// 说话人/角色名 (如有)
    pub speaker: Option<String>,
    /// 所属字段名 (如 content, text, desc, name)
    pub field_name: String,
}

/// 判定字段是否属于确定性的代码元数据 (绝对排除)
fn is_metadata_blacklist_key(key: &str) -> bool {
    matches!(
        key.to_lowercase().as_str(),
        "id" | "key"
            | "model"
            | "voice"
            | "sound"
            | "effect"
            | "bgm"
            | "tag"
            | "color"
            | "code"
            | "icon"
            | "image"
            | "prefab"
            | "type"
            | "level"
            | "index"
            | "abnormality"
            | "personalityid"
            | "skin"
            | "costume"
            | "speaker"
            | "teller"
    )
}

/// 泛化 AST 递归遍历提取器
/// - `only_hangul = true`: 仅提取包含韩文的生肉条目 (用于从客户端提取新增待译文本)
/// - `only_hangul = false`: 提取所有自然语言文本 (用于从零协会中文库提取语料充实 TM 记忆库)
pub fn extract_segments_from_value(root: &Value, only_hangul: bool) -> Result<Vec<ExtractedItem>> {
    let mut items = Vec::new();
    traverse_node(root, "", None, only_hangul, &mut items);
    Ok(items)
}

fn traverse_node(
    current: &Value,
    current_pointer: &str,
    inherited_speaker: Option<&str>,
    only_hangul: bool,
    results: &mut Vec<ExtractedItem>,
) {
    match current {
        Value::Object(map) => {
            // 尝试在当前层嗅探说话人 (teller 优先，其次 speaker，其次 model)
            let mut local_speaker = inherited_speaker;
            if let Some(teller) = map.get("teller").and_then(Value::as_str) {
                if !teller.trim().is_empty() {
                    local_speaker = Some(teller);
                }
            } else if let Some(speaker) = map.get("speaker").and_then(Value::as_str) {
                if !speaker.trim().is_empty() {
                    local_speaker = Some(speaker);
                }
            } else if let Some(model) = map.get("model").and_then(Value::as_str) {
                if !model.trim().is_empty() {
                    local_speaker = Some(model);
                }
            }

            for (key, val) in map {
                let token = encode_token(key);
                let next_pointer = format!("{}/{}", current_pointer, token);

                if is_metadata_blacklist_key(key) {
                    continue;
                }

                if let Some(text) = val.as_str() {
                    let should_extract = if only_hangul {
                        contains_hangul(text) && !text.trim().is_empty()
                    } else {
                        !text.trim().is_empty()
                    };

                    if should_extract {
                        results.push(ExtractedItem {
                            pointer: next_pointer.clone(),
                            source_kr: text.to_string(),
                            speaker: local_speaker.map(ToString::to_string),
                            field_name: key.clone(),
                        });
                    }
                } else {
                    traverse_node(val, &next_pointer, local_speaker, only_hangul, results);
                }
            }
        }
        Value::Array(list) => {
            for (idx, val) in list.iter().enumerate() {
                let next_pointer = format!("{}/{}", current_pointer, idx);
                match val {
                    Value::String(text) => {
                        let should_extract = if only_hangul {
                            contains_hangul(text) && !text.trim().is_empty()
                        } else {
                            !text.trim().is_empty()
                        };

                        if should_extract {
                            let field_name = current_pointer.rsplit('/').next().unwrap_or("").to_string();
                            results.push(ExtractedItem {
                                pointer: next_pointer.clone(),
                                source_kr: text.to_string(),
                                speaker: inherited_speaker.map(ToString::to_string),
                                field_name,
                            });
                        }
                    }
                    _ => {
                        traverse_node(val, &next_pointer, inherited_speaker, only_hangul, results);
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_story_data() -> Result<()> {
        let story_json = json!({
            "dataList": [
                {
                    "id": 1,
                    "place": "5F 레스토랑",
                    "content": "균열이 마침내 완전히 찢어지고..."
                },
                {
                    "id": 2,
                    "model": "이상",
                    "content": "그대는 변하였구료, 푸이 군."
                }
            ]
        });

        let items = extract_segments_from_value(&story_json, true)?;
        assert_eq!(items.len(), 3); // place + content + content

        assert_eq!(items[0].pointer, "/dataList/0/place");
        assert_eq!(items[0].source_kr, "5F 레스토랑");
        assert_eq!(items[0].speaker, None);

        assert_eq!(items[1].pointer, "/dataList/0/content");
        assert_eq!(items[1].source_kr, "균열이 마침내 완전히 찢어지고...");

        assert_eq!(items[2].pointer, "/dataList/1/content");
        assert_eq!(items[2].source_kr, "그대는 변하였구료, 푸이 군.");
        assert_eq!(items[2].speaker.as_deref(), Some("이상"));

        Ok(())
    }

    #[test]
    fn test_extract_rpg_system_nested() -> Result<()> {
        let rpg_json = json!({
            "dataList": [
                {
                    "key": "D1010",
                    "texts": [
                        {
                            "index": 0,
                            "speaker": "이상",
                            "text": "잠시 발걸음을 멈추어도 되겠소."
                        },
                        {
                            "index": 1,
                            "speaker": "단테",
                            "text": "<신체 능력이 약해진 것 때문이지?>"
                        }
                    ]
                }
            ]
        });

        let items = extract_segments_from_value(&rpg_json, true)?;
        assert_eq!(items.len(), 2);

        assert_eq!(items[0].pointer, "/dataList/0/texts/0/text");
        assert_eq!(items[0].source_kr, "잠시 발걸음을 멈추어도 되겠소.");
        assert_eq!(items[0].speaker.as_deref(), Some("이상"));

        assert_eq!(items[1].pointer, "/dataList/0/texts/1/text");
        assert_eq!(items[1].source_kr, "<신체 능력이 약해진 것 때문이지?>");
        assert_eq!(items[1].speaker.as_deref(), Some("단테"));

        Ok(())
    }

    #[test]
    fn test_extract_unknown_schema_drift() -> Result<()> {
        // 模拟未来官方全新变异结构：官方新搞了一个嵌套三层的 event_node，且字段名是 dialogue
        let new_feature_json = json!({
            "metadata": { "version": "9.9", "id": "special_99" },
            "events": [
                {
                    "node": {
                        "dialogue": "새로운 미지의 이벤트 대사입니다.",
                        "choice_options": [
                            "첫번째 선택지",
                            "두번째 선택지"
                        ]
                    }
                }
            ]
        });

        let items = extract_segments_from_value(&new_feature_json, true)?;
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].pointer, "/events/0/node/dialogue");
        assert_eq!(items[0].source_kr, "새로운 미지의 이벤트 대사입니다.");
        assert_eq!(items[1].pointer, "/events/0/node/choice_options/0");
        assert_eq!(items[1].source_kr, "첫번째 선택지");
        assert_eq!(items[2].pointer, "/events/0/node/choice_options/1");
        assert_eq!(items[2].source_kr, "두번째 선택지");

        Ok(())
    }
}
