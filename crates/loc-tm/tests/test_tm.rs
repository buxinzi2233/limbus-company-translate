use anyhow::Result;
use loc_core::Store;
use loc_tm::{index_parallel_corpora, match_segment, MatchTier};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_tm_indexing_and_matching() -> Result<()> {
    let store = Store::open_in_memory()?;

    // 构造临时测试语料目录
    let temp = tempdir()?;
    let kr_dir = temp.path().join("kr");
    let zh_dir = temp.path().join("zh");
    fs::create_dir_all(kr_dir.join("StoryData"))?;
    fs::create_dir_all(zh_dir.join("StoryData"))?;

    let kr_sample = json!({
        "dataList": [
            { "id": 1, "content": "균열이 마침내 완전히 찢어지고..." },
            { "id": 2, "content": "몸 상태가 약해진 것 때문이지?" }
        ]
    });

    let zh_sample = json!({
        "dataList": [
            { "id": 1, "content": "裂隙终于彻底撕裂……" },
            { "id": 2, "content": "是因为身体衰弱的原因吗？" }
        ]
    });

    fs::write(
        kr_dir.join("StoryData/S1029.json"),
        serde_json::to_string_pretty(&kr_sample)?,
    )?;
    fs::write(
        zh_dir.join("StoryData/S1029.json"),
        serde_json::to_string_pretty(&zh_sample)?,
    )?;

    // 执行对齐与反哺
    let summary = index_parallel_corpora(&kr_dir, &zh_dir, "upstream_llc_gold", 100, &store)?;
    assert_eq!(summary.scanned_files, 1);
    assert_eq!(summary.aligned_entries, 2);

    // 测试 101% 上下文匹配
    let match_101 = match_segment("균열이 마침내 완전히 찢어지고...", Some("StoryData/S1029.json"), &store)?
        .expect("应当命中 TM");
    assert_eq!(match_101.tier, MatchTier::Context101);
    assert_eq!(match_101.target_zh, "裂隙终于彻底撕裂……");
    assert_eq!(match_101.priority, 100);

    // 测试 100% 跨文件命中 (传入其他文件 scope)
    let match_100 = match_segment("몸 상태가 약해진 것 때문이지?", Some("OtherFile.json"), &store)?
        .expect("跨文件也应当 100% 命中");
    assert_eq!(match_100.tier, MatchTier::Exact100);
    assert_eq!(match_100.target_zh, "是因为身体衰弱的原因吗？");

    // 未知文本应返回 None
    let match_none = match_segment("완전히 새로운 미지의 대사입니다", None, &store)?;
    assert!(match_none.is_none());

    Ok(())
}
