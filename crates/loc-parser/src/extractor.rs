use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::pointer::encode_token;

/// 从游戏 JSON 中提取出的待本地化单元
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedItem {
    /// RFC-6901 JSON 树中的绝对寻址路径 (如 /dataList/0/texts/1/text)
    pub pointer: String,
    pub identity: String,
    pub snapshot_bound: bool,
    /// 原始韩文文本
    pub source_kr: String,
    /// 说话人/角色名 (如有)
    pub speaker: Option<String>,
    /// 所属字段名 (如 content, text, desc, name)
    pub field_name: String,
}

// Metadata is excluded by schema, never by the language of its value.
fn is_metadata_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
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
            | "imgstr"
            | "usage"
            | "codename"
            | "colorcode"
            | "outlinecolorcode"
            | "maintextcolor"
            | "maintextglowcolor"
            | "debugnodeid"
            | "iconid"
            | "keywords"
            | "undefined"
    )
}

// Display fields verified against the current client resources.
// Unknown nonempty strings fail the import rather than silently disappear.
fn is_display_field(key: &str) -> bool {
    matches!(
        key,
        "abName"
            | "abnormalityName"
            | "add"
            | "area"
            | "askLevelUp"
            | "behaveDesc"
            | "chapter"
            | "chapterNumber"
            | "chaptertitle"
            | "clue"
            | "company"
            | "content"
            | "desc"
            | "description"
            | "dialog"
            | "displayName"
            | "dlg"
            | "eventDesc"
            | "failureDesc"
            | "flavor"
            | "goalDescription1"
            | "goalDescription2"
            | "goalDescription3"
            | "goalDescription4"
            | "goalDescription5"
            | "goalDescription6"
            | "goalDescription7"
            | "goalDescription8"
            | "longName"
            | "lowMoraleDescription"
            | "mainText"
            | "message"
            | "messageDesc"
            | "min"
            | "name"
            | "nameWithTitle"
            | "nickName"
            | "openCondition"
            | "openConditionNumber"
            | "panicDescription"
            | "panicName"
            | "parttitle"
            | "place"
            | "prevDesc"
            | "rawDesc"
            | "relatedChapterText"
            | "result"
            | "sentence"
            | "shortName"
            | "simpleDesc"
            | "skinItemDesc"
            | "skinItemTitle"
            | "songWriter"
            | "speaker"
            | "specialName"
            | "statText"
            | "story"
            | "subDesc"
            | "subText"
            | "successDesc"
            | "summary"
            | "teacher"
            | "teller"
            | "text"
            | "timeline"
            | "title"
            | "variation"
            | "variation2"
    )
}

/// Extract nonempty display fields independently of their source language.
pub fn extract_segments_from_value(root: &Value) -> Result<Vec<ExtractedItem>> {
    let mut items = Vec::new();
    traverse_node(root, "", "", None, &mut items)?;
    let identities = crate::source_identities(
        root,
        &items
            .iter()
            .map(|item| item.pointer.as_str())
            .collect::<Vec<_>>(),
    )?;
    Ok(items
        .into_iter()
        .map(|item| ExtractedItem {
            identity: identities[&item.pointer].encoded.clone(),
            snapshot_bound: identities[&item.pointer].snapshot_bound,
            ..item
        })
        .collect())
}

fn traverse_node(
    current: &Value,
    current_pointer: &str,
    field_name: &str,
    inherited_speaker: Option<&str>,
    results: &mut Vec<ExtractedItem>,
) -> Result<()> {
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
                if is_metadata_key(key) {
                    continue;
                }
                let next_pointer = format!("{}/{}", current_pointer, encode_token(key));
                traverse_node(val, &next_pointer, key, local_speaker, results)?;
            }
        }
        Value::Array(list) => {
            for (idx, val) in list.iter().enumerate() {
                let next_pointer = format!("{}/{}", current_pointer, idx);
                traverse_node(val, &next_pointer, field_name, inherited_speaker, results)?;
            }
        }
        Value::String(text) if !text.trim().is_empty() => {
            if !is_display_field(field_name) {
                bail!("Unclassified localization string at {} (field {:?}); classify this field before importing", current_pointer, field_name);
            }
            results.push(ExtractedItem {
                identity: String::new(),
                snapshot_bound: false,
                pointer: current_pointer.to_owned(),
                source_kr: text.clone(),
                speaker: inherited_speaker.map(ToString::to_string),
                field_name: field_name.to_owned(),
            });
        }
        _ => {}
    }
    Ok(())
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

        let items = extract_segments_from_value(&story_json)?;
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

        let items = extract_segments_from_value(&rpg_json)?;
        assert_eq!(items.len(), 4);
        let items: Vec<_> = items
            .into_iter()
            .filter(|item| item.field_name == "text")
            .collect();

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

        let error = extract_segments_from_value(&new_feature_json).unwrap_err();
        assert!(error
            .to_string()
            .contains("Unclassified localization string"));
        assert!(error.to_string().contains("/events/0/node/dialogue"));

        Ok(())
    }
}
