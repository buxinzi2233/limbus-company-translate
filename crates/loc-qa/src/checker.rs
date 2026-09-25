use crate::issue::Issue;
use crate::rules::*;

/// 对单个条目的中文译文与韩文原文进行全量门禁与语义稽核
pub fn check_segment_pair(source_kr: &str, target_zh: &str) -> Vec<Issue> {
    let mut issues = Vec::new();

    // 1. L02: 绝对封杀全角波浪号
    if let Some(issue) = check_l02_tilde(target_zh) {
        issues.push(issue);
    }

    // 2. L03: 占位符 1:1 对齐
    if let Some(issue) = check_l03_placeholder(source_kr, target_zh) {
        issues.push(issue);
    }

    // 3. L04: 关键词方括号后半角空格
    if let Some(issue) = check_l04_space(target_zh) {
        issues.push(issue);
    }

    // 4. L05: TextMeshPro 标签配对与 LIFO 闭合
    if let Some(issue) = check_l05_tmp_tags(target_zh) {
        issues.push(issue);
    }

    // 5. L08: 韩文未译残留检测
    if let Some(issue) = check_l08_hangul_residual(target_zh) {
        issues.push(issue);
    }

    // 6. 语义硬拦截: 跨语言数值失配检测
    if let Some(issue) = check_numeric_invariant(source_kr, target_zh) {
        issues.push(issue);
    }

    // 7. 语义硬拦截: 机制极性反转检测 (以下 vs 以上)
    if let Some(issue) = check_polarity_conflict(source_kr, target_zh) {
        issues.push(issue);
    }

    issues
}
