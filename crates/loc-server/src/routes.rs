use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use loc_core::Store;
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
        .route("/api/search", get(api_search))
        .route("/api/qa/check", post(api_qa_check))
        .route("/api/qa/fix", post(api_qa_fix))
        // 静态文件与 SPA 路由
        .fallback(static_handler)
        .with_state(state)
}

async fn api_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.store.get_latest_project() {
        Ok(project) => Json(json!({
            "status": "ok",
            "version": "0.1.0",
            "project": project
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

async fn api_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20);
    match state.store.search_segments_fts(&params.q, limit) {
        Ok(results) => Json(json!({
            "status": "ok",
            "total": results.len(),
            "results": results
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

async fn api_qa_check(Json(req): Json<QaCheckRequest>) -> impl IntoResponse {
    let kr = req.kr.as_deref().unwrap_or("");
    let issues = loc_qa::check_segment_pair(kr, &req.text);
    Json(json!({
        "passed": issues.is_empty(),
        "issue_count": issues.len(),
        "issues": issues
    }))
}

async fn api_qa_fix(Json(req): Json<FixRequest>) -> impl IntoResponse {
    let fixed = loc_qa::auto_fix_target_zh(&req.text);
    Json(json!({
        "original": req.text,
        "fixed": fixed
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
