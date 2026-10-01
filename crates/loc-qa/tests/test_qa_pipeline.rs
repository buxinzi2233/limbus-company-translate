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
    assert!(!rule_ids.contains(&"L04"));
    assert!(rule_ids.contains(&"L05"), "必须捕获 L05 未闭合 color 标签");
    assert!(rule_ids.contains(&"NUMERIC_MISMATCH"), "必须捕获数值失配");
    assert!(
        rule_ids.contains(&"POLARITY_CONFLICT"),
        "必须捕获极性反转 (以下翻成以上)"
    );

    // 验证可序列化为 JSON (供 AI 作为自愈诊断输入)
    let json_issues = serde_json::to_string_pretty(&issues)?;
    assert!(json_issues.contains("\"rule_id\": \"L02\""));

    // 运行自动修复
    let fixed_text = auto_fix_target_zh(bad_zh);
    assert!(fixed_text.contains("10~20")); // 波浪号被修复
    assert!(fixed_text.contains("[呼吸法]状态")); // Bracket syntax is preserved.

    Ok(())
}

#[test]
fn qa_preserves_formatted_placeholders_signs_and_renderer_attributes() {
    for (source, target, rule) in [
        ("피해 {0:N2}", "伤害{0:N3}", "L03"),
        ("피해 {0} {0}", "伤害{0}", "L03"),
        ("피해 {0,8:F2}", "伤害{0,8:F3}", "L03"),
        ("피해 {0}", "伤害{0", "L03"),
        ("피해 -20%", "伤害20%", "NUMERIC_MISMATCH"),
        ("피해 +20%", "伤害-20%", "NUMERIC_MISMATCH"),
        ("<color=#ff0000>피해 20%</color>", "伤害20%", "TMP_MISMATCH"),
        (
            "<sprite name=\"dice\">피해",
            "<sprite name=\"coin\">伤害",
            "TMP_MISMATCH",
        ),
        ("<b>피해</b>", "<b>伤害", "L05"),
    ] {
        assert!(
            check_segment_pair(source, target)
                .iter()
                .any(|issue| issue.rule_id == rule),
            "{source} -> {target}: expected {rule}"
        );
    }
    for (source, target) in [
        ("피해 {{0}} {0:N2}", "伤害{{0}} {0:N2}"),
        ("피해 -20%", "伤害−20%"),
        ("피해 10-20", "伤害10~20"),
        (
            "<font-weight=700>피해 20%</font-weight>",
            "<font-weight=700>伤害20%</font-weight>",
        ),
        ("<sprite name=\"dice\">피해", "<sprite name=\"dice\">伤害"),
        ("<noparse><b>피해</noparse>", "<noparse><b>伤害</noparse>"),
    ] {
        assert!(
            check_segment_pair(source, target).is_empty(),
            "{source} -> {target}: {:?}",
            check_segment_pair(source, target)
        );
    }
}

#[test]
fn keyword_syntax_and_trailing_spaces_are_not_rewritten() {
    for text in [
        "对目标施加[DarkFlame]",
        "[{0}]行动",
        "呼吸法 ",
        "[选项]继续",
        "[A][B]",
    ] {
        assert_eq!(auto_fix_target_zh(text), text);
        assert!(!check_segment_pair(text, text)
            .iter()
            .any(|issue| issue.rule_id == "L04"));
    }
}
