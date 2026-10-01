use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Term {
    pub id: i64,
    pub source_kr: String,
    pub target_zh: String,
    pub category: String,
    pub forbidden_terms: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Term {
    pub fn new(
        source_kr: impl Into<String>,
        target_zh: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            id: 0,
            source_kr: source_kr.into(),
            target_zh: target_zh.into(),
            category: category.into(),
            forbidden_terms: None,
            description: None,
            created_at: Utc::now(),
        }
    }
}
