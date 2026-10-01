use anyhow::Result;
use loc_cli::commands::{run_extract, run_inject};
use loc_core::{SaveTranslation, Stage, Store};
use serde_json::{json, Value};
use std::{fs, path::Path};
use tempfile::tempdir;

fn write_ticket(path: &Path, entries: &[Value]) -> Result<()> {
    fs::write(
        path,
        entries
            .iter()
            .map(serde_json::to_string)
            .collect::<serde_json::Result<Vec<_>>>()?
            .join("\n"),
    )?;
    Ok(())
}

#[test]
fn injection_preflights_all_files_and_preserves_originals_on_failure() -> Result<()> {
    let dir = tempdir()?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    for file in ["a.json", "z.json"] {
        fs::write(kr.join(file), r#"{"text":"안녕하세요"}"#)?;
        fs::write(zh.join(file), r#"{"text":"原有译文"}"#)?;
    }
    let ticket = dir.path().join("ticket.jsonl");
    write_ticket(
        &ticket,
        &[
            json!({"file":"a.json", "pointer":"/text", "source_kr":"안녕하세요", "target_zh":"你好"}),
            json!({"file":"z.json", "pointer":"/missing", "source_kr":"안녕하세요", "target_zh":"你好"}),
        ],
    )?;
    assert!(run_inject(
        &Store::open_in_memory()?,
        &zh,
        Some(&kr),
        Some(&ticket),
        true,
        false
    )
    .is_err());
    assert_eq!(
        fs::read_to_string(zh.join("a.json"))?,
        r#"{"text":"原有译文"}"#
    );
    assert!(!zh.join(".lct-backups").exists());
    assert!(!zh.join(".lct-inject.lock").exists());
    Ok(())
}

#[test]
fn injection_rejects_traversal_duplicate_nonstring_stale_and_malformed_tickets() -> Result<()> {
    let dir = tempdir()?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(kr.join("a.json"), r#"{"text":"안녕하세요","id":1}"#)?;
    fs::write(zh.join("a.json"), r#"{"text":"旧译","id":1}"#)?;
    let ticket = dir.path().join("ticket.jsonl");
    let good =
        json!({"file":"a.json", "pointer":"/text", "source_kr":"안녕하세요", "target_zh":"你好"});
    for path in [
        "../outside.json",
        "/tmp/outside.json",
        "a/../a.json",
        "a/./b.json",
        "C:\\outside.json",
    ] {
        let mut item = good.clone();
        item["file"] = json!(path);
        write_ticket(&ticket, &[item])?;
        assert!(
            run_inject(
                &Store::open_in_memory()?,
                &zh,
                Some(&kr),
                Some(&ticket),
                true,
                false
            )
            .is_err(),
            "{path}"
        );
    }
    for (field, value) in [
        ("pointer", "/id"),
        ("source_kr", "이전 원문"),
        ("target_zh", "你好1"),
    ] {
        let mut item = good.clone();
        item[field] = json!(value);
        write_ticket(&ticket, &[item])?;
        assert!(run_inject(
            &Store::open_in_memory()?,
            &zh,
            Some(&kr),
            Some(&ticket),
            true,
            false
        )
        .is_err());
    }
    write_ticket(&ticket, &[good.clone(), good.clone()])?;
    assert!(run_inject(
        &Store::open_in_memory()?,
        &zh,
        Some(&kr),
        Some(&ticket),
        true,
        false
    )
    .is_err());
    fs::write(
        &ticket,
        format!("{}\nnot-json", serde_json::to_string(&good)?),
    )?;
    let error = run_inject(
        &Store::open_in_memory()?,
        &zh,
        Some(&kr),
        Some(&ticket),
        true,
        false,
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("line 2"));
    assert_eq!(
        fs::read_to_string(zh.join("a.json"))?,
        r#"{"text":"旧译","id":1}"#
    );
    Ok(())
}

#[test]
fn injection_retains_byte_exact_backups_and_a_recovery_manifest() -> Result<()> {
    let dir = tempdir()?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(kr.join("a.json"), r#"{"text":"안녕하세요"}"#)?;
    let original = b"{\n  \"text\": \"old\"\n}\n";
    fs::write(zh.join("a.json"), original)?;
    let ticket = dir.path().join("ticket.jsonl");
    write_ticket(
        &ticket,
        &[
            json!({"file":"a.json", "pointer":"/text", "source_kr":"안녕하세요", "target_zh":"你好"}),
        ],
    )?;
    let result = run_inject(
        &Store::open_in_memory()?,
        &zh,
        Some(&kr),
        Some(&ticket),
        true,
        false,
    )?;
    let backup = result.backup_directory.unwrap();
    assert_eq!(fs::read(backup.join("originals/a.json"))?, original);
    let manifest: Value = serde_json::from_slice(&fs::read(backup.join("manifest.json"))?)?;
    assert_eq!(manifest["entries"][0]["file"], "a.json");
    assert_eq!(manifest["entries"][0]["existed"], true);
    assert_eq!(manifest["version"], 1);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(zh.join("a.json"))?)?["text"],
        "你好"
    );
    assert!(!zh.join(".lct-inject.lock").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn injection_rejects_symlink_targets() -> Result<()> {
    let dir = tempdir()?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(kr.join("a.json"), r#"{"text":"안녕하세요"}"#)?;
    let outside = dir.path().join("outside.json");
    fs::write(&outside, r#"{"text":"unchanged"}"#)?;
    std::os::unix::fs::symlink(&outside, zh.join("a.json"))?;
    let ticket = dir.path().join("ticket.jsonl");
    write_ticket(
        &ticket,
        &[
            json!({"file":"a.json", "pointer":"/text", "source_kr":"안녕하세요", "target_zh":"你好"}),
        ],
    )?;
    assert!(run_inject(
        &Store::open_in_memory()?,
        &zh,
        Some(&kr),
        Some(&ticket),
        true,
        false
    )
    .is_err());
    assert_eq!(fs::read_to_string(outside)?, r#"{"text":"unchanged"}"#);
    Ok(())
}

#[test]
fn reextract_preserves_edits_and_marks_changed_source_for_review() -> Result<()> {
    let dir = tempdir()?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(
        kr.join("a.json"),
        r#"{"dataList":[{"content":"안녕하세요"}]}"#,
    )?;
    fs::write(zh.join("a.json"), r#"{"dataList":[{"content":"旧译"}]}"#)?;
    let store = Store::open_in_memory()?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let item = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: item.id,
        expected_updated_at: item.updated_at,
        target_zh: "人工修订",
    })?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let saved = store.get_segment("test", item.id)?;
    assert_eq!(saved.target_zh.as_deref(), Some("人工修订"));
    assert_eq!(saved.stage, Stage::Reviewed);
    fs::write(
        kr.join("a.json"),
        r#"{"dataList":[{"content":"안녕히 가세요"}]}"#,
    )?;
    let ticket = dir.path().join("updated.jsonl");
    run_extract(&store, Some("test".into()), &kr, &zh, Some(&ticket), false)?;
    let changed = store.get_segment("test", item.id)?;
    assert_eq!(changed.previous_kr.as_deref(), Some("안녕하세요"));
    assert_eq!(changed.target_zh.as_deref(), Some("人工修订"));
    assert_eq!(changed.stage, Stage::OutdatedDiff);
    assert!(!changed.qa_passed);
    let exported: Value = serde_json::from_str(fs::read_to_string(ticket)?.trim())?;
    assert_eq!(exported["stage"], "OutdatedDiff");
    assert_eq!(exported["target_zh"], "人工修订");
    Ok(())
}

#[test]
fn failed_extraction_keeps_the_previous_ticket_intact() -> Result<()> {
    let dir = tempdir()?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(
        kr.join("valid.json"),
        r#"{"dataList":[{"content":"안녕하세요"}]}"#,
    )?;
    fs::write(kr.join("invalid.json"), "{broken-json")?;
    let ticket = dir.path().join("existing.jsonl");
    fs::write(&ticket, "previous approved ticket\n")?;
    assert!(run_extract(
        &Store::open_in_memory()?,
        Some("test".into()),
        &kr,
        &zh,
        Some(&ticket),
        false
    )
    .is_err());
    assert_eq!(fs::read_to_string(ticket)?, "previous approved ticket\n");
    Ok(())
}

#[test]
fn normalized_assets_round_trip_and_collisions_fail_before_database_changes() -> Result<()> {
    let dir = tempdir()?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir_all(kr.join("KR_StoryData/KR_Chapter"))?;
    fs::create_dir(&zh)?;
    fs::write(
        kr.join("KR_StoryData/KR_Chapter/KR_name_KR_middle.json"),
        r#"{"content":"안녕하세요"}"#,
    )?;
    let store = Store::open_in_memory()?;
    run_extract(&store, Some("test".into()), &kr, &zh, None, false)?;
    let path = "StoryData/Chapter/name_KR_middle.json";
    assert_eq!(store.get_files("test")?[0].relative_path, path);
    let ticket = dir.path().join("ticket.jsonl");
    write_ticket(
        &ticket,
        &[json!({"file":path,"pointer":"/content","source_kr":"안녕하세요","target_zh":"你好"})],
    )?;
    run_inject(&store, &zh, Some(&kr), Some(&ticket), true, false)?;
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(zh.join(path))?)?["content"],
        "你好"
    );
    fs::create_dir_all(kr.join("StoryData/Chapter"))?;
    fs::write(kr.join(path), r#"{"content":"다른 원문"}"#)?;
    let empty = Store::open_in_memory()?;
    let error = run_extract(&empty, Some("other".into()), &kr, &zh, None, false).unwrap_err();
    assert!(format!("{error:#}").contains("collision"));
    assert!(empty.get_latest_project()?.is_none());
    Ok(())
}

#[test]
fn ambiguous_tm_is_not_filled_and_summary_counts_persisted_edits() -> Result<()> {
    let dir = tempdir()?;
    let kr = dir.path().join("kr");
    let zh = dir.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(kr.join("a.json"), r#"{"content":"안녕하세요"}"#)?;
    let store = Store::open_in_memory()?;
    for target in ["你好", "您好"] {
        store.upsert_tm_entry(&loc_core::TMEntry::new(
            loc_tm::hash_text("안녕하세요"),
            "안녕하세요",
            target,
            "test",
            1,
        ))?;
    }
    let first = run_extract(&store, Some("test".into()), &kr, &zh, None, true)?;
    assert_eq!(first.tm_exact_filled, 0);
    assert_eq!(first.pending_untranslated, 1);
    let item = store
        .query_segments_paged("test", None, None, None, 0, 10)?
        .1
        .remove(0);
    store.save_reviewed_translation(&SaveTranslation {
        project_id: "test",
        id: item.id,
        target_zh: "人工译文",
        expected_updated_at: item.updated_at,
    })?;
    let second = run_extract(&store, Some("test".into()), &kr, &zh, None, true)?;
    assert_eq!(second.already_translated, 1);
    assert_eq!(second.pending_untranslated, 0);
    fs::write(kr.join("a.json"), r#"{"content":"안녕히 가세요"}"#)?;
    let third = run_extract(&store, Some("test".into()), &kr, &zh, None, true)?;
    assert_eq!(third.needs_review, 1);
    assert_eq!(
        third.total_segments,
        third.already_translated
            + third.pending_untranslated
            + third.tm_exact_filled
            + third.needs_review
    );
    Ok(())
}

#[test]
fn injection_rejects_reordered_target_records_before_writing() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    fs::write(
        kr.join("a.json"),
        r#"{"dataList":[{"id":1,"text":"안녕하세요"},{"id":2,"text":"감사합니다"}]}"#,
    )?;
    let original = r#"{"dataList":[{"id":2,"text":"谢谢"},{"id":1,"text":"你好"}]}"#;
    fs::write(zh.join("a.json"), original)?;
    let ticket = root.path().join("ticket.jsonl");
    write_ticket(
        &ticket,
        &[
            json!({"file":"a.json","pointer":"/dataList/0/text","source_kr":"안녕하세요","target_zh":"您好"}),
        ],
    )?;
    let error = run_inject(
        &Store::open_in_memory()?,
        &zh,
        Some(&kr),
        Some(&ticket),
        true,
        false,
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("Target array identity differs"));
    assert_eq!(fs::read_to_string(zh.join("a.json"))?, original);
    assert!(!zh.join(".lct-inject.lock").exists());
    Ok(())
}
