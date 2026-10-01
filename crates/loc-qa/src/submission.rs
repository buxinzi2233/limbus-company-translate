use crate::{check_segment_pair, Issue};
use anyhow::Result;
use chrono::{DateTime, Utc};
use loc_core::{SaveTranslation, SegmentWithFile, Store};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DraftSubmission {
    pub project_id: String,
    pub id: i64,
    pub expected_updated_at: DateTime<Utc>,
    pub target_zh: String,
    pub producer: String,
}

#[derive(Debug, Error)]
pub enum SubmissionError {
    #[error("Draft requires a project, positive segment ID, 1..128 byte producer and 1..100000 byte translation")]
    InvalidInput,
    #[error("QA rejected draft; no changes were saved: {0:?}")]
    QaRejected(Vec<Issue>),
}

/// Validate against the persisted source; CAS rejects any change during QA.
pub fn submit_ai_draft(
    store: &Store,
    input: &DraftSubmission,
    permit: Option<&loc_core::DraftPermit>,
) -> Result<SegmentWithFile> {
    if input.project_id.trim().is_empty()
        || input.id <= 0
        || input.producer.trim().is_empty()
        || input.producer.len() > 128
        || input.target_zh.trim().is_empty()
        || input.target_zh.len() > 100_000
    {
        return Err(SubmissionError::InvalidInput.into());
    }
    let current = store.get_segment(&input.project_id, input.id)?;
    if current.updated_at != input.expected_updated_at {
        return Err(loc_core::StoreError::Conflict(input.id).into());
    }
    let issues = check_segment_pair(&current.source_kr, &input.target_zh);
    if !issues.is_empty() {
        return Err(SubmissionError::QaRejected(issues).into());
    }
    store.save_ai_draft(
        &SaveTranslation {
            project_id: &input.project_id,
            id: input.id,
            expected_updated_at: input.expected_updated_at,
            target_zh: &input.target_zh,
        },
        &input.producer,
        permit,
    )
}
