use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Fatal,
    Error,
    Warn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Issue {
    pub rule_id: String,
    pub severity: IssueSeverity,
    pub message: String,
    pub pointer: Option<String>,
    pub auto_fixable: bool,
    pub suggested_fix: Option<String>,
}

impl Issue {
    pub fn new(
        rule_id: impl Into<String>,
        severity: IssueSeverity,
        message: impl Into<String>,
        auto_fixable: bool,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            severity,
            message: message.into(),
            pointer: None,
            auto_fixable,
            suggested_fix: None,
        }
    }
}
