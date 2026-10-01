use anyhow::Result;
use loc_core::Store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchTier {
    /// Exact source and file match; no pointer-level identity is implied.
    FileExact,
    /// Conflicting translations require review and must not be auto-filled.
    Ambiguous,
    /// 100% 原文跨文件精确命中
    Exact100,
    /// 模糊匹配推荐 (70% ~ 99% 相似度)
    Fuzzy(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchCandidate {
    pub tier: MatchTier,
    pub target_zh: String,
    pub source_kr: String,
    pub source_origin: String,
    pub priority: i32,
}

/// 计算简单的字符串哈希 (用于 100% 精确秒级直查)
pub fn hash_text(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut s = DefaultHasher::new();
    text.trim().hash(&mut s);
    format!("{:x}", s.finish())
}

/// 匹配器：根据待译韩文从 Store TM 库中寻找最佳匹配
pub fn match_segment(
    source_kr: &str,
    file_scope: Option<&str>,
    store: &Store,
) -> Result<Option<MatchCandidate>> {
    let clean_kr = source_kr.trim();
    if clean_kr.is_empty() {
        return Ok(None);
    }

    let h = hash_text(clean_kr);

    let exact: Vec<_> = store
        .query_tm_candidates(&h)?
        .into_iter()
        .filter(|entry| entry.source_kr.trim() == clean_kr && !entry.target_zh.trim().is_empty())
        .collect();
    let has_file_match = file_scope.is_some()
        && exact
            .iter()
            .any(|entry| entry.file_scope.as_deref() == file_scope);
    let candidates: Vec<_> = exact
        .into_iter()
        .filter(|entry| !has_file_match || entry.file_scope.as_deref() == file_scope)
        .collect();
    let Some(entry) = candidates.first() else {
        return Ok(None);
    };
    let ambiguous = candidates
        .iter()
        .any(|candidate| candidate.target_zh != entry.target_zh);
    let tier = if ambiguous {
        MatchTier::Ambiguous
    } else if has_file_match {
        MatchTier::FileExact
    } else {
        MatchTier::Exact100
    };
    Ok(Some(MatchCandidate {
        tier,
        target_zh: entry.target_zh.clone(),
        source_kr: entry.source_kr.clone(),
        source_origin: entry.source_origin.clone(),
        priority: entry.priority,
    }))
}
