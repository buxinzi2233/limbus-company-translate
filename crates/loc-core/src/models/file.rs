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
    pub fn new(
        project_id: impl Into<String>,
        relative_path: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
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

/// Classify normalized game resources by purpose, not by their container directory.
/// Counts remain localizable fields, including names and repeated speaker labels.
pub fn resource_category(relative_path: &str) -> &'static str {
    let path = relative_path.replace('\\', "/");
    if ["StoryData/", "PersonalityVoiceDlg/", "EGOVoiceDig/"]
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || [
            "RPGSystem/rpg-loc-dialogue-",
            "RPGSystem/rpg-loc-narration-",
            "RPGSystem/rpg-loc-quest-",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        return "story";
    }
    let family = path.split(['-', '_', '.']).next().unwrap_or("");
    match family {
        "AbnormalityGuides" => "story",
        "Skills"
        | "Bufs"
        | "Passives"
        | "BattleKeywords"
        | "BattleSpeechBubbleDlg"
        | "BattleResultHint"
        | "PanicInfo"
        | "Enemies"
        | "Assist" => "battle",
        _ => "system",
    }
}

/// Domain filters use the same persisted categories as progress statistics.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceCategory {
    Battle,
    Story,
    System,
}

impl ResourceCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Battle => "battle",
            Self::Story => "story",
            Self::System => "system",
        }
    }
}
