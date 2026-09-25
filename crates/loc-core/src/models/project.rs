use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub live_client_kr_dir: Option<String>,
    pub upstream_llc_dir: Option<String>,
    pub workspace_zh_dir: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            live_client_kr_dir: None,
            upstream_llc_dir: None,
            workspace_zh_dir: None,
            created_at: now,
            updated_at: now,
        }
    }
}
