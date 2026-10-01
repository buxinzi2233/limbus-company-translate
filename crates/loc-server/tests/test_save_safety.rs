use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use loc_core::{FileRecord, Project, Segment, Stage, Store};
use serde_json::{json, Value};
use tower::ServiceExt;

fn fixture() -> Result<(Store, Router)> {
    let store = Store::open_in_memory()?;
    let old = Project::new("other", "Other project");
    store.upsert_project(&old)?;
    let other_file = store.upsert_file(&FileRecord::new("other", "other.json", "story"))?;
    let other = Segment::new(other_file, "other", "/text", "{0}에게 {1}의 피해를 입힘");
    store.batch_upsert_segments(&[other])?;
    let mut active = Project::new("active", "Active project");
    active.updated_at = old.updated_at + chrono::Duration::seconds(1);
    store.upsert_project(&active)?;
    let file = store.upsert_file(&FileRecord::new("active", "active.json", "story"))?;
    let item = Segment::new(file, "active", "/text", "{0}에게 {1}의 피해를 입힘");
    store.batch_upsert_segments(&[item])?;
    Ok((store.clone(), loc_server::create_router(store)))
}

async fn call(app: &Router, method: &str, uri: &str, body: Value) -> Result<(StatusCode, Value)> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("Host", "127.0.0.1:3000")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&bytes)?))
}

#[tokio::test]
async fn reviewed_save_is_source_aware_versioned_and_updates_scoped_stats() -> Result<()> {
    let (store, app) = fixture()?;
    let before = store.get_segment("active", 2)?;
    let mut request = json!({"project_id":"active", "id":2, "stage":4,
        "target_zh":"对{0}造成{1}点伤害", "expected_updated_at":before.updated_at});
    let (status, saved) = call(&app, "POST", "/api/segments/update", request.clone()).await?;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["item"]["stage"], "Reviewed");
    assert_eq!(saved["qa_passed"], true);
    assert_ne!(saved["item"]["updated_at"], request["expected_updated_at"]);
    let persisted = store.get_segment("active", 2)?;
    assert_eq!(persisted.target_zh.as_deref(), Some("对{0}造成{1}点伤害"));
    request["target_zh"] = json!("向{0}施加{1}点伤害");
    let (status, _) = call(&app, "POST", "/api/segments/update", request).await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        store.get_segment("active", 2)?.target_zh,
        persisted.target_zh
    );
    let (_, status) = call(&app, "GET", "/api/status", Value::Null).await?;
    assert_eq!(status["stats"]["total_segments"], 1);
    assert_eq!(status["stats"]["translated_segments"], 1);
    assert_eq!(status["stats"]["reviewed_segments"], 1);
    assert_eq!(status["stats"]["categories"]["story"]["translated"], 1);
    let (_, files) = call(&app, "GET", "/api/files", Value::Null).await?;
    assert_eq!(files["files"][0]["translated_segments"], 1);
    let (_, items) = call(&app, "GET", "/api/segments", Value::Null).await?;
    assert_eq!(items["total"], 1);
    assert_eq!(items["items"][0]["project_id"], "active");
    let (_, search) = call(&app, "GET", "/api/search?q=%7B0%7D", Value::Null).await?;
    assert_eq!(search["segments"].as_array().unwrap().len(), 1);
    assert_eq!(search["segments"][0]["segment_id"], 2);
    Ok(())
}

#[tokio::test]
async fn rejected_qa_and_missing_or_foreign_records_do_not_write() -> Result<()> {
    let (store, app) = fixture()?;
    let before = store.get_segment("active", 2)?;
    for (id, project, text, expected) in [
        (2, "active", "伤害", StatusCode::UNPROCESSABLE_ENTITY),
        (1, "active", "对{0}造成{1}点伤害", StatusCode::NOT_FOUND),
        (2, "other", "对{0}造成{1}点伤害", StatusCode::CONFLICT),
        (999, "active", "对{0}造成{1}点伤害", StatusCode::NOT_FOUND),
    ] {
        let (status, body) = call(&app, "POST", "/api/segments/update", json!({
            "project_id":project, "id":id, "stage":4, "target_zh":text, "expected_updated_at":before.updated_at,
        })).await?;
        assert_eq!(status, expected, "{body}");
    }
    let after = store.get_segment("active", 2)?;
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(after.target_zh, None);
    assert_eq!(after.stage, Stage::Untranslated);
    Ok(())
}

#[tokio::test]
async fn server_enforces_both_file_and_segment_locks() -> Result<()> {
    for file_lock in [true, false] {
        let (store, app) = fixture()?;
        let connection = store.raw_connection();
        if file_lock {
            connection.lock().unwrap().execute(
                "UPDATE files SET is_locked = 1 WHERE project_id = 'active'",
                [],
            )?;
        } else {
            connection
                .lock()
                .unwrap()
                .execute("UPDATE segments SET stage = 9 WHERE id = 2", [])?;
        }
        let current = store.get_segment("active", 2)?;
        let (status, _) = call(
            &app,
            "POST",
            "/api/segments/update",
            json!({"project_id":"active", "id":2,
            "target_zh":"对{0}造成{1}点伤害", "stage":4, "expected_updated_at":current.updated_at}),
        )
        .await?;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(store.get_segment("active", 2)?.target_zh, None);
    }
    Ok(())
}

#[tokio::test]
async fn request_boundaries_and_database_errors_are_not_silenced() -> Result<()> {
    let (store, app) = fixture()?;
    for url in [
        "/api/segments?page=0",
        "/api/segments?page=18446744073709551615&page_size=200",
        "/api/segments?stage=8",
        "/api/search?q=x&limit=999999",
    ] {
        assert_eq!(
            call(&app, "GET", url, Value::Null).await?.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        call(&app, "GET", "/api/does-not-exist", Value::Null)
            .await?
            .0,
        StatusCode::NOT_FOUND
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .header("Host", "evil.example")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute("DROP TABLE tm_entries", [])?;
    assert_eq!(
        call(&app, "GET", "/api/status", Value::Null).await?.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        call(&app, "GET", "/api/search?q=test", Value::Null)
            .await?
            .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    Ok(())
}
