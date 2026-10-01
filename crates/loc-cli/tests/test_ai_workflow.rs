#[path = "../../loc-core/tests/support/draft_grants.rs"]
mod draft_grants;
use anyhow::Result;
use loc_cli::commands::{run_extract, run_inject};
use loc_core::{SaveTranslation, Stage, Store, StoreError};
use loc_qa::{submit_ai_draft, DraftSubmission};
use serde_json::json;
use std::{fs, process::Command};
use tempfile::tempdir;

#[test]
fn real_cli_draft_requires_review_before_injection_and_logs_each_write() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    let data = root.path().join("data");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::create_dir(&data)?;
    fs::write(kr.join("a.json"), r#"{"text":"체력 {0} 20%"}"#)?;
    let store = Store::open(data.join("project.db"))?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let row = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    let permit = draft_grants::grant(&store, "test", vec![row.id])?;
    let permit_path = root.path().join("permit.json");
    fs::write(&permit_path, serde_json::to_vec(&permit)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&permit_path, fs::Permissions::from_mode(0o600))?;
    }
    let input = root.path().join("draft.json");
    fs::write(
        &input,
        serde_json::to_vec(&json!({"project_id":"test","id":row.id,
        "expected_updated_at":row.updated_at,"target_zh":"生命{0}20%","producer":"test-agent", "stage":4}))?,
    )?;
    let result = Command::new(env!("CARGO_BIN_EXE_lct"))
        .arg("--data-dir")
        .arg(&data)
        .arg("--json")
        .arg("draft")
        .arg("--draft-permit")
        .arg(&permit_path)
        .arg("--input")
        .arg(&input)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let draft = store.get_segment("test", row.id)?;
    assert_eq!(draft.stage, Stage::AiDrafted);
    assert!(draft.qa_passed);
    assert_eq!(store.get_project_stats("test")?.reviewed_segments, 0);
    assert_eq!(
        store
            .query_segments_by_stages("test", None, &[1, 2, 3, 5], None, 0, 100)?
            .0,
        1
    );
    let events = store.translation_events("test", None, 20)?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "ai_draft");
    assert!(
        events[0].producer.starts_with("grant:") && events[0].producer.ends_with("/test-agent")
    );
    assert_eq!(
        run_inject(&store, &zh, Some(&kr), None, true, false)?.modified_files,
        0
    );
    assert!(!zh.join("a.json").exists());
    store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: row.id,
        expected_updated_at: draft.updated_at,
        target_zh: "生命{0}20%",
    })?;
    let reviewed = store.get_segment("test", row.id)?;
    assert_eq!(
        store
            .query_segments_by_stages("test", None, &[1, 2, 3, 5], None, 0, 100)?
            .0,
        0
    );
    assert_eq!(
        store.translation_events("test", None, 20)?[0].kind,
        "review"
    );
    let injected = run_inject(&store, &zh, Some(&kr), None, true, false)?;
    assert_eq!(injected.injected_segments, 1);
    assert_eq!(store.file_operations("test", None, 20)?[0].kind, "inject");
    let restored = Command::new(env!("CARGO_BIN_EXE_lct"))
        .arg("--data-dir")
        .arg(&data)
        .arg("restore")
        .arg("--workspace")
        .arg(&zh)
        .arg("--backup")
        .arg(injected.backup_directory.unwrap())
        .output()?;
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(store.file_operations("test", None, 20)?[0].kind, "restore");
    assert!(!zh.join("a.json").exists());
    assert!(store.file_operations("foreign", None, 20)?.is_empty());
    let error = submit_ai_draft(
        &store,
        &DraftSubmission {
            project_id: "test".into(),
            id: row.id,
            expected_updated_at: reviewed.updated_at,
            target_zh: "体力{0}20%".into(),
            producer: "test-agent".into(),
        },
        Some(&permit),
    )
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StoreError>(),
        Some(StoreError::Reviewed(_))
    ));
    assert_eq!(store.translation_events("test", None, 20)?.len(), 2);
    assert_eq!(
        store.get_segment("test", row.id)?.target_zh,
        reviewed.target_zh
    );
    let imports = store.source_imports("test", None, 20)?;
    assert_eq!(imports[0].source_directory, kr.to_str().unwrap());
    assert_eq!(imports[0].added_or_reactivated, 1);
    Ok(())
}

#[test]
fn qa_conflicts_locks_and_audit_failures_do_not_partially_save_drafts() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(kr.join("a.json"), r#"{"text":"체력 {0} 20%"}"#)?;
    let store = Store::open_in_memory()?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let row = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    let permit = draft_grants::grant(&store, "test", vec![row.id])?;
    let valid = DraftSubmission {
        project_id: "test".into(),
        id: row.id,
        expected_updated_at: row.updated_at,
        target_zh: "生命{0}20%".into(),
        producer: "test-agent".into(),
    };
    assert!(submit_ai_draft(
        &store,
        &DraftSubmission {
            target_zh: "生命20%".into(),
            ..valid.clone()
        },
        Some(&permit),
    )
    .is_err());
    assert!(submit_ai_draft(
        &store,
        &DraftSubmission {
            project_id: "foreign".into(),
            ..valid.clone()
        },
        Some(&permit),
    )
    .is_err());
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute("UPDATE files SET is_locked = 1", [])?;
    assert!(submit_ai_draft(&store, &valid, Some(&permit)).is_err());
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute("UPDATE files SET is_locked = 0", [])?;
    store.raw_connection().lock().unwrap().execute_batch("CREATE TRIGGER reject_audit BEFORE INSERT ON translation_events BEGIN SELECT RAISE(ABORT,'audit unavailable'); END;")?;
    assert!(submit_ai_draft(&store, &valid, Some(&permit)).is_err());
    assert_eq!(
        store.get_segment("test", row.id)?.updated_at,
        row.updated_at
    );
    assert_eq!(store.get_segment("test", row.id)?.target_zh, None);
    assert!(store.translation_events("test", None, 20)?.is_empty());
    assert!(store.search_segments_fts("test", "生命", 10)?.is_empty());
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_audit;")?;
    submit_ai_draft(&store, &valid, Some(&permit))?;
    assert!(submit_ai_draft(&store, &valid, Some(&permit)).is_err());
    assert!(store.translation_events("foreign", None, 20)?.is_empty());
    assert!(store.translation_events("test", None, 101).is_err());
    Ok(())
}

#[test]
fn imported_chinese_requires_review_and_source_history_is_atomic() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(
        kr.join("a.json"),
        r#"{"text":"안녕하세요","desc":"감사합니다"}"#,
    )?;
    fs::write(zh.join("a.json"), r#"{"text":"你好","desc":"谢谢"}"#)?;
    let store = Store::open_in_memory()?;
    let summary = run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    assert_eq!(summary.needs_review, 2);
    assert_eq!(summary.already_translated, 0);
    assert!(store
        .query_segments_paged("test", None, None, None, 0, 20)?
        .1
        .iter()
        .all(|row| row.stage == Stage::Imported));
    fs::write(kr.join("a.json"), r#"{"text":"새로운 인사"}"#)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let imports = store.source_imports("test", None, 20)?;
    assert_eq!(imports[0].changed, 1);
    assert_eq!(imports[0].retired, 1);
    assert_eq!(imports.len(), 2);
    assert_eq!(
        store.source_imports("test", Some(imports[0].id), 20)?[0].id,
        imports[1].id
    );
    store.raw_connection().lock().unwrap().execute_batch("CREATE TRIGGER reject_import_audit BEFORE INSERT ON source_imports BEGIN SELECT RAISE(ABORT,'audit unavailable'); END;")?;
    fs::write(kr.join("a.json"), "{}")?;
    assert!(run_extract(&store, Some("test".into()), &kr, &zh, None, false).is_err());
    assert_eq!(store.get_project_stats("test")?.total_segments, 1);
    assert_eq!(store.source_imports("test", None, 20)?.len(), 2);
    Ok(())
}

#[test]
fn explicit_injection_selects_the_requested_project_and_reports_receipt_failure() -> Result<()> {
    use loc_cli::commands::inject::run_scoped_inject;
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(kr.join("a.json"), r#"{"text":"안녕하세요"}"#)?;
    let store = Store::open_in_memory()?;
    for (id, target) in [("older", "项目甲"), ("newer", "项目乙")] {
        run_extract(&store, Some(id.into()), &kr, &zh, None, false)?;
        let item = store
            .query_segments_paged(id, None, None, None, 0, 10)?
            .1
            .remove(0);
        store.save_reviewed_translation(&SaveTranslation {
            project_id: id,
            id: item.id,
            expected_updated_at: item.updated_at,
            target_zh: target,
        })?;
    }
    assert!(run_scoped_inject(&store, &zh, Some(&kr), None, true, false, Some("missing")).is_err());
    assert!(!zh.join("a.json").exists());
    run_scoped_inject(&store, &zh, Some(&kr), None, true, false, Some("older"))?;
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(zh.join("a.json"))?)?["text"],
        "项目甲"
    );
    assert!(store.file_operations("newer", None, 20)?.is_empty());
    store.raw_connection().lock().unwrap().execute_batch("CREATE TRIGGER reject_receipt BEFORE INSERT ON file_operations BEGIN SELECT RAISE(ABORT,'receipt unavailable'); END;")?;
    let error =
        run_scoped_inject(&store, &zh, Some(&kr), None, true, false, Some("newer")).unwrap_err();
    assert!(format!("{error:#}").contains("Files were injected successfully"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(zh.join("a.json"))?)?["text"],
        "项目乙"
    );
    assert_eq!(store.file_operations("older", None, 20)?.len(), 1);
    Ok(())
}

#[test]
fn legacy_reviews_require_confirmation_and_source_changes_invalidate_it() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(kr.join("a.json"), r#"{"text":"안녕하세요"}"#)?;
    let store = Store::open_in_memory()?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let id = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1[0]
        .id;
    store.raw_connection().lock().unwrap().execute_batch(
        "UPDATE segments SET stage=4,qa_passed=1,target_zh='你好';
         DROP TRIGGER require_draft_write_guard; ALTER TABLE segments DROP COLUMN review_confirmed;",
    )?;
    loc_core::db::schema::migrate(&store.raw_connection().lock().unwrap())?;
    assert!(!store.get_segment("test", id)?.review_confirmed);
    assert_eq!(store.review_queue("test")?.0, 1);
    assert_eq!(store.get_project_stats("test")?.reviewed_segments, 0);
    assert_eq!(
        run_inject(&store, &zh, Some(&kr), None, true, false)?.modified_files,
        0
    );
    for query in ["你好", "안녕하세요"] {
        assert!(!store.search_segments_fts("test", query, 10)?[0].review_confirmed);
    }
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute("UPDATE files SET is_locked=1", [])?;
    assert_eq!(store.review_queue("test")?.0, 0);
    assert_eq!(store.get_project_stats("test")?.pending_review_segments, 0);
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute("UPDATE files SET is_locked=0", [])?;
    let row = store.get_segment("test", id)?;
    let saved = store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id,
        expected_updated_at: row.updated_at,
        target_zh: "你好",
    })?;
    assert!(saved.review_confirmed);
    assert_eq!(store.review_queue("test")?.0, 0);
    assert_eq!(
        run_inject(&store, &zh, Some(&kr), None, true, false)?.modified_files,
        1
    );
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER require_draft_write_guard; ALTER TABLE segments DROP COLUMN review_confirmed;")?;
    loc_core::db::schema::migrate(&store.raw_connection().lock().unwrap())?;
    assert!(store.get_segment("test", id)?.review_confirmed);
    fs::write(kr.join("a.json"), r#"{"text":"반갑습니다"}"#)?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    assert!(!store.get_segment("test", id)?.review_confirmed);
    assert_eq!(store.review_queue("test")?.0, 1);
    assert_eq!(
        run_inject(&store, &zh, Some(&kr), None, true, false)?.modified_files,
        0
    );
    Ok(())
}
