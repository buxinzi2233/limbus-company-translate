use crate::embedded_assets::EmbeddedAssets;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Query, State},
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use loc_core::{SaveTranslation, Store, StoreError};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub project_id: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
    pub limit: Option<usize>,
}
#[derive(Deserialize)]
pub struct SegmentsQuery {
    pub category: Option<loc_core::ResourceCategory>,
    pub selection: Option<loc_core::SegmentSelection>,
    pub file_id: Option<i64>,
    pub stage: Option<i32>,
    pub search: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}
#[derive(Deserialize)]
pub struct UpdateSegmentRequest {
    pub project_id: String,
    pub id: i64,
    pub target_zh: String,
    pub stage: i32,
    pub expected_updated_at: DateTime<Utc>,
}
#[derive(Deserialize)]
pub struct QaCheckRequest {
    pub text: String,
    pub kr: String,
}
#[derive(Deserialize)]
pub struct FixRequest {
    pub text: String,
}

struct ApiError(StatusCode, String);
impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        if error.downcast_ref::<loc_core::ScopeError>().is_some() {
            return Self(StatusCode::CONFLICT, format!("{error:#}"));
        }
        let status = match error.downcast_ref::<StoreError>() {
            Some(StoreError::NotFound(_)) => StatusCode::NOT_FOUND,
            Some(StoreError::Locked(_)) => StatusCode::FORBIDDEN,
            Some(StoreError::Conflict(_) | StoreError::Reviewed(_)) => StatusCode::CONFLICT,
            None | Some(StoreError::Poisoned) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self(status, format!("{error:#}"))
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"status":"error", "message":self.1}))).into_response()
    }
}

pub fn create_router(store: Store) -> Router {
    create_scoped_router(store, None)
}

pub fn create_scoped_router(store: Store, project_id: Option<String>) -> Router {
    Router::new()
        .route("/api/status", get(api_status))
        .route("/api/files", get(api_files))
        .route("/api/audit", get(api_audit))
        .route("/api/segments", get(api_segments))
        .route("/api/review-queue", get(api_review_queue))
        .route("/api/segments/update", post(api_update_segment))
        .route("/api/search", get(api_search))
        .route("/api/references/status", get(api_reference_status))
        .route("/api/references/search", get(api_reference_search))
        .route("/api/tm/stats", get(api_tm_stats))
        .route("/api/tm/search", get(api_tm_search))
        .route("/api/qa/terminology", post(api_terminology))
        .route("/api/qa/check", post(api_qa_check))
        .route("/api/qa/fix", post(api_qa_fix))
        .fallback(static_handler)
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(axum::middleware::from_fn(local_requests_only))
        .with_state(Arc::new(AppState { store, project_id }))
}

async fn local_requests_only(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    for name in [header::HOST, header::ORIGIN] {
        if let Some(value) = request.headers().get(&name) {
            let local = value
                .to_str()
                .ok()
                .and_then(|value| {
                    let url = if name == header::HOST {
                        format!("http://{value}")
                    } else {
                        value.to_string()
                    };
                    url.parse::<Uri>().ok()
                })
                .is_some_and(|uri| matches!(uri.host(), Some("localhost" | "127.0.0.1" | "[::1]")));
            if !local {
                return ApiError(
                    StatusCode::FORBIDDEN,
                    format!("Non-loopback {name} is not allowed"),
                )
                .into_response();
            }
        }
    }
    next.run(request).await
}

fn selected_project(state: &AppState) -> Result<Option<loc_core::Project>, ApiError> {
    match &state.project_id {
        Some(id) => state.store.get_project(id)?.map(Some).ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                format!("Project {id} does not exist"),
            )
        }),
        None => Ok(state.store.get_latest_project()?),
    }
}

fn active_project(state: &AppState) -> Result<loc_core::Project, ApiError> {
    selected_project(state)?.ok_or_else(|| {
        ApiError(
            StatusCode::CONFLICT,
            "No active project; run lct init first".into(),
        )
    })
}

async fn api_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project = selected_project(&state)?;
    let stats = state
        .store
        .get_project_stats(project.as_ref().map_or("", |p| p.id.as_str()))?;
    Ok(Json(
        json!({"status":"ok", "system":"LCT", "version":env!("CARGO_PKG_VERSION"), "project":project, "stats":stats, "task_scope":state.store.task_scope(project.as_ref().map_or("", |p| p.id.as_str()))?}),
    ))
}
#[derive(Deserialize)]
struct AuditQuery {
    category: Option<loc_core::ResourceCategory>,
    operations_before: Option<i64>,
    before: Option<i64>,
    imports_before: Option<i64>,
    corpus_offset: Option<usize>,
}

async fn api_audit(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if query.operations_before.is_some_and(|id| id <= 0)
        || query.before.is_some_and(|id| id <= 0)
        || query.imports_before.is_some_and(|id| id <= 0)
        || query.corpus_offset.is_some_and(|offset| offset > 1_000_000)
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid audit pagination cursor".into(),
        ));
    }
    let project = active_project(&state)?;
    let operations = state
        .store
        .file_operations(&project.id, query.operations_before, 20)?;
    let next_operations = if operations.len() == 20 {
        operations.last().map(|row| row.id)
    } else {
        None
    };
    let events = state.store.translation_events_by_category(
        &project.id,
        query.before,
        20,
        query.category,
    )?;
    let imports = state
        .store
        .source_imports(&project.id, query.imports_before, 20)?;
    let offset = query.corpus_offset.unwrap_or(0);
    let corpora = state.store.corpus_sources(offset, 20)?;
    let next_events = if events.len() == 20 {
        events.last().map(|row| row.id)
    } else {
        None
    };
    let next_imports = if imports.len() == 20 {
        imports.last().map(|row| row.id)
    } else {
        None
    };
    let next_corpora = if corpora.len() == 20 {
        Some(offset + 20)
    } else {
        None
    };
    Ok(Json(
        json!({"status":"ok", "project":project, "events":events, "imports":imports,
        "corpora":corpora, "next_events":next_events, "next_imports":next_imports, "next_corpora":next_corpora, "operations":operations, "next_operations":next_operations}),
    ))
}

async fn api_files(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project = active_project(&state)?;
    let files = state.store.get_files(&project.id)?;
    Ok(Json(
        json!({"status":"ok", "total":files.len(), "files":files}),
    ))
}
async fn api_segments(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SegmentsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project = active_project(&state)?;
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(50);
    if page == 0
        || !(1..=200).contains(&page_size)
        || params
            .stage
            .is_some_and(|stage| !matches!(stage, 0..=5 | 9))
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Require page >= 1, page_size in 1..=200 and stage in 0,1,2,3,4,9".into(),
        ));
    }
    let offset = (page - 1)
        .checked_mul(page_size)
        .filter(|offset| *offset <= i64::MAX as usize)
        .ok_or_else(|| {
            ApiError(
                StatusCode::BAD_REQUEST,
                "Pagination offset exceeds SQLite integer range".into(),
            )
        })?;
    let (total, items) = state.store.query_categorized_segments(
        &project.id,
        params
            .selection
            .unwrap_or(loc_core::SegmentSelection::Inventory),
        (
            params.file_id,
            &params.stage.into_iter().collect::<Vec<_>>(),
            params.search.as_deref(),
        ),
        params.category,
        offset,
        page_size,
    )?;
    Ok(Json(
        json!({"status":"ok", "total":total, "page":page, "page_size":page_size, "items":items}),
    ))
}
async fn api_review_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SegmentsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project = active_project(&state)?;
    let (total, items) = state.store.review_queue_selected(
        &project.id,
        params
            .selection
            .unwrap_or(loc_core::SegmentSelection::Inventory),
        params.category,
    )?;
    Ok(Json(
        json!({"status":"ok", "total":total, "items":items, "page":1, "page_size":100}),
    ))
}

async fn api_update_segment(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateSegmentRequest>,
) -> Result<Response, ApiError> {
    if req.stage != 4 || req.target_zh.trim().is_empty() || req.target_zh.len() > 100_000 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Reviewed saves require stage=4 and target_zh of 1..100000 bytes".into(),
        ));
    }
    let project = active_project(&state)?;
    if req.project_id != project.id {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Active project changed; reload before saving".into(),
        ));
    }
    let current = state.store.get_segment(&project.id, req.id)?;
    if current.is_locked || current.stage == loc_core::Stage::UpstreamFrozen {
        return Err(anyhow::Error::from(StoreError::Locked(req.id)).into());
    }
    if current.updated_at != req.expected_updated_at {
        return Err(anyhow::Error::from(StoreError::Conflict(req.id)).into());
    }
    let issues = loc_qa::check_segment_pair(&current.source_kr, &req.target_zh);
    if !issues.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"status":"error", "message":"QA rejected the translation; no changes were saved", "qa_passed":false, "issues":issues}))).into_response());
    }
    let item = state.store.save_reviewed_translation(&SaveTranslation {
        project_id: &project.id,
        id: req.id,
        expected_updated_at: req.expected_updated_at,
        target_zh: &req.target_zh,
    })?;
    Ok(
        Json(json!({"status":"ok", "id":item.id, "qa_passed":true, "issues":issues, "item":item}))
            .into_response(),
    )
}
fn search_limit(params: &SearchParams) -> Result<usize, ApiError> {
    let limit = params.limit.unwrap_or(20);
    if !(1..=200).contains(&limit) || params.q.len() > 4096 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Require limit in 1..=200 and query <= 4096 bytes".into(),
        ));
    }
    Ok(limit)
}
async fn api_tm_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        json!({"status":"ok", "tm_entries_count":state.store.get_tm_count()?}),
    ))
}
async fn api_tm_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let results = state
        .store
        .search_tm_fts(&params.q, search_limit(&params)?)?;
    Ok(Json(
        json!({"status":"ok", "query":params.q, "total":results.len(), "results":results}),
    ))
}
async fn api_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project = active_project(&state)?;
    let limit = search_limit(&params)?;
    let segments = state
        .store
        .search_segments_fts(&project.id, &params.q, limit)?;
    let tm_entries = state.store.search_tm_fts(&params.q, limit)?;
    Ok(Json(
        json!({"status":"ok", "query":params.q, "total_found":segments.len()+tm_entries.len(), "segments":segments, "tm_entries":tm_entries}),
    ))
}
async fn api_qa_check(Json(req): Json<QaCheckRequest>) -> Json<serde_json::Value> {
    let issues = loc_qa::check_segment_pair(&req.kr, &req.text);
    Json(json!({"passed":issues.is_empty(), "issue_count":issues.len(), "issues":issues}))
}
async fn api_qa_fix(Json(req): Json<FixRequest>) -> Json<serde_json::Value> {
    Json(json!({"original":req.text, "fixed":loc_qa::auto_fix_target_zh(&req.text)}))
}

async fn static_handler(uri: Uri) -> Response {
    if uri.path().starts_with("/api/") {
        return ApiError(
            StatusCode::NOT_FOUND,
            format!("Unknown API route: {}", uri.path()),
        )
        .into_response();
    }
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match EmbeddedAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(mime.as_ref()).unwrap(),
                )
                .body(Body::from(content.data))
                .unwrap()
        }
        None => {
            // SPA Fallback: 任何未找到的路由都返回 index.html
            match EmbeddedAssets::get("index.html") {
                Some(content) => Response::builder()
                    .header(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("text/html; charset=utf-8"),
                    )
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

async fn api_reference_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<loc_core::references::ReferenceStatus>, ApiError> {
    let project = state.project_id.clone().ok_or_else(|| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "Reference access requires an explicitly bound project".into(),
        )
    })?;
    tokio::task::spawn_blocking(move || state.store.reference_status(&project))
        .await
        .map_err(|error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(ApiError::from)
}

async fn api_reference_search(
    State(state): State<Arc<AppState>>,
    Query(input): Query<SearchParams>,
) -> Result<Json<loc_core::references::ReferenceResults>, ApiError> {
    let project = state.project_id.clone().ok_or_else(|| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "Reference access requires an explicitly bound project".into(),
        )
    })?;
    let limit = input.limit.ok_or_else(|| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "Reference search requires an explicit limit (1-50)".into(),
        )
    })?;
    tokio::task::spawn_blocking(move || state.store.search_references(&project, &input.q, limit))
        .await
        .map_err(|error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(|error| {
            let status = match error.downcast_ref::<loc_core::references::ReferenceError>() {
                Some(loc_core::references::ReferenceError::InvalidQuery) => StatusCode::BAD_REQUEST,
                Some(loc_core::references::ReferenceError::NotConfigured) => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
                Some(loc_core::references::ReferenceError::ProjectMismatch { .. }) => {
                    StatusCode::FORBIDDEN
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            ApiError(status, format!("{error:#}"))
        })
}

#[derive(Deserialize)]
struct TermCheckRequest {
    id: i64,
    target_zh: String,
}

async fn api_terminology(
    State(state): State<Arc<AppState>>,
    Json(input): Json<TermCheckRequest>,
) -> Result<Json<loc_qa::terminology::TermAdviceReport>, ApiError> {
    if input.id < 1 || input.target_zh.trim().is_empty() || input.target_zh.len() > 100_000 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Terminology check requires positive id and 1-100000 byte target_zh".into(),
        ));
    }
    let project = state.project_id.clone().ok_or_else(|| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "Terminology check requires bound project".into(),
        )
    })?;
    tokio::task::spawn_blocking(move || {
        loc_qa::terminology::check_term_advice(&state.store, &project, input.id, &input.target_zh)
    })
    .await
    .map_err(|error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
    .map(Json)
    .map_err(ApiError::from)
}
