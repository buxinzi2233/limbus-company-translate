use anyhow::Result;
use loc_cli::commands::{run_extract, run_inject};
use loc_core::{SaveTranslation, Stage, Store};
use serde_json::{json, Value};
use std::fs;
use tempfile::tempdir;

#[test]
fn bom_sources_and_references_work_across_extract_index_and_inject() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let raw = "\u{feff}{\"dataList\":[{\"id\":1,\"text\":\"안녕하세요\"}]}";
    fs::write(kr.join("a.json"), raw)?;
    fs::write(
        zh.join("a.json"),
        "\u{feff}{\"dataList\":[{\"id\":1,\"text\":\"你好\"}]}",
    )?;
    let store = Store::open(root.path().join("project.db"))?;
    let extracted = run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    assert_eq!(extracted.total_segments, 1);
    let row = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    assert_eq!(row.target_zh.as_deref(), Some("你好"));
    let indexed = loc_tm::index_parallel_corpora(&kr, &zh, "bom-fixture", 1, &store)?;
    assert_eq!(indexed.aligned_entries, 1);
    store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: row.id,
        expected_updated_at: row.updated_at,
        target_zh: "您好",
    })?;
    let injected = run_inject(&store, &zh, Some(&kr), None, true, false)?;
    assert_eq!(injected.injected_segments, 1);
    assert_eq!(fs::read_to_string(kr.join("a.json"))?, raw);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(zh.join("a.json"))?)?["dataList"][0]["text"],
        "您好"
    );
    Ok(())
}

#[test]
fn ambiguous_arrays_preserve_all_records_but_never_reuse_edits_after_reordering() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let path = kr.join("a.json");
    let source = json!({"items":[{"id":200,"text":"안녕하세요","voice":"a"},{"id":200,"text":"감사합니다"},{"text":"반갑습니다"}]});
    fs::write(&path, serde_json::to_vec(&source)?)?;
    let store = Store::open(root.path().join("project.db"))?;
    let corpus_kr = root.path().join("corpus-kr");
    let corpus_zh = root.path().join("corpus-zh");
    fs::create_dir(&corpus_kr)?;
    fs::create_dir(&corpus_zh)?;
    fs::write(corpus_kr.join("a.json"), r#"{"text":"안녕하세요"}"#)?;
    fs::write(corpus_zh.join("a.json"), r#"{"text":"你好"}"#)?;
    assert_eq!(
        loc_tm::index_parallel_corpora(&corpus_kr, &corpus_zh, "verified-fixture", 1, &store)?
            .aligned_entries,
        1
    );
    let first = run_extract(&store, Some("test".into()), &kr, &zh, None, true)?;
    assert_eq!(first.pending_untranslated, 3);
    assert_eq!(first.tm_exact_filled, 0);
    assert_eq!(first.total_segments, 3);
    assert_eq!(first.snapshot_bound_segments, 3);
    let rows = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1;
    let hello = rows
        .iter()
        .find(|row| row.source_kr == "안녕하세요")
        .unwrap();
    store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: hello.id,
        expected_updated_at: hello.updated_at,
        target_zh: "你好",
    })?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, true)?;
    assert_eq!(
        store.get_segment("test", hello.id)?.target_zh.as_deref(),
        Some("你好")
    );
    let reordered = json!({"items":[source["items"][1], source["items"][0], source["items"][2]]});
    fs::write(&path, serde_json::to_vec(&reordered)?)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, true)?;
    assert!(store.get_segment("test", hello.id).is_err());
    let current = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1;
    assert_eq!(current.len(), 3);
    assert!(current
        .iter()
        .all(|row| row.target_zh.is_none() && row.stage == Stage::Untranslated));
    let historical: String = store.raw_connection().lock().unwrap().query_row(
        "SELECT target_zh FROM segments WHERE id = ?1 AND active = 0",
        [hello.id],
        |row| row.get(0),
    )?;
    assert_eq!(historical, "你好");
    Ok(())
}

#[test]
fn ambiguous_bilingual_alignment_and_invalid_ids_fail_without_partial_import() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(
        kr.join("a.json"),
        r#"{"items":[{"id":1,"text":"안녕하세요"},{"id":1,"text":"감사합니다"}]}"#,
    )?;
    fs::write(
        zh.join("a.json"),
        r#"{"items":[{"id":1,"text":"你好"},{"id":1,"text":"谢谢"}]}"#,
    )?;
    let store = Store::open(root.path().join("project.db"))?;
    assert!(format!(
        "{:#}",
        run_extract(&store, Some("test".into()), &kr, &zh, None, false).unwrap_err()
    )
    .contains("Cannot align snapshot-bound"));
    assert!(format!(
        "{:#}",
        loc_tm::index_parallel_corpora(&kr, &zh, "unsafe", 1, &store).unwrap_err()
    )
    .contains("Cannot index snapshot-bound"));
    assert_eq!(store.get_project_stats("test")?.total_segments, 0);
    let tm_count: i64 = store.raw_connection().lock().unwrap().query_row(
        "SELECT COUNT(*) FROM tm_entries",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(tm_count, 0);
    fs::remove_file(zh.join("a.json"))?;
    fs::write(
        kr.join("a.json"),
        r#"{"items":[{"id":null,"text":"안녕하세요"}]}"#,
    )?;
    assert!(format!(
        "{:#}",
        run_extract(&store, Some("test".into()), &kr, &zh, None, false).unwrap_err()
    )
    .contains("Invalid id"));
    assert_eq!(store.get_project_stats("test")?.total_segments, 0);
    Ok(())
}

#[test]
fn snapshot_injection_requires_current_database_identity_and_matching_output() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let source =
        json!({"items":[{"id":1,"text":"안녕하세요","voice":"a"},{"id":1,"text":"감사합니다"}]});
    fs::write(kr.join("a.json"), serde_json::to_vec(&source)?)?;
    let store = Store::open(root.path().join("project.db"))?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let row = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: row.id,
        expected_updated_at: row.updated_at,
        target_zh: "你好",
    })?;
    let ticket = root.path().join("ticket.jsonl");
    fs::write(
        &ticket,
        serde_json::to_vec(
            &json!({"file":"a.json","pointer":row.key_path,"source_kr":row.source_kr,"target_zh":"你好"}),
        )?,
    )?;
    assert!(format!(
        "{:#}",
        run_inject(&store, &zh, Some(&kr), Some(&ticket), true, false).unwrap_err()
    )
    .contains("Snapshot-bound source identity"));
    let changed =
        json!({"items":[{"id":1,"text":"안녕하세요","voice":"b"},{"id":1,"text":"감사합니다"}]});
    fs::write(kr.join("a.json"), serde_json::to_vec(&changed)?)?;
    assert!(format!(
        "{:#}",
        run_inject(&store, &zh, Some(&kr), None, true, false).unwrap_err()
    )
    .contains("Snapshot-bound source identity"));
    assert!(!zh.join("a.json").exists());
    fs::write(kr.join("a.json"), serde_json::to_vec(&source)?)?;
    assert_eq!(
        run_inject(&store, &zh, Some(&kr), None, true, false)?.injected_segments,
        1
    );
    let before = fs::read(zh.join("a.json"))?;
    assert!(run_inject(&store, &zh, Some(&kr), None, true, false).is_err());
    assert_eq!(fs::read(zh.join("a.json"))?, before);
    Ok(())
}

#[test]
fn frozen_snapshot_records_cannot_be_retired_by_changing_array_identity() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let source = json!({"items":[{"id":1,"text":"안녕하세요"},{"id":1,"text":"감사합니다"}]});
    fs::write(kr.join("a.json"), serde_json::to_vec(&source)?)?;
    let store = Store::open(root.path().join("project.db"))?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let row = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute("UPDATE segments SET stage = 9 WHERE id = ?1", [row.id])?;
    fs::write(
        kr.join("a.json"),
        serde_json::to_vec(&json!({"items":[source["items"][1],source["items"][0]]}))?,
    )?;
    let error = run_extract(&store, Some("test".into()), &kr, &zh, None, false).unwrap_err();
    assert!(format!("{error:#}").contains("Cannot retire frozen segment"));
    assert_eq!(store.get_segment("test", row.id)?.key_path, row.key_path);
    assert_eq!(store.get_project_stats("test")?.total_segments, 2);
    Ok(())
}

#[test]
fn legacy_fts_rowids_migrate_without_changing_translations_or_lifecycle() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(
        kr.join("a.json"),
        r#"{"items":[{"id":1,"text":"안녕하세요"}]}"#,
    )?;
    let database = root.path().join("project.db");
    let store = Store::open(&database)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let row = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    let saved = store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: row.id,
        expected_updated_at: row.updated_at,
        target_zh: "你好",
    })?;
    // Reproduce a legacy derived index whose rowids drifted after text updates.
    store.raw_connection().lock().unwrap().execute_batch(
        "PRAGMA user_version = 0;
        UPDATE segments_fts SET rowid = rowid + 1000;",
    )?;
    drop(store);
    let store = Store::open(&database)?;
    let restored = store.get_segment("test", row.id)?;
    assert_eq!(restored.updated_at, saved.updated_at);
    assert_eq!(restored.target_zh.as_deref(), Some("你好"));
    assert!(restored.review_confirmed);
    assert_eq!(
        store.search_segments_fts("test", "안녕하세요", 10)?.len(),
        1
    );
    let mismatched: i64 = store.raw_connection().lock().unwrap().query_row(
        "SELECT COUNT(*) FROM segments_fts WHERE rowid != segment_id",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(mismatched, 0);
    store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: restored.id,
        expected_updated_at: restored.updated_at,
        target_zh: "您好",
    })?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    assert_eq!(
        store.search_segments_fts("test", "안녕하세요", 10)?.len(),
        1
    );
    fs::write(kr.join("a.json"), "{}")?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    assert!(store
        .search_segments_fts("test", "안녕하세요", 10)?
        .is_empty());
    drop(store);
    assert_eq!(
        Store::open(&database)?
            .get_project_stats("test")?
            .total_segments,
        0
    );
    Ok(())
}

#[test]
fn display_fields_include_non_hangul_and_speakers_without_importing_metadata() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let source = json!({"dataList":[{"id":1,"content":"EXP","teller":"돈키호테","speaker":"???",
        "name":"FOMO","desc":"{0} %{1} -{2}/%","model":"이상","usage":"(800101, VERY_HIGH)",
        "codeName":"L-04-14-53","debugNodeId ":"DN-10503","undefined":"-","keywords":["Vulnerable"],
        "text":"呼吸法 ","result":["…!",""],"iconID":"A"}]});
    fs::write(kr.join("a.json"), serde_json::to_vec(&source)?)?;
    let store = Store::open(root.path().join("project.db"))?;
    let summary = run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    assert_eq!(summary.total_segments, 7);
    let before = store
        .query_segments_paged("test", None, None, None, 0, 20)?
        .1;
    assert!(before.iter().any(|row| row.source_kr == "EXP"));
    assert!(before.iter().any(|row| row.source_kr == "呼吸法 "));
    let mut unknown = source;
    unknown["dataList"][0]["newDisplayField"] = json!("new content");
    fs::write(kr.join("a.json"), serde_json::to_vec(&unknown)?)?;
    let error = run_extract(&store, Some("test".into()), &kr, &zh, None, false).unwrap_err();
    assert!(format!("{error:#}").contains("/dataList/0/newDisplayField"));
    let after = store
        .query_segments_paged("test", None, None, None, 0, 20)?
        .1;
    assert_eq!(
        before.iter().map(|row| row.id).collect::<Vec<_>>(),
        after.iter().map(|row| row.id).collect::<Vec<_>>()
    );
    Ok(())
}
