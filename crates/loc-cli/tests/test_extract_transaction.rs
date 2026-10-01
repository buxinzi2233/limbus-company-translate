use anyhow::Result;
use loc_cli::commands::run_extract;
use loc_core::Store;
use serde_json::{json, Value};
use std::{fs, path::Path};
use tempfile::tempdir;

fn write_source(path: &Path, text: &str) -> Result<()> {
    fs::write(path, serde_json::to_vec(&json!({"content":text}))?)?;
    Ok(())
}

#[test]
fn malformed_later_file_does_not_create_project_or_partial_rows() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    write_source(&kr.join("a.json"), "안녕하세요")?;
    fs::write(kr.join("z.json"), "{bad-json")?;
    let ticket = root.path().join("ticket.jsonl");
    fs::write(&ticket, "previous ticket\n")?;
    let store = Store::open(root.path().join("test.db"))?;
    assert!(run_extract(&store, Some("new".into()), &kr, &zh, Some(&ticket), true).is_err());
    assert!(store.get_latest_project()?.is_none());
    assert_eq!(store.get_project_stats("new")?.total_files, 0);
    assert_eq!(store.get_project_stats("new")?.total_segments, 0);
    assert!(store.search_segments_fts("new", "안녕", 20)?.is_empty());
    assert_eq!(fs::read_to_string(ticket)?, "previous ticket\n");
    Ok(())
}

#[test]
fn database_failure_rolls_back_existing_rows_metadata_and_full_text_index() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    write_source(&kr.join("a.json"), "안녕하세요")?;
    let database = root.path().join("test.db");
    let store = Store::open(&database)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let before = store
        .query_segments_paged("test", None, None, None, 0, 20)?
        .1
        .remove(0);
    let before_file = store.get_files("test")?.remove(0);
    let before_project = store.get_project("test")?.unwrap();
    store.raw_connection().lock().unwrap().execute_batch(
        "CREATE TRIGGER reject_last_segment BEFORE INSERT ON segments WHEN NEW.source_kr = '막기'
        BEGIN SELECT RAISE(ABORT, 'late import constraint failure'); END;",
    )?;
    write_source(&kr.join("a.json"), "새로운 원문")?;
    write_source(&kr.join("z.json"), "막기")?;
    let ticket = root.path().join("ticket.jsonl");
    fs::write(&ticket, "keep original ticket\n")?;
    let error =
        run_extract(&store, Some("test".into()), &kr, &zh, Some(&ticket), false).unwrap_err();
    assert!(format!("{error:#}").contains("late import constraint failure"));
    drop(store);
    let reopened = Store::open(&database)?;
    let after = reopened.get_segment("test", before.id)?;
    assert_eq!(after.source_kr, before.source_kr);
    assert_eq!(after.previous_kr, before.previous_kr);
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(reopened.get_files("test")?.len(), 1);
    assert_eq!(
        reopened.get_files("test")?[0].updated_at,
        before_file.updated_at
    );
    assert_eq!(
        reopened.get_project("test")?.unwrap().updated_at,
        before_project.updated_at
    );
    assert!(reopened
        .search_segments_fts("test", "새로운", 20)?
        .is_empty());
    assert_eq!(reopened.search_segments_fts("test", "안녕", 20)?.len(), 1);
    assert_eq!(fs::read_to_string(ticket)?, "keep original ticket\n");
    Ok(())
}

#[test]
fn successful_batch_returns_matching_persisted_records_and_ticket() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    write_source(&kr.join("a.json"), "안녕하세요")?;
    write_source(&kr.join("z.json"), "안녕히 가세요")?;
    let store = Store::open(root.path().join("test.db"))?;
    let ticket = root.path().join("ticket.jsonl");
    let summary = run_extract(&store, Some("test".into()), &kr, &zh, Some(&ticket), false)?;
    assert_eq!(summary.scanned_files, 2);
    assert_eq!(summary.total_segments, 2);
    assert_eq!(summary.pending_untranslated, 2);
    let rows = store
        .query_segments_paged("test", None, None, None, 0, 20)?
        .1;
    let exported: Vec<Value> = fs::read_to_string(ticket)?
        .lines()
        .map(serde_json::from_str)
        .collect::<serde_json::Result<_>>()?;
    assert_eq!(exported.len(), rows.len());
    for (item, row) in exported.iter().zip(rows.iter()) {
        assert_eq!(item["file"], row.relative_path);
        assert_eq!(item["pointer"], row.key_path);
        assert_eq!(item["source_kr"], row.source_kr);
    }
    Ok(())
}

#[test]
fn invalid_ticket_destination_is_rejected_before_database_import() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    write_source(&kr.join("a.json"), "안녕하세요")?;
    write_source(&kr.join("z.json"), "안녕히 가세요")?;
    let output_directory = root.path().join("not-a-ticket-file");
    fs::create_dir(&output_directory)?;
    fs::write(output_directory.join("keep"), "unchanged")?;
    let store = Store::open(root.path().join("test.db"))?;
    let error = run_extract(
        &store,
        Some("test".into()),
        &kr,
        &zh,
        Some(&output_directory),
        false,
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("Ticket destination must be a regular file"));
    assert!(store.get_latest_project()?.is_none());
    assert_eq!(store.get_project_stats("test")?.total_segments, 0);
    assert_eq!(
        fs::read_to_string(output_directory.join("keep"))?,
        "unchanged"
    );
    Ok(())
}

#[test]
fn ticket_cannot_replace_inputs_database_or_sqlite_journals() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    write_source(&kr.join("a.json"), "안녕하세요")?;
    let database = root.path().join("test.db");
    let store = Store::open(&database)?;
    for output in [
        database.clone(),
        root.path().join("test.db-wal"),
        kr.join("a.json"),
        zh.join("ticket.jsonl"),
    ] {
        assert!(run_extract(&store, Some("test".into()), &kr, &zh, Some(&output), false).is_err());
        assert!(store.get_latest_project()?.is_none());
    }
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(kr.join("a.json"))?)?["content"],
        "안녕하세요"
    );
    drop(store);
    assert!(Store::open(&database)?.get_latest_project()?.is_none());
    Ok(())
}

#[cfg(unix)]
#[test]
fn broken_ticket_symlink_is_not_replaced() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    write_source(&kr.join("a.json"), "안녕하세요")?;
    let ticket = root.path().join("ticket.jsonl");
    std::os::unix::fs::symlink(root.path().join("missing"), &ticket)?;
    let store = Store::open(root.path().join("test.db"))?;
    assert!(run_extract(&store, Some("test".into()), &kr, &zh, Some(&ticket), false).is_err());
    assert!(fs::symlink_metadata(ticket)?.file_type().is_symlink());
    assert!(store.get_latest_project()?.is_none());
    Ok(())
}
