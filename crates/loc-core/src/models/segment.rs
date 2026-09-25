use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 词条在本地化流水线中的生命周期阶段 (Stage)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum Stage {
    /// 0: 全新提取的韩文生肉，尚未翻译
    Untranslated = 0,
    /// 1: TM 翻译记忆库 100% 精准命中，自动填入
    TmExact = 1,
    /// 2: 官方新版本修改了原文，保留旧译文供参考，展示字符级 Diff
    OutdatedDiff = 2,
    /// 3: AI 批量草拟完成，已通过机器门禁初检
    AiDrafted = 3,
    /// 4: 人类指挥官审核通过，已锁定
    Reviewed = 4,
    /// 9: 属于上游零协会已汉化的权威基准，物理只读锁定，严禁污染
    UpstreamFrozen = 9,
}

impl From<i32> for Stage {
    fn from(val: i32) -> Self {
        match val {
            0 => Stage::Untranslated,
            1 => Stage::TmExact,
            2 => Stage::OutdatedDiff,
            3 => Stage::AiDrafted,
            4 => Stage::Reviewed,
            9 => Stage::UpstreamFrozen,
            _ => Stage::Untranslated,
        }
    }
}

/// 标准翻译条目模型 (Segment)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: i64,
    pub project_id: String,
    pub file_id: i64,
    pub key_path: String,
    pub source_kr: String,
    pub previous_kr: Option<String>,
    pub target_zh: Option<String>,
    pub speaker: Option<String>,
    pub context_info: Option<String>,
    pub stage: Stage,
    pub qa_passed: bool,
    pub qa_issues: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Segment {
    pub fn new(
        file_id: i64,
        project_id: impl Into<String>,
        key_path: impl Into<String>,
        source_kr: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            project_id: project_id.into(),
            file_id,
            key_path: key_path.into(),
            source_kr: source_kr.into(),
            previous_kr: None,
            target_zh: None,
            speaker: None,
            context_info: None,
            stage: Stage::Untranslated,
            qa_passed: true,
            qa_issues: None,
            created_at: now,
            updated_at: now,
        }
    }
}
