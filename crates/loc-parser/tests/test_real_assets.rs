use anyhow::Result;
use loc_parser::{extract_segments_from_value, inject_translations_into_value, serialize_to_game_json};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn test_real_korean_baseline_extraction() -> Result<()> {
    // 测试从真实韩文基准资源提取生肉
    let kr_path = Path::new("/home/buxinzi/Documents/巴士汉化-哈基米版/references/baseline-KR/StoryData/1D101A.json");
    if !kr_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(kr_path)?;
    let parsed: Value = serde_json::from_str(&raw)?;

    let extracted = extract_segments_from_value(&parsed, true)?;
    assert!(!extracted.is_empty(), "韩文剧情 1D101A 应当提取出待译生肉");
    assert!(extracted.iter().any(|e| e.pointer.contains("/content")));

    Ok(())
}

#[test]
fn test_real_game_story_json_round_trip() -> Result<()> {
    let sample_path = Path::new("/home/buxinzi/Documents/巴士汉化-哈基米版/workspace/LLC_zh-CN/StoryData/S1029B.json");
    if !sample_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(sample_path)?;
    let parsed: Value = serde_json::from_str(&raw)?;

    // 提取中文已翻译文本 (用于 TM 记忆库语料吸纳)
    let extracted = extract_segments_from_value(&parsed, false)?;
    assert!(!extracted.is_empty(), "S1029B 应当提取出文本");

    // 验证说话人识别：至少有普伊？、但丁、李箱等
    let speakers: Vec<&str> = extracted.iter().filter_map(|e| e.speaker.as_deref()).collect();
    assert!(!speakers.is_empty(), "应当正确提取说话人上下文");

    // 模拟修改第 1 条文本并无损写回
    let mut modified = parsed.clone();
    let first = &extracted[0];
    let patch = [(first.pointer.as_str(), "【测试注入】第一句测试剧情")];

    inject_translations_into_value(&mut modified, &patch)?;
    let serialized = serialize_to_game_json(&modified)?;
    assert!(serialized.contains("【测试注入】第一句测试剧情"));

    Ok(())
}

#[test]
fn test_real_game_rpg_json_round_trip() -> Result<()> {
    let sample_path = Path::new("/home/buxinzi/Documents/巴士汉化-哈基米版/workspace/LLC_zh-CN/RPGSystem/rpg-loc-dialogue-floor-1-b.json");
    if !sample_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(sample_path)?;
    let parsed: Value = serde_json::from_str(&raw)?;

    // 提取嵌套的 texts[].text 结构
    let extracted = extract_segments_from_value(&parsed, false)?;
    assert!(extracted.len() > 100, "floor-1-b 应当提取出海量 RPG 对话条目");

    // 验证深层 pointer
    assert!(extracted.iter().any(|e| e.pointer.contains("/texts/")));

    Ok(())
}
