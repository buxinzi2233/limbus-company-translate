use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TMEntry {
    pub id: i64,
    pub hash_kr: String,
    pub source_kr: String,
    pub target_zh: String,
    pub file_scope: Option<String>,
    pub speaker: Option<String>,
    pub source_origin: String,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
}

impl TMEntry {
    pub fn new(
        hash_kr: impl Into<String>,
        source_kr: impl Into<String>,
        target_zh: impl Into<String>,
        source_origin: impl Into<String>,
        priority: i32,
    ) -> Self {
        Self {
            id: 0,
            hash_kr: hash_kr.into(),
            source_kr: source_kr.into(),
            target_zh: target_zh.into(),
            file_scope: None,
            speaker: None,
            source_origin: source_origin.into(),
            priority,
            created_at: Utc::now(),
        }
    }
}
