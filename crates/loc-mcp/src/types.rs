use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
pub struct ProjectInput {
    pub project_id: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ListInput {
    pub selection: Option<loc_core::SegmentSelection>,
    pub project_id: String,
    #[schemars(range(min = 0, max = 1000000))]
    pub offset: usize,
    #[schemars(range(min = 1, max = 50))]
    pub limit: usize,
    pub stage: Option<i32>,
    #[schemars(length(max = 256))]
    pub search: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SegmentInput {
    pub project_id: String,
    #[schemars(range(min = 1))]
    pub id: i64,
}

#[derive(Deserialize, JsonSchema)]
pub struct DraftInput {
    pub project_id: String,
    pub id: i64,
    pub expected_updated_at: DateTime<Utc>,
    #[schemars(length(min = 1, max = 100000))]
    pub target_zh: String,
    #[schemars(length(min = 1, max = 128))]
    pub producer: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CheckInput {
    pub project_id: String,
    pub id: i64,
    pub target_zh: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchInput {
    pub project_id: String,
    pub query: String,
    #[schemars(range(min = 1, max = 50))]
    pub limit: usize,
}

#[derive(Deserialize, JsonSchema)]
pub struct EventsInput {
    pub project_id: String,
    pub before_id: Option<i64>,
    #[schemars(range(min = 1, max = 50))]
    pub limit: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentView {
    pub id: i64,
    pub project_id: String,
    pub relative_path: String,
    pub key_path: String,
    pub source_kr: String,
    pub previous_kr: Option<String>,
    pub target_zh: Option<String>,
    pub speaker: Option<String>,
    pub context_info: Option<String>,
    pub stage: i32,
    pub is_locked: bool,
    pub qa_passed: bool,
    pub qa_issues: Option<String>,
    pub review_confirmed: bool,
    pub updated_at: DateTime<Utc>,
}
impl From<loc_core::SegmentWithFile> for SegmentView {
    fn from(row: loc_core::SegmentWithFile) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            relative_path: row.relative_path,
            key_path: row.key_path,
            source_kr: row.source_kr,
            previous_kr: row.previous_kr,
            target_zh: row.target_zh,
            speaker: row.speaker,
            context_info: row.context_info,
            stage: row.stage as i32,
            is_locked: row.is_locked,
            qa_passed: row.qa_passed,
            qa_issues: row.qa_issues,
            review_confirmed: row.review_confirmed,
            updated_at: row.updated_at,
        }
    }
}
#[derive(Serialize, JsonSchema)]
pub struct SegmentPage {
    pub total: usize,
    pub next_offset: Option<usize>,
    pub items: Vec<SegmentView>,
}
#[derive(Serialize, JsonSchema)]
pub struct ProjectStatus {
    pub draft_grant_id: Option<String>,
    pub task_scope: Option<loc_core::TaskScope>,
    pub project_id: String,
    pub name: String,
    pub source_directory: Option<String>,
    pub translation_directory: Option<String>,
    pub total: i64,
    pub translated: i64,
    pub reviewed: i64,
    pub pending_review: i64,
    pub shared_tm_entries: i64,
}
#[derive(Serialize, JsonSchema)]
pub struct MemoryMatch {
    pub id: i64,
    pub source_kr: String,
    pub target_zh: String,
    pub file_scope: Option<String>,
    pub speaker: Option<String>,
    pub source_origin: String,
    pub priority: i32,
}
#[derive(Serialize, JsonSchema)]
pub struct MemoryResults {
    pub scope: String,
    pub items: Vec<MemoryMatch>,
}
#[derive(Serialize, JsonSchema)]
pub struct QaIssue {
    pub rule_id: String,
    pub message: String,
}
impl From<loc_qa::Issue> for QaIssue {
    fn from(issue: loc_qa::Issue) -> Self {
        Self {
            rule_id: issue.rule_id,
            message: issue.message,
        }
    }
}
#[derive(Serialize, JsonSchema)]
pub struct QaResult {
    pub terminology: loc_qa::terminology::TermAdviceReport,
    pub id: i64,
    pub source_version: DateTime<Utc>,
    pub passed: bool,
    pub issues: Vec<QaIssue>,
}
#[derive(Serialize, JsonSchema)]
pub struct ReviewEvent {
    pub id: i64,
    pub segment_id: i64,
    pub kind: String,
    pub producer: String,
    pub source_kr: String,
    pub previous_target: Option<String>,
    pub target_zh: String,
    pub previous_stage: i32,
    pub resulting_stage: i32,
    pub created_at: DateTime<Utc>,
}
#[derive(Serialize, JsonSchema)]
pub struct EventPage {
    pub items: Vec<ReviewEvent>,
    pub next_before_id: Option<i64>,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    ProjectMismatch,
    InvalidInput,
    NotFound,
    Locked,
    Conflict,
    Reviewed,
    QaRejected,
    WriteForbidden,
    ExistingTranslation,
    Cancelled,
    ResponseTooLarge,
    Internal,
}
#[derive(Serialize, JsonSchema)]
pub struct ToolFailure {
    pub code: FailureCode,
    pub message: String,
    pub issues: Vec<QaIssue>,
}

#[derive(Serialize, JsonSchema)]
pub struct DraftReceipt {
    pub id: i64,
    pub stage: i32,
    pub qa_passed: bool,
    pub review_confirmed: bool,
    pub updated_at: DateTime<Utc>,
}
