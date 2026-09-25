use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use loc_core::{FileRecord, Project, Segment, Store};
use loc_server::create_router;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn test_api_endpoints_full_lifecycle() -> Result<()> {
    let store = Store::open_in_memory()?;

    // 初始化测试数据
    let project = Project::new("test-lct", "LCT Test Project");
    store.upsert_project(&project)?;

    let file_rec = FileRecord {
        id: 0,
        project_id: "test-lct".to_string(),
        relative_path: "StoryData/S101A.json".to_string(),
        category: "story".to_string(),
        total_segments: 1,
        translated_segments: 0,
        reviewed_segments: 0,
        is_locked: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let file_id = store.upsert_file(&file_rec)?;

    let segment = Segment::new(
        file_id,
        "test-lct",
        "/dataList/0/content",
        "우리는 마침내 도착했다.",
    );
    store.batch_upsert_segments(&[segment])?;

    let app = create_router(store);

    // 1. 测试 GET /api/status
    let req = Request::builder()
        .uri("/api/status")
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. 测试 GET /api/files
    let req = Request::builder()
        .uri("/api/files")
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. 测试 GET /api/segments
    let req = Request::builder()
        .uri("/api/segments?page=1&page_size=10")
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. 测试 POST /api/segments/update
    let update_body = json!({
        "id": 1,
        "target_zh": "我们终于抵达了。",
        "stage": 4
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/segments/update")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&update_body)?))?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    // 5. 测试 POST /api/qa/check
    let bad_qa_body = json!({
        "text": "测试全角波浪号～"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/qa/check")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&bad_qa_body)?))?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    // 6. 测试 POST /api/qa/fix
    let fix_body = json!({
        "text": "测试全角波浪号～"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/qa/fix")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&fix_body)?))?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}
