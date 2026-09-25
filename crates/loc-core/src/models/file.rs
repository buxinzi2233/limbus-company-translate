use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub project_id: String,
    pub relative_path: String,
    pub category: String,
    pub total_segments: i32,
    pub translated_segments: i32,
    pub reviewed_segments: i32,
    pub is_locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FileRecord {
    pub fn new(project_id: impl Into<String>, relative_path: impl Into<String>, category: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            project_id: project_id.into(),
            relative_path: relative_path.into(),
            category: category.into(),
            total_segments: 0,
            translated_segments: 0,
            reviewed_segments: 0,
            is_locked: false,
            created_at: now,
            updated_at: now,
        }
    }
}
