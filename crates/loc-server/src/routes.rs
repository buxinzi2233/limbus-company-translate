use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use loc_core::Store;
use loc_qa::check_segment_pair;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::embedded_assets::EmbeddedAssets;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct SegmentsQuery {
    pub file_id: Option<i64>,
    pub stage: Option<i32>,
    pub search: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Deserialize)]
pub struct UpdateSegmentRequest {
    pub id: i64,
    pub target_zh: String,
    pub stage: Option<i32>,
}

#[derive(Deserialize)]
pub struct QaCheckRequest {
    pub text: String,
    pub kr: Option<String>,
}

#[derive(Deserialize)]
pub struct FixRequest {
    pub text: String,
}

pub fn create_router(store: Store) -> Router {
    let state = Arc::new(AppState { store });

    Router::new()
        // API 路由
        .route("/api/status", get(api_status))
        .route("/api/files", get(api_files))
        .route("/api/segments", get(api_segments))
        .route("/api/segments/update", post(api_update_segment))
        .route("/api/search", get(api_search))
        .route("/api/tm/stats", get(api_tm_stats))
        .route("/api/tm/search", get(api_tm_search))
        .route("/api/qa/check", post(api_qa_check))
        .route("/api/qa/fix", post(api_qa_fix))
        // 静态文件与 SPA 路由
        .fallback(static_handler)
        .with_state(state)
}

async fn api_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let project = state.store.get_latest_project().ok().flatten();
    let stats = state.store.get_project_stats().ok();
    Json(json!({
        "status": "ok",
        "system": "LCT (Limbus Company Translate)",
        "version": "0.2.0-LCT",
        "project": project,
        "stats": stats,
    }))
}

async fn api_files(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let project = state.store.get_latest_project().ok().flatten();
    let project_id = project.as_ref().map(|p| p.id.as_str()).unwrap_or("limbus-active");
    match state.store.get_files(project_id) {
        Ok(files) => Json(json!({
            "status": "ok",
            "total": files.len(),
            "files": files,
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": e.to_string(),
        })),
    }
}

async fn api_segments(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SegmentsQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * page_size;

    match state.store.query_segments_paged(
        params.file_id,
        params.stage,
        params.search.as_deref(),
        offset,
        page_size,
    ) {
        Ok((total, items)) => Json(json!({
            "status": "ok",
            "total": total,
            "page": page,
            "page_size": page_size,
            "items": items,
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": e.to_string(),
        })),
    }
}

async fn api_update_segment(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateSegmentRequest>,
) -> impl IntoResponse {
    // 自动进行 QA 校验并记录问题
    let issues = check_segment_pair("", &req.target_zh);
    let qa_passed = issues.is_empty();
    let qa_issues_str = if qa_passed {
        None
    } else {
        Some(serde_json::to_string(&issues).unwrap_or_default())
    };

    match state.store.update_segment_translation(
        req.id,
        &req.target_zh,
        req.stage,
        qa_passed,
        qa_issues_str.as_deref(),
    ) {
        Ok(_) => Json(json!({
            "status": "ok",
            "id": req.id,
            "qa_passed": qa_passed,
            "issues": issues,
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": e.to_string(),
        })),
    }
}

async fn api_tm_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.store.get_tm_count() {
        Ok(count) => Json(json!({
            "status": "ok",
            "tm_entries_count": count,
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": e.to_string(),
        })),
    }
}

async fn api_tm_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20);
    match state.store.search_tm_fts(&params.q, limit) {
        Ok(results) => Json(json!({
            "status": "ok",
            "query": params.q,
            "total": results.len(),
            "results": results,
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": e.to_string(),
        })),
    }
}

async fn api_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20);
    let seg_results = state.store.search_segments_fts(&params.q, limit).unwrap_or_default();
    let tm_results = state.store.search_tm_fts(&params.q, limit).unwrap_or_default();

    Json(json!({
        "status": "ok",
        "query": params.q,
        "segments": seg_results,
        "tm_entries": tm_results,
        "total_found": seg_results.len() + tm_results.len(),
    }))
}

async fn api_qa_check(Json(req): Json<QaCheckRequest>) -> impl IntoResponse {
    let kr = req.kr.as_deref().unwrap_or("");
    let issues = loc_qa::check_segment_pair(kr, &req.text);
    Json(json!({
        "passed": issues.is_empty(),
        "issue_count": issues.len(),
        "issues": issues,
    }))
}

async fn api_qa_fix(Json(req): Json<FixRequest>) -> impl IntoResponse {
    let fixed = loc_qa::auto_fix_target_zh(&req.text);
    Json(json!({
        "original": req.text,
        "fixed": fixed,
    }))
}

async fn static_handler(uri: Uri) -> Response {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match EmbeddedAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())
                .body(Body::from(content.data))
                .unwrap()
        }
        None => {
            // SPA Fallback: 任何未找到的路由都返回 index.html
            match EmbeddedAssets::get("index.html") {
                Some(content) => Response::builder()
                    .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
                    .body(Body::from(content.data))
                    .unwrap(),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("404 Not Found"))
                    .unwrap(),
            }
        }
    }
}
