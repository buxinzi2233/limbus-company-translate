use crate::types::{FailureCode, ToolFailure};
use rmcp::Json;

pub fn failure(code: FailureCode, message: impl Into<String>) -> Json<ToolFailure> {
    Json(ToolFailure {
        code,
        message: message.into(),
        issues: Vec::new(),
    })
}

pub fn from_error(error: anyhow::Error) -> Json<ToolFailure> {
    use loc_core::StoreError;
    if let Some(access) = error.downcast_ref::<loc_core::DraftAccessError>() {
        let code = match access {
            loc_core::DraftAccessError::Existing(_) => FailureCode::ExistingTranslation,
            _ => FailureCode::WriteForbidden,
        };
        return failure(code, format!("{error:#}"));
    }
    if let Some(scope) = error.downcast_ref::<loc_core::ScopeError>() {
        let code = match scope {
            loc_core::ScopeError::Stale(_) => FailureCode::Conflict,
            loc_core::ScopeError::Missing(_) => FailureCode::NotFound,
        };
        return failure(code, format!("{error:#}"));
    }
    use loc_qa::SubmissionError;
    if let Some(reference) = error.downcast_ref::<loc_core::references::ReferenceError>() {
        use loc_core::references::ReferenceError;
        let code = match reference {
            ReferenceError::InvalidQuery => FailureCode::InvalidInput,
            ReferenceError::ProjectMismatch { .. } => FailureCode::ProjectMismatch,
            ReferenceError::NotConfigured | ReferenceError::InvalidDatabase(_) => {
                FailureCode::Internal
            }
        };
        return failure(code, format!("{error:#}"));
    }
    let code = match error.downcast_ref::<StoreError>() {
        Some(StoreError::NotFound(_)) => FailureCode::NotFound,
        Some(StoreError::Locked(_)) => FailureCode::Locked,
        Some(StoreError::Conflict(_)) => FailureCode::Conflict,
        Some(StoreError::Reviewed(_)) => FailureCode::Reviewed,
        Some(StoreError::Poisoned) => FailureCode::Internal,
        None => match error.downcast_ref::<SubmissionError>() {
            Some(SubmissionError::InvalidInput) => FailureCode::InvalidInput,
            Some(SubmissionError::QaRejected(issues)) => {
                return Json(ToolFailure {
                    code: FailureCode::QaRejected,
                    message:
                        "QA rejected the draft; nothing was saved. Correct the reported issues."
                            .into(),
                    issues: issues.iter().cloned().map(Into::into).collect(),
                })
            }
            None => FailureCode::Internal,
        },
    };
    failure(code, format!("{error:#}"))
}
