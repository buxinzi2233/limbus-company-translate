#[path = "../../loc-core/tests/support/draft_grants.rs"]
mod draft_grants;
use anyhow::{Context, Result};
use loc_core::{FileRecord, Project, Segment, Store};
use rmcp::{
    model::{CallToolRequest, CallToolRequestParams, ClientRequest},
    service::PeerRequestOptions,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::{json, Value};
use std::time::Duration;
use tempfile::tempdir;

fn request(name: &str, arguments: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name.to_owned())
        .with_arguments(arguments.as_object().unwrap().clone())
}

#[tokio::test]
async fn real_stdio_client_exercises_scoped_tools_and_write_boundaries() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let root = tempdir()?;
        let store = Store::open(root.path().join("project.db"))?;
        for name in ["bound", "foreign"] {
            store.upsert_project(&Project::new(name, name))?;
            let file = store.upsert_file(&FileRecord::new(name, "a.json", "system"))?;
            store.batch_upsert_segments(&[Segment::new(file, name, "/text", "체력 {0} 20%")])?;
        }
        let permit = draft_grants::grant(&store, "bound", vec![1])?;
        let permit_path = root.path().join("permit.json");
        std::fs::write(&permit_path, serde_json::to_vec(&permit)?)?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&permit_path, std::fs::Permissions::from_mode(0o600))?;
        }
        let row = store.get_segment("bound", 1)?;
        let transport = TokioChildProcess::new(tokio::process::Command::new(env!("CARGO_BIN_EXE_lct")).configure(|c| {
            c.arg("--data-dir").arg(root.path()).args(["mcp", "--project-id", "bound", "--draft-permit"]).arg(&permit_path);
        }))?;
        let client = ().serve(transport).await?;
        let tools = client.list_all_tools().await?;
        assert_eq!(tools.len(), 9);
        let write_tool = tools.iter().find(|tool| tool.name == "submit_draft").unwrap();
        assert_eq!(write_tool.annotations.as_ref().unwrap().read_only_hint, Some(false));
        assert_eq!(write_tool.annotations.as_ref().unwrap().destructive_hint, Some(true));
        assert!(tools.iter().all(|tool| tool.output_schema.is_some()));
        assert!(tools.iter().all(|tool| tool.name != "approve" && tool.name != "inject"));
        let status = client.call_tool(request("project_status", json!({"project_id":"bound"}))).await?;
        assert_eq!(status.structured_content.unwrap()["total"], 1);
        let coverage = client.call_tool(request("reference_status", json!({"project_id":"bound"}))).await?;
        assert_eq!(coverage.structured_content.unwrap()["configured"], false);
        let unconfigured = client.call_tool(request("search_references", json!({"project_id":"bound","query":"呼吸法","limit":5}))).await?;
        assert_eq!(unconfigured.is_error, Some(true));
        assert!(unconfigured.structured_content.unwrap()["message"].as_str().unwrap().contains("No reference database configured"));

        let mismatch = client.call_tool(request("get_segment", json!({"project_id":"foreign","id":2}))).await?;
        assert_eq!(mismatch.is_error, Some(true));
        assert_eq!(mismatch.structured_content.unwrap()["code"], "project_mismatch");
        let foreign_id = client.call_tool(request("get_segment", json!({"project_id":"bound","id":2}))).await?;
        assert_eq!(foreign_id.structured_content.unwrap()["code"], "not_found");
        let invalid = client.call_tool(request("list_segments", json!({"project_id":"bound","offset":0,"limit":51}))).await?;
        assert_eq!(invalid.structured_content.unwrap()["code"], "invalid_input");
        assert_eq!(client.call_tool(request("get_segment", json!({"project_id":"bound"}))).await?.is_error, Some(true));
        assert!(client.call_tool(request("no_such_tool", json!({}))).await.is_err());
        let page = client.call_tool(request("list_segments", json!({"project_id":"bound","offset":0,"limit":1,"stage":0,"search":null,"irrelevant":"ignored"}))).await?;
        assert_eq!(page.structured_content.unwrap()["items"][0]["id"], 1);
        let memory = client.call_tool(request("search_memory", json!({"project_id":"bound","query":"体力","limit":5}))).await?;
        assert_eq!(memory.structured_content.unwrap()["scope"], "database_shared_reference_only");
        let checked = client.call_tool(request("check_translation", json!({"project_id":"bound","id":1,"target_zh":"体力{0}20%"}))).await?;
        assert_eq!(checked.structured_content.unwrap()["passed"], true);
        let rejected = client.call_tool(request("submit_draft", json!({"project_id":"bound","id":1,"expected_updated_at":row.updated_at,"target_zh":"体力20%","producer":"integration-client"}))).await?;
        assert_eq!(rejected.is_error, Some(true));
        assert_eq!(rejected.structured_content.unwrap()["code"], "qa_rejected");
        assert_eq!(store.get_segment("bound",1)?.updated_at, row.updated_at);
        let draft_args = json!({"project_id":"bound","id":1,"expected_updated_at":row.updated_at,"target_zh":"体力{0}20%","producer":"integration-client","stage":4});
        let saved = client.call_tool(request("submit_draft", draft_args.clone())).await?;
        assert_eq!(saved.is_error, Some(false));
        let saved = saved.structured_content.unwrap();
        assert_eq!(saved["stage"], 3);
        assert_eq!(saved["review_confirmed"], false);
        let conflict = client.call_tool(request("submit_draft", draft_args.clone())).await?;
        assert_eq!(conflict.structured_content.unwrap()["code"], "conflict");
        let events = client.call_tool(request("list_review_events", json!({"project_id":"bound","before_id":null,"limit":10}))).await?;
        assert_eq!(events.structured_content.unwrap()["items"][0]["kind"], "ai_draft");
        let current = store.get_segment("bound", 1)?;
        let mut current_args = draft_args;
        current_args["expected_updated_at"] = json!(current.updated_at);
        store.raw_connection().lock().unwrap().execute("UPDATE files SET is_locked=1 WHERE project_id='bound'", [])?;
        let locked = client.call_tool(request("submit_draft", current_args.clone())).await?;
        assert_eq!(locked.structured_content.unwrap()["code"], "locked");
        store.raw_connection().lock().unwrap().execute("UPDATE files SET is_locked=0 WHERE project_id='bound'", [])?;
        store.save_reviewed_translation(&loc_core::SaveTranslation { project_id:"bound",id:1,expected_updated_at:current.updated_at,target_zh:"体力{0}20%" })?;
        current_args["expected_updated_at"] = json!(store.get_segment("bound",1)?.updated_at);
        let reviewed = client.call_tool(request("submit_draft", current_args)).await?;
        assert_eq!(reviewed.structured_content.unwrap()["code"], "reviewed");
        assert_eq!(store.translation_events("bound", None, 10)?.len(), 2);

        // Exercise a real cancellation while SQLite is locked, then keep using the session.
        store.raw_connection().lock().unwrap().execute_batch("BEGIN EXCLUSIVE;")?;
        let pending = client.send_cancellable_request(
            ClientRequest::CallToolRequest(CallToolRequest::new(request("list_segments", json!({"project_id":"bound","offset":0,"limit":1})))),
            PeerRequestOptions::no_options(),
        ).await?;
        pending.cancel(Some("Integration cancellation".into())).await?;
        store.raw_connection().lock().unwrap().execute_batch("ROLLBACK;")?;
        let after_cancel = client.call_tool(request("project_status", json!({"project_id":"bound"}))).await?;
        assert_eq!(after_cancel.structured_content.unwrap()["reviewed"], 1);
        client.cancel().await?;
        Ok(())
    }).await.context("Real MCP workflow exceeded 30 seconds")?
}

#[test]
fn mcp_rejects_missing_database_and_project_without_stdout_noise() -> Result<()> {
    let root = tempdir()?;
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lct"))
        .arg("--data-dir")
        .arg(root.path())
        .args(["mcp", "--project-id", "missing"])
        .output()?;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!root.path().join("project.db").exists());
    Store::open(root.path().join("project.db"))?;
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lct"))
        .arg("--data-dir")
        .arg(root.path())
        .args(["mcp", "--project-id", "missing"])
        .output()?;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not exist"));
    Ok(())
}

#[test]
fn mcp_rejects_malformed_and_oversized_frames_explicitly() -> Result<()> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let root = tempdir()?;
    let store = Store::open(root.path().join("project.db"))?;
    store.upsert_project(&Project::new("bound", "Bound"))?;
    for input in [b"{broken}\n".to_vec(), vec![b'x'; 1_048_578]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_lct"))
            .arg("--data-dir")
            .arg(root.path())
            .args(["mcp", "--project-id", "bound"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdin = child.stdin.take().unwrap();
        if let Err(error) = stdin.write_all(&input) {
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
        }
        drop(stdin);
        let output = child.wait_with_output()?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid MCP frame"));
    }
    Ok(())
}
