use anyhow::Result;
use loc_qa::{auto_fix_target_zh, check_segment_pair};

#[test]
fn test_qa_comprehensive_check_and_auto_fix() -> Result<()> {
    let kr = "체력이 20% 이하일 때, 3턴 동안 피해량 50% 증가 및 [호흡] 2단계 획득";
    // 劣质译文：全角波浪号 + 漏空格 + 数值3写成2 + 极性写成以上 + 未闭合color
    let bad_zh = "<color=#ff0000>体力在20%以上时，在2回合内造成10～20点伤害并获得2层[呼吸法]状态";

    let issues = check_segment_pair(kr, bad_zh);
    assert!(!issues.is_empty(), "应当捕获多个错误");

    let rule_ids: Vec<&str> = issues.iter().map(|i| i.rule_id.as_str()).collect();
    assert!(rule_ids.contains(&"L02"), "必须捕获 L02 全角波浪号");
    assert!(rule_ids.contains(&"L04"), "必须捕获 L04 缺失空格");
    assert!(rule_ids.contains(&"L05"), "必须捕获 L05 未闭合 color 标签");
    assert!(rule_ids.contains(&"NUMERIC_MISMATCH"), "必须捕获数值失配");
    assert!(rule_ids.contains(&"POLARITY_CONFLICT"), "必须捕获极性反转 (以下翻成以上)");

    // 验证可序列化为 JSON (供 AI 作为自愈诊断输入)
    let json_issues = serde_json::to_string_pretty(&issues)?;
    assert!(json_issues.contains("\"rule_id\": \"L02\""));

    // 运行自动修复
    let fixed_text = auto_fix_target_zh(bad_zh);
    assert!(fixed_text.contains("10~20")); // 波浪号被修复
    assert!(fixed_text.contains("[呼吸法] 状态")); // 空格被补齐

    Ok(())
}
