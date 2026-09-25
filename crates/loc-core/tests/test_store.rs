use anyhow::Result;
use loc_core::{FileRecord, Project, Segment, Stage, Store, TMEntry};

#[test]
fn test_store_lifecycle_and_fts5_search() -> Result<()> {
    let store = Store::open_in_memory()?;

    // 1. 项目创建
    let project = Project::new("s8-test", "第8赛季测试项目");
    store.upsert_project(&project)?;
    let p = store.get_project("s8-test")?.expect("项目应当存在");
    assert_eq!(p.name, "第8赛季测试项目");

    // 2. 文件创建
    let file = FileRecord::new("s8-test", "StoryData/S1029B.json", "Story");
    let file_id = store.upsert_file(&file)?;
    assert!(file_id > 0);

    // 3. 词条批量存入
    let mut seg1 = Segment::new(
        file_id,
        "s8-test",
        "/dataList/0/content",
        "균열이 마침내 완전히 찢어지고...",
    );
    seg1.target_zh = Some("裂隙终于彻底撕裂……".to_string());
    seg1.stage = Stage::Reviewed;

    let mut seg2 = Segment::new(
        file_id,
        "s8-test",
        "/dataList/1/content",
        "몸 상태가 처음 계약했을 때처럼 약해진 것 때문이지?",
    );
    seg2.speaker = Some("단테".to_string());
    seg2.target_zh = Some("<是因为身体能力衰退得宛如最初签订契约时那般，对吧？>".to_string());
    seg2.stage = Stage::TmExact;

    let inserted = store.batch_upsert_segments(&[seg1, seg2])?;
    assert_eq!(inserted, 2);

    // 4. FTS5 全文搜索检验 (短词 + 长句双重检索)
    // 4.1 搜短词 "契约" (2字符)
    let zh_short = store.search_segments_fts("契约", 10)?;
    assert_eq!(zh_short.len(), 1);
    assert_eq!(zh_short[0].relative_path, "StoryData/S1029B.json");
    assert_eq!(zh_short[0].speaker.as_deref(), Some("단테"));

    // 4.2 搜长句 "身体能力衰退" (6字符，触发 FTS5 trigram 索引)
    let zh_long = store.search_segments_fts("身体能力衰退", 10)?;
    assert_eq!(zh_long.len(), 1);
    assert_eq!(zh_long[0].key_path, "/dataList/1/content");

    // 4.3 搜韩文短词 "균열" (2字符)
    let kr_short = store.search_segments_fts("균열", 10)?;
    assert_eq!(kr_short.len(), 1);
    assert_eq!(kr_short[0].target_zh.as_deref(), Some("裂隙终于彻底撕裂……"));

    // 4.4 搜韩文长句 "완전히 찢어지고" (8字符，触发 FTS5 trigram 索引)
    let kr_long = store.search_segments_fts("완전히 찢어지고", 10)?;
    assert_eq!(kr_long.len(), 1);
    assert_eq!(kr_long[0].key_path, "/dataList/0/content");

    // 5. TM 记忆库检验
    let tm_entry = TMEntry::new(
        "hash_123",
        "몸 상태가 처음 계약했을 때처럼...",
        "<是因为身体能力衰退得宛如最初签订契约时那般，对吧？>",
        "upstream_llc_gold",
        100,
    );
    store.upsert_tm_entry(&tm_entry)?;
    let queried = store.query_tm_exact("hash_123")?.expect("应当命中TM");
    assert_eq!(queried.priority, 100);
    assert_eq!(queried.source_origin, "upstream_llc_gold");

    Ok(())
}
