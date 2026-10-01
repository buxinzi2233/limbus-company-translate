use anyhow::Result;
use loc_cli::commands::run_extract;
use loc_core::{SaveTranslation, Stage, Store};
use serde_json::{json, Value};
use std::fs;
use tempfile::tempdir;

#[test]
fn reordered_ids_keep_edits_deleted_rows_retire_and_reappear_safely() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let a = kr.join("a.json");
    let b = kr.join("b.json");
    fs::write(
        &a,
        serde_json::to_vec(
            &json!({"dataList":[{"id":1,"content":"안녕하세요"},{"id":2,"content":"감사합니다"}]}),
        )?,
    )?;
    fs::write(&b, r#"{"content":"반갑습니다"}"#)?;
    let db = root.path().join("test.db");
    let store = Store::open(&db)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let first = store
        .query_segments_paged("test", None, None, None, 0, 20)?
        .1;
    let hello = first
        .iter()
        .find(|row| row.source_kr == "안녕하세요")
        .unwrap();
    let thanks = first
        .iter()
        .find(|row| row.source_kr == "감사합니다")
        .unwrap();
    let saved = store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: hello.id,
        expected_updated_at: hello.updated_at,
        target_zh: "你好",
    })?;
    fs::write(
        &a,
        serde_json::to_vec(
            &json!({"dataList":[{"id":2,"content":"감사합니다"},{"id":1,"content":"안녕하세요"}]}),
        )?,
    )?;
    fs::remove_file(&b)?;
    let ticket = root.path().join("ticket.jsonl");
    run_extract(&store, Some("test".into()), &kr, &zh, Some(&ticket), false)?;
    let moved = store.get_segment("test", hello.id)?;
    assert_eq!(moved.key_path, "/dataList/1/content");
    assert_eq!(moved.target_zh.as_deref(), Some("你好"));
    assert_eq!(moved.stage, Stage::Reviewed);
    assert!(moved.updated_at > saved.updated_at);
    assert!(store
        .save_reviewed_translation(&SaveTranslation {
            project_id: "test",
            id: hello.id,
            expected_updated_at: saved.updated_at,
            target_zh: "旧草稿"
        })
        .is_err());
    assert_eq!(store.get_project_stats("test")?.total_files, 1);
    assert_eq!(store.get_project_stats("test")?.total_segments, 2);
    assert!(store
        .search_segments_fts("test", "반갑습니다", 10)?
        .is_empty());
    assert!(store.search_segments_fts("test", "반갑", 10)?.is_empty());
    fs::write(&a, r#"{"dataList":[{"id":2,"content":"감사합니다"}]}"#)?;
    run_extract(&store, Some("test".into()), &kr, &zh, Some(&ticket), false)?;
    assert!(store.get_segment("test", hello.id).is_err());
    assert_eq!(store.get_project_stats("test")?.total_segments, 1);
    assert_eq!(
        store.get_segment("test", thanks.id)?.key_path,
        "/dataList/0/content"
    );
    assert!(!fs::read_to_string(&ticket)?.contains("안녕하세요"));
    let historical: String = store.raw_connection().lock().unwrap().query_row(
        "SELECT target_zh FROM segments WHERE id = ?1 AND active = 0",
        [hello.id],
        |row| row.get(0),
    )?;
    assert_eq!(historical, "你好");
    fs::write(&a, r#"{"dataList":[{"id":1,"content":"새로운 인사"}]}"#)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let revived = store.get_segment("test", hello.id)?;
    assert_eq!(revived.target_zh.as_deref(), Some("你好"));
    assert_eq!(revived.previous_kr.as_deref(), Some("안녕하세요"));
    assert_eq!(revived.stage, Stage::OutdatedDiff);
    assert!(!revived.qa_passed);
    fs::write(&a, "{}")?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    assert_eq!(store.get_project_stats("test")?.total_segments, 0);
    drop(store);
    assert_eq!(
        Store::open(&db)?.get_project_stats("test")?.total_segments,
        0
    );
    Ok(())
}

#[test]
fn extraction_aligns_translated_arrays_by_nested_typed_ids_and_rejects_duplicates() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let source = json!({"dataList":[{"id":1,"texts":[{"index":0,"text":"안녕하세요"},{"index":1,"text":"감사합니다"}]}]});
    fs::write(kr.join("a.json"), serde_json::to_vec(&source)?)?;
    fs::write(
        zh.join("a.json"),
        r#"{"dataList":[{"id":1,"texts":[{"index":1,"text":"谢谢"},{"index":0,"text":"你好"}]}]}"#,
    )?;
    let store = Store::open_in_memory()?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let rows = store
        .query_segments_paged("test", None, None, None, 0, 20)?
        .1;
    assert_eq!(
        rows.iter()
            .find(|row| row.source_kr == "안녕하세요")
            .unwrap()
            .target_zh
            .as_deref(),
        Some("你好")
    );
    let mut duplicate: Value = source.clone();
    duplicate["dataList"][0]["texts"][1]["index"] = json!(0);
    fs::write(kr.join("a.json"), serde_json::to_vec(&duplicate)?)?;
    let error = run_extract(&store, Some("test".into()), &kr, &zh, None, false).unwrap_err();
    assert!(format!("{error:#}").contains("Cannot align snapshot-bound"));
    assert_eq!(store.get_project_stats("test")?.total_segments, 2);
    assert_eq!(
        store.get_segment("test", rows[0].id)?.updated_at,
        rows[0].updated_at
    );
    Ok(())
}

#[test]
fn legacy_database_import_requires_array_review_and_locked_changes_roll_back() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let path = kr.join("a.json");
    fs::write(&path, r#"{"items":[{"id":1,"text":"안녕하세요"}]}"#)?;
    let database = root.path().join("legacy.db");
    let store = Store::open(&database)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let segment = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: segment.id,
        expected_updated_at: segment.updated_at,
        target_zh: "你好",
    })?;
    // Reproduce a database written before stable identities were introduced.
    store.raw_connection().lock().unwrap().execute_batch(
        "DROP INDEX idx_segment_identity;
        ALTER TABLE files DROP COLUMN active;
        ALTER TABLE segments DROP COLUMN active;
        ALTER TABLE segments DROP COLUMN source_identity;
        ALTER TABLE segments DROP COLUMN retired_key_path;",
    )?;
    drop(store);
    let store = Store::open(&database)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let migrated = store.get_segment("test", segment.id)?;
    assert_eq!(migrated.stage, Stage::OutdatedDiff);
    assert!(!migrated.qa_passed);
    assert_eq!(migrated.target_zh.as_deref(), Some("你好"));
    store.raw_connection().lock().unwrap().execute(
        "UPDATE files SET is_locked = 1 WHERE id = ?1",
        [segment.file_id],
    )?;
    fs::write(&path, r#"{"items":[{"id":1,"text":"새로운 인사"}]}"#)?;
    assert!(run_extract(&store, Some("test".into()), &kr, &zh, None, false).is_err());
    assert_eq!(
        store.get_segment("test", segment.id)?.source_kr,
        "안녕하세요"
    );
    assert_eq!(store.get_project_stats("test")?.total_segments, 1);
    Ok(())
}
