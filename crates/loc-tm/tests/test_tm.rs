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

    // Same-file matching does not imply pointer-level context.
    let match_101 = match_segment(
        "균열이 마침내 완전히 찢어지고...",
        Some("StoryData/S1029.json"),
        &store,
    )?
    .expect("应当命中 TM");
    assert_eq!(match_101.tier, MatchTier::FileExact);
    assert_eq!(match_101.target_zh, "裂隙终于彻底撕裂……");
    assert_eq!(match_101.priority, 100);

    // 测试 100% 跨文件命中 (传入其他文件 scope)
    let match_100 = match_segment(
        "몸 상태가 약해진 것 때문이지?",
        Some("OtherFile.json"),
        &store,
    )?
    .expect("跨文件也应当 100% 命中");
    assert_eq!(match_100.tier, MatchTier::Exact100);
    assert_eq!(match_100.target_zh, "是因为身体衰弱的原因吗？");

    // 未知文本应返回 None
    let match_none = match_segment("완전히 새로운 미지의 대사입니다", None, &store)?;
    assert!(match_none.is_none());

    Ok(())
}

#[test]
fn repeated_index_is_idempotent_and_invalid_corpus_never_partially_commits() -> Result<()> {
    let dir = tempdir()?;
    let store = Store::open(dir.path().join("tm.db"))?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(kr.join("a.json"), r#"{"content":"안녕하세요"}"#)?;
    fs::write(zh.join("a.json"), r#"{"content":"你好"}"#)?;
    assert_eq!(
        index_parallel_corpora(&kr, &zh, "test", 100, &store)?.inserted_entries,
        1
    );
    assert_eq!(
        index_parallel_corpora(&kr, &zh, "test", 100, &store)?.inserted_entries,
        0
    );
    assert_eq!(store.get_tm_count()?, 1);
    assert_eq!(store.search_tm_fts("你好", 20)?.len(), 1);
    fs::write(zh.join("a.json"), r#"{"content":"您好"}"#)?;
    fs::write(kr.join("z.json"), r#"{"content":"안녕히 가세요"}"#)?;
    fs::write(zh.join("z.json"), "invalid json")?;
    assert!(index_parallel_corpora(&kr, &zh, "test", 100, &store).is_err());
    assert_eq!(store.get_tm_count()?, 1);
    assert!(store.search_tm_fts("您好", 20)?.is_empty());
    Ok(())
}

#[test]
fn hash_collisions_file_context_and_conflicting_candidates_are_distinguished() -> Result<()> {
    use loc_core::TMEntry;
    use loc_tm::hash_text;
    let store = Store::open_in_memory()?;
    let source = "안녕하세요";
    let collision = TMEntry::new(hash_text(source), "다른 원문", "其他原文", "test", 999);
    store.upsert_tm_entry(&collision)?;
    assert!(match_segment(source, Some("a.json"), &store)?.is_none());
    let mut unrelated = TMEntry::new(hash_text(source), source, "其他文件译文", "test", 999);
    unrelated.file_scope = Some("b.json".into());
    store.upsert_tm_entry(&unrelated)?;
    let mut local = TMEntry::new(hash_text(source), source, "你好", "test", 1);
    local.file_scope = Some("a.json".into());
    let id = store.upsert_tm_entry(&local)?;
    assert_eq!(store.upsert_tm_entry(&local)?, id);
    let matched = match_segment(source, Some("a.json"), &store)?.unwrap();
    assert_eq!(matched.target_zh, "你好");
    assert_eq!(matched.tier, MatchTier::FileExact);
    local.target_zh = "您好".into();
    store.upsert_tm_entry(&local)?;
    assert_eq!(
        match_segment(source, Some("a.json"), &store)?.unwrap().tier,
        MatchTier::Ambiguous
    );
    assert_eq!(
        match_segment(source, None, &store)?.unwrap().tier,
        MatchTier::Ambiguous
    );
    Ok(())
}

#[test]
fn sqlite_failure_rolls_back_every_entry_in_the_batch() -> Result<()> {
    use loc_core::TMEntry;
    let store = Store::open_in_memory()?;
    store.raw_connection().lock().unwrap().execute_batch(
        "CREATE TRIGGER reject_bad_tm BEFORE INSERT ON tm_entries WHEN NEW.target_zh = 'reject'
        BEGIN SELECT RAISE(ABORT, 'test constraint violation'); END;",
    )?;
    let entries = [
        TMEntry::new("a", "안녕하세요", "你好", "test", 1),
        TMEntry::new("b", "안녕히 가세요", "reject", "test", 1),
    ];
    assert!(store.upsert_tm_entries_batch(&entries).is_err());
    assert_eq!(store.get_tm_count()?, 0);
    assert!(store.search_tm_fts("你好", 20)?.is_empty());
    Ok(())
}

#[test]
fn corpus_alignment_follows_ids_not_array_positions_and_rejects_ambiguous_ids() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(
        kr.join("a.json"),
        r#"{"dataList":[{"id":1,"text":"안녕하세요"},{"id":2,"text":"감사합니다"}]}"#,
    )?;
    fs::write(
        zh.join("a.json"),
        r#"{"dataList":[{"id":2,"text":"谢谢"},{"id":1,"text":"你好"}]}"#,
    )?;
    let store = Store::open_in_memory()?;
    assert_eq!(
        index_parallel_corpora(&kr, &zh, "test", 100, &store)?.inserted_entries,
        2
    );
    assert_eq!(
        match_segment("안녕하세요", Some("a.json"), &store)?
            .unwrap()
            .target_zh,
        "你好"
    );
    assert_eq!(
        match_segment("감사합니다", Some("a.json"), &store)?
            .unwrap()
            .target_zh,
        "谢谢"
    );
    assert_eq!(
        index_parallel_corpora(&kr, &zh, "test", 100, &store)?.inserted_entries,
        0
    );
    fs::write(
        zh.join("a.json"),
        r#"{"dataList":[{"id":1,"text":"谢谢"},{"id":1,"text":"你好"}]}"#,
    )?;
    assert!(index_parallel_corpora(&kr, &zh, "test", 100, &store).is_err());
    assert_eq!(store.get_tm_count()?, 2);
    Ok(())
}
