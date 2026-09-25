use anyhow::Result;
use serde_json::Value;

use crate::pointer::set_at_pointer;

/// 基于 RFC-6901 指针列表将翻译回填至原始 JSON AST
pub fn inject_translations_into_value(root: &mut Value, translations: &[(&str, &str)]) -> Result<usize> {
    let mut modified_count = 0;
    for (pointer, target_zh) in translations {
        set_at_pointer(root, pointer, Value::String(target_zh.to_string()))?;
        modified_count += 1;
    }
    Ok(modified_count)
}

/// 将 serde_json::Value 序列化为标准的无损 UTF-8 游戏 JSON 字符串 (2空格缩进，不转义中文)
pub fn serialize_to_game_json(root: &Value) -> Result<String> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"  ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(root, &mut ser)?;
    let mut res = String::from_utf8(buf)?;
    res.push('\n');
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_round_trip_injection() -> Result<()> {
        let mut original = json!({
            "dataList": [
                {
                    "id": 1,
                    "content": "균열이 마침내 완전히 찢어지고...",
                    "texts": [
                        { "index": 0, "text": "첫번째" }
                    ]
                }
            ]
        });

        let patch = [
            ("/dataList/0/content", "裂隙终于彻底撕裂……"),
            ("/dataList/0/texts/0/text", "第一条"),
        ];

        let count = inject_translations_into_value(&mut original, &patch)?;
        assert_eq!(count, 2);

        assert_eq!(
            original["dataList"][0]["content"].as_str(),
            Some("裂隙终于彻底撕裂……")
        );
        assert_eq!(
            original["dataList"][0]["texts"][0]["text"].as_str(),
            Some("第一条")
        );

        let serialized = serialize_to_game_json(&original)?;
        assert!(serialized.contains("\"content\": \"裂隙终于彻底撕裂……\""));
        assert!(serialized.contains("\"text\": \"第一条\""));
        assert!(serialized.ends_with('\n'));

        Ok(())
    }
}
