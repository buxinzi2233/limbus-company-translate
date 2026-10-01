#[path = "../../loc-core/tests/support/draft_grants.rs"]
mod draft_grants;
use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use loc_core::{FileRecord, Project, Segment, Store};
use loc_qa::{submit_ai_draft, DraftSubmission};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn call(app: &Router, method: &str, path: &str, body: Value) -> Result<(StatusCode, Value)> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 2_000_000).await?;
    Ok((status, serde_json::from_slice(&bytes)?))
}

#[tokio::test]
async fn review_queue_audit_pagination_and_failed_review_are_project_scoped() -> Result<()> {
    let store = Store::open_in_memory()?;
    for project_id in ["foreign", "active"] {
        store.upsert_project(&Project::new(project_id, project_id))?;
        let file = store.upsert_file(&FileRecord::new(project_id, "a.json", "system"))?;
        store.batch_upsert_segments(&[Segment::new(file, project_id, "/text", "안녕하세요")])?;
    }
    let active_file = store.get_files("active")?[0].id;
    store.batch_upsert_segments(
        &(1..21)
            .map(|i| Segment::new(active_file, "active", &format!("/text{i}"), "안녕하세요"))
            .collect::<Vec<_>>(),
    )?;
    let foreign_permit = draft_grants::grant(&store, "foreign", vec![1])?;
    let active_permit = draft_grants::grant(&store, "active", (2..23).collect())?;
    let foreign = store.get_segment("foreign", 1)?;
    submit_ai_draft(
        &store,
        &DraftSubmission {
            project_id: "foreign".into(),
            id: 1,
            expected_updated_at: foreign.updated_at,
            target_zh: "外部项目".into(),
            producer: "foreign-agent".into(),
        },
        Some(&foreign_permit),
    )?;
    for index in 0..21 {
        let current = store.get_segment("active", 2 + index)?;
        submit_ai_draft(
            &store,
            &DraftSubmission {
                project_id: "active".into(),
                id: current.id,
                expected_updated_at: current.updated_at,
                target_zh: if index % 2 == 0 {
                    "你好".into()
                } else {
                    "您好".into()
                },
                producer: "test-agent".into(),
            },
            Some(&active_permit),
        )?;
    }
    let app = loc_server::create_router(store.clone());
    let (status, queue) = call(&app, "GET", "/api/review-queue", Value::Null).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(queue["total"], 21);
    assert_eq!(queue["items"][0]["stage"], "AiDrafted");
    let (status, audit) = call(&app, "GET", "/api/audit", Value::Null).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit["events"].as_array().unwrap().len(), 20);
    assert!(audit["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|event| (2..23).contains(&event["segment_id"].as_i64().unwrap())));
    let cursor = audit["next_events"].as_i64().unwrap();
    let (_, older) = call(
        &app,
        "GET",
        &format!("/api/audit?before={cursor}"),
        Value::Null,
    )
    .await?;
    assert_eq!(older["events"].as_array().unwrap().len(), 1);
    assert!(older["next_events"].is_null());
    for query in [
        "before=0",
        "imports_before=-1",
        "corpus_offset=1000001",
        "operations_before=-1",
    ] {
        assert_eq!(
            call(&app, "GET", &format!("/api/audit?{query}"), Value::Null)
                .await?
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let row = store.get_segment("active", 2)?;
    let save = json!({"project_id":"active","id":2,"expected_updated_at":row.updated_at,"stage":4,"target_zh":"你好"});
    store.raw_connection().lock().unwrap().execute_batch("CREATE TRIGGER reject_review BEFORE INSERT ON translation_events WHEN NEW.kind='review' BEGIN SELECT RAISE(ABORT,'review audit unavailable'); END;")?;
    assert_eq!(
        call(&app, "POST", "/api/segments/update", save.clone())
            .await?
            .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(store.get_segment("active", 2)?.updated_at, row.updated_at);
    assert_eq!(store.translation_events("active", None, 100)?.len(), 21);
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_review;")?;
    assert_eq!(
        call(&app, "POST", "/api/segments/update", save).await?.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/api/review-queue", Value::Null).await?.1["total"],
        20
    );
    assert_eq!(
        call(&app, "GET", "/api/audit", Value::Null).await?.1["events"][0]["kind"],
        "review"
    );
    Ok(())
}

#[tokio::test]
async fn explicitly_bound_ui_does_not_switch_when_another_project_is_created() -> Result<()> {
    let store = Store::open_in_memory()?;
    store.upsert_project(&Project::new("selected", "Selected"))?;
    let app = loc_server::create_scoped_router(store.clone(), Some("selected".into()));
    store.upsert_project(&Project::new("newer", "Newer"))?;
    assert_eq!(
        call(&app, "GET", "/api/status", Value::Null).await?.1["project"]["id"],
        "selected"
    );
    assert_eq!(
        call(&app, "GET", "/api/audit", Value::Null).await?.1["project"]["id"],
        "selected"
    );
    let missing = loc_server::create_scoped_router(store, Some("missing".into()));
    assert_eq!(
        call(&missing, "GET", "/api/status", Value::Null).await?.0,
        StatusCode::NOT_FOUND
    );
    Ok(())
}
