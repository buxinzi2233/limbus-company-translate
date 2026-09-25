use anyhow::Result;
use loc_core::Store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchTier {
    /// 101% 上下文匹配 (同文件同指针路径同原文)
    Context101,
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
pub fn match_segment(source_kr: &str, file_scope: Option<&str>, store: &Store) -> Result<Option<MatchCandidate>> {
    let clean_kr = source_kr.trim();
    if clean_kr.is_empty() {
        return Ok(None);
    }

    let h = hash_text(clean_kr);

    // 1. 尝试 100%/101% 精确哈希匹配
    if let Some(entry) = store.query_tm_exact(&h)? {
        let tier = if let (Some(req_scope), Some(entry_scope)) = (file_scope, &entry.file_scope) {
            if req_scope == entry_scope {
                MatchTier::Context101
            } else {
                MatchTier::Exact100
            }
        } else {
            MatchTier::Exact100
        };

        return Ok(Some(MatchCandidate {
            tier,
            target_zh: entry.target_zh,
            source_kr: entry.source_kr,
            source_origin: entry.source_origin,
            priority: entry.priority,
        }));
    }

    Ok(None)
}
