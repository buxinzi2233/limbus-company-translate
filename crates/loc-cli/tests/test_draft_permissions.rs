use anyhow::Result;
use loc_cli::commands::run_extract;
use loc_core::{
    DraftAccessError, DraftGrantRequest, DraftPermit, ScopeManifest, SegmentWithFile, Store,
};
use loc_qa::{submit_ai_draft, DraftSubmission};
use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    sync::{Arc, Barrier},
};
use tempfile::{tempdir, TempDir};

fn fixture() -> Result<(TempDir, Store, Vec<SegmentWithFile>)> {
    let root = tempdir()?;
    fs::create_dir(root.path().join("kr"))?;
    fs::create_dir(root.path().join("zh"))?;
    fs::write(
        root.path().join("kr/a.json"),
        r#"{"dataList":[{"id":1,"text":"체력 {0} 20%"},{"id":2,"text":"체력 {0} 20%"},{"id":3,"text":"체력 {0} 20%"},{"id":4,"text":"체력 {0} 20%"}]}"#,
    )?;
    let store = Store::open(root.path().join("project.db"))?;
    run_extract(
        &store,
        Some("bound".into()),
        &root.path().join("kr"),
        &root.path().join("zh"),
        None,
        false,
    )?;
    let rows = store
        .query_segments_paged("bound", None, None, None, 0, 100)?
        .1;
    assert_eq!(rows.len(), 4);
    install_scope(&store, &rows[..3], "a")?;
    Ok((root, store, rows))
}

fn install_scope(store: &Store, rows: &[SegmentWithFile], digest_char: &str) -> Result<()> {
    let identities: Vec<String> = {
        let conn = store.raw_connection();
        let conn = conn.lock().unwrap();
        rows.iter()
            .map(|r| {
                Ok(conn.query_row(
                    "SELECT source_identity FROM segments WHERE id=?1",
                    [r.id],
                    |r| r.get(0),
                )?)
            })
            .collect::<Result<_>>()?
    };
    let manifest: ScopeManifest = serde_json::from_value(json!({
        "project_id":"bound", "files":[{"path":"a.json","sha256":"a".repeat(64),"selected_segments":rows.len()}],
        "segments":rows.iter().zip(identities).map(|(r,identity)|json!({"segment_id":r.id,"path":"a.json","pointer":r.key_path,"source_identity":identity,"source_kr":r.source_kr})).collect::<Vec<_>>()
    }))?;
    store.import_task_scope(
        &manifest,
        "Explicit task",
        &digest_char.repeat(64),
        store.latest_source_import_id("bound")?,
    )?;
    Ok(())
}

fn issue(
    store: &Store,
    worker: &str,
    ids: Vec<i64>,
) -> Result<(loc_core::DraftGrantReceipt, DraftPermit)> {
    let scope = store.task_scope("bound")?.unwrap();
    store.create_draft_grant(&DraftGrantRequest {
        project_id: "bound".into(),
        worker: worker.into(),
        manifest_sha256: scope.manifest_sha256,
        source_import_id: scope.source_import_id,
        segment_ids: ids,
    })
}

fn submission(row: &SegmentWithFile) -> DraftSubmission {
    DraftSubmission {
        project_id: "bound".into(),
        id: row.id,
        expected_updated_at: row.updated_at,
        target_zh: "体力{0}20%".into(),
        producer: "arbitrary-label".into(),
    }
}

fn permit_file(root: &Path, permit: &DraftPermit) -> Result<std::path::PathBuf> {
    let path = root.join("permit.json");
    fs::write(&path, serde_json::to_vec(permit)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

#[tokio::test]
async fn real_mcp_defaults_to_read_only_and_enforces_process_bound_membership() -> Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let (root, store, rows) = fixture()?;
        let (grant, permit) = issue(&store, "worker-A", vec![rows[0].id])?;
        let path = permit_file(root.path(), &permit)?;
        for configured in [false, true] {
            let transport = TokioChildProcess::new(
                tokio::process::Command::new(env!("CARGO_BIN_EXE_lct")).configure(|c| {
                    c.arg("--data-dir")
                        .arg(root.path())
                        .args(["mcp", "--project-id", "bound"]);
                    if configured {
                        c.arg("--draft-permit").arg(&path);
                    }
                }),
            )?;
            let client = ().serve(transport).await?;
            let status = client
                .call_tool(
                    CallToolRequestParams::new("project_status")
                        .with_arguments(json!({"project_id":"bound"}).as_object().unwrap().clone()),
                )
                .await?
                .structured_content
                .unwrap();
            assert_eq!(status["draft_grant_id"].is_string(), configured);
            for row in &rows {
                let mut args = serde_json::to_value(submission(row))?;
                // Caller-supplied capability fields and producer labels cannot change the binding.
                args["permit"] = serde_json::to_value(&permit)?;
                args["stage"] = json!(4);
                let reply = client
                    .call_tool(
                        CallToolRequestParams::new("submit_draft")
                            .with_arguments(args.as_object().unwrap().clone()),
                    )
                    .await?;
                let result = reply.structured_content.unwrap();
                if configured && row.id == rows[0].id {
                    assert_eq!(result["stage"], 3, "{result}");
                    assert_eq!(result["review_confirmed"], false);
                } else {
                    assert_eq!(result["code"], "write_forbidden", "{result}");
                }
            }
            if configured {
                let current = store.get_segment("bound", rows[0].id)?;
                let result = client
                    .call_tool(
                        CallToolRequestParams::new("submit_draft").with_arguments(
                            serde_json::to_value(submission(&current))?
                                .as_object()
                                .unwrap()
                                .clone(),
                        ),
                    )
                    .await?
                    .structured_content
                    .unwrap();
                assert_eq!(result["code"], "existing_translation");
                store.revoke_draft_grant("bound", &grant.id)?;
                let result = client
                    .call_tool(
                        CallToolRequestParams::new("submit_draft").with_arguments(
                            serde_json::to_value(submission(&rows[1]))?
                                .as_object()
                                .unwrap()
                                .clone(),
                        ),
                    )
                    .await?
                    .structured_content
                    .unwrap();
                assert_eq!(result["code"], "write_forbidden");
            }
            client.cancel().await?;
        }
        assert_eq!(store.translation_events("bound", None, 50)?.len(), 1);
        for row in &rows[1..] {
            assert_eq!(store.get_segment("bound", row.id)?.target_zh, None);
        }
        Ok(())
    })
    .await?
}

#[test]
fn grants_are_exclusive_revocable_and_pinned_to_scope_and_source() -> Result<()> {
    let (root, store, rows) = fixture()?;
    let (receipt, permit) = issue(&store, "A", vec![rows[0].id])?;
    assert!(issue(&store, "B", vec![rows[1].id, rows[0].id]).is_err());
    // A failed multi-ID grant rolls back even its non-overlapping members.
    let (_, second) = issue(&store, "B", vec![rows[1].id])?;
    assert!(issue(&store, "outside", vec![rows[3].id]).is_err());
    store.revoke_draft_grant("bound", &receipt.id)?;
    assert!(submit_ai_draft(&store, &submission(&rows[0]), Some(&permit)).is_err());
    let (_, replacement) = issue(&store, "replacement", vec![rows[0].id])?;
    install_scope(&store, &rows[..3], "b")?;
    assert!(submit_ai_draft(&store, &submission(&rows[0]), Some(&replacement)).is_err());
    assert!(submit_ai_draft(&store, &submission(&rows[1]), Some(&second)).is_err());
    let (_, third) = issue(&store, "C", vec![rows[2].id])?;
    // Even unchanged individual source text does not permit writes after a new import.
    run_extract(
        &store,
        Some("bound".into()),
        &root.path().join("kr"),
        &root.path().join("zh"),
        None,
        false,
    )?;
    let latest = store.get_segment("bound", rows[2].id)?;
    assert!(submit_ai_draft(&store, &submission(&latest), Some(&third)).is_err());
    assert!(store.translation_events("bound", None, 50)?.is_empty());
    Ok(())
}

#[test]
fn first_draft_only_rejects_existing_text_and_concurrent_writers() -> Result<()> {
    let (root, store, rows) = fixture()?;
    let (_, permit) = issue(&store, "A", vec![rows[0].id, rows[1].id, rows[2].id])?;
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let store = Store::open(root.path().join("project.db")).unwrap();
            let permit = permit.clone();
            let input = submission(&rows[0]);
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                submit_ai_draft(&store, &input, Some(&permit)).is_ok()
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|ok| **ok).count(), 1);
    let current = store.get_segment("bound", rows[0].id)?;
    let error = submit_ai_draft(&store, &submission(&current), Some(&permit)).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<DraftAccessError>(),
        Some(DraftAccessError::Existing(_))
    ));
    for stage in [1, 2, 4, 5, 9] {
        // Simulate previously imported/reviewed records, not an API write bypass.
        store.raw_connection().lock().unwrap().execute(
            "UPDATE segments SET target_zh='旧译文',stage=?1 WHERE id=?2",
            [stage, rows[1].id],
        )?;
        let current = store.get_segment("bound", rows[1].id)?;
        assert!(submit_ai_draft(&store, &submission(&current), Some(&permit)).is_err());
        assert_eq!(
            store.get_segment("bound", rows[1].id)?.target_zh.as_deref(),
            Some("旧译文")
        );
    }
    store.raw_connection().lock().unwrap().execute(
        "UPDATE segments SET target_zh='旧译文',stage=0 WHERE id=?1",
        [rows[1].id],
    )?;
    assert!(submit_ai_draft(
        &store,
        &submission(&store.get_segment("bound", rows[1].id)?),
        Some(&permit)
    )
    .is_err());
    // Fresh CAS cannot bypass the version pinned when a grant was issued.
    store.raw_connection().lock().unwrap().execute(
        "UPDATE segments SET updated_at='2099-01-01T00:00:00Z' WHERE id=?1",
        [rows[2].id],
    )?;
    assert!(submit_ai_draft(
        &store,
        &submission(&store.get_segment("bound", rows[2].id)?),
        Some(&permit)
    )
    .is_err());
    assert_eq!(store.translation_events("bound", None, 50)?.len(), 1);
    Ok(())
}

#[test]
fn real_cli_issues_private_permit_and_rejects_missing_or_revoked_capabilities() -> Result<()> {
    let (root, store, rows) = fixture()?;
    let scope = store.task_scope("bound")?.unwrap();
    let request = root.path().join("request.json");
    let output = root.path().join("cli-permit.json");
    fs::write(
        &request,
        serde_json::to_vec(
            &json!({"project_id":"bound","worker":"cli-worker","manifest_sha256":scope.manifest_sha256,"source_import_id":scope.source_import_id,"segment_ids":[rows[0].id]}),
        )?,
    )?;
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_lct"))
        .arg("--data-dir")
        .arg(root.path())
        .args(["draft-grant", "--input"])
        .arg(&request)
        .arg("--output")
        .arg(&output)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let receipt: Value = serde_json::from_slice(&result.stdout)?;
    assert!(receipt.get("token").is_none());
    let permit = loc_cli::commands::draft_grants::read_permit(&output)?;
    submit_ai_draft(&store, &submission(&rows[0]), Some(&permit))?;
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_lct"))
        .arg("--data-dir")
        .arg(root.path())
        .args([
            "draft-revoke",
            "--project-id",
            "bound",
            "--grant-id",
            receipt["id"].as_str().unwrap(),
        ])
        .output()?;
    assert!(result.status.success());
    assert!(store.check_draft_permit("bound", &permit).is_err());
    assert!(submit_ai_draft(&store, &submission(&rows[1]), None).is_err());
    Ok(())
}

#[test]
fn absent_scope_forged_permits_and_wrong_projects_fail_closed() -> Result<()> {
    let (root, store, rows) = fixture()?;
    let (_, permit) = issue(&store, "owner", vec![rows[0].id])?;
    let forged: DraftPermit = serde_json::from_value(json!({"token":"0".repeat(64)}))?;
    assert!(submit_ai_draft(&store, &submission(&rows[0]), Some(&forged)).is_err());
    store.upsert_project(&loc_core::Project::new("other", "Other"))?;
    assert!(store.check_draft_permit("other", &permit).is_err());
    let wrong = DraftSubmission {
        project_id: "other".into(),
        ..submission(&rows[0])
    };
    assert!(submit_ai_draft(&store, &wrong, Some(&permit)).is_err());
    let file = permit_file(root.path(), &forged)?;
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_lct"))
        .arg("--data-dir")
        .arg(root.path())
        .args(["mcp", "--project-id", "bound", "--draft-permit"])
        .arg(file)
        .output()?;
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&result.stderr).contains(&"0".repeat(64)));
    // Remove only test-scope metadata to exercise the missing-scope gate.
    store.raw_connection().lock().unwrap().execute(
        "DELETE FROM translation_scope_segments WHERE project_id='bound'",
        [],
    )?;
    store.raw_connection().lock().unwrap().execute(
        "DELETE FROM translation_scope_files WHERE project_id='bound'",
        [],
    )?;
    store.raw_connection().lock().unwrap().execute(
        "DELETE FROM translation_scopes WHERE project_id='bound'",
        [],
    )?;
    assert!(submit_ai_draft(&store, &submission(&rows[0]), Some(&permit)).is_err());
    assert!(store.translation_events("bound", None, 50)?.is_empty());
    assert_eq!(
        store.get_segment("bound", rows[0].id)?.updated_at,
        rows[0].updated_at
    );
    Ok(())
}

#[test]
fn legacy_draft_updates_are_fenced_and_successful_transactions_leave_no_guard() -> Result<()> {
    let (_root, store, rows) = fixture()?;
    let error = store
        .raw_connection()
        .lock()
        .unwrap()
        .execute(
            "UPDATE segments SET target_zh='旧进程越界写入',stage=3 WHERE id=?1",
            [rows[3].id],
        )
        .unwrap_err();
    assert!(error.to_string().contains("Draft write denied"));
    assert_eq!(store.get_segment("bound", rows[3].id)?.target_zh, None);
    let (_, permit) = issue(&store, "current", vec![rows[0].id])?;
    submit_ai_draft(&store, &submission(&rows[0]), Some(&permit))?;
    let remaining: i64 = store.raw_connection().lock().unwrap().query_row(
        "SELECT COUNT(*) FROM draft_write_guards",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(remaining, 0);
    assert!(store
        .raw_connection()
        .lock()
        .unwrap()
        .execute(
            "UPDATE segments SET target_zh='旧进程覆盖草稿',stage=3 WHERE id=?1",
            [rows[0].id],
        )
        .is_err());
    assert_eq!(store.translation_events("bound", None, 50)?.len(), 1);
    Ok(())
}
