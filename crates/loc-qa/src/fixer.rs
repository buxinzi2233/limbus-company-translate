use crate::rules::{fix_l02_tilde, fix_l04_bracket_space};

/// 对中文译文执行确定性安全自动修复 (波浪号转半角 + 关键词空格补齐)
pub fn auto_fix_target_zh(text: &str) -> String {
    let step1 = fix_l02_tilde(text);
    fix_l04_bracket_space(&step1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_fix() {
        let dirty = "自身获得2层[呼吸法]状态，造成10～20点伤害";
        let clean = auto_fix_target_zh(dirty);
        assert_eq!(clean, "自身获得2层[呼吸法] 状态，造成10~20点伤害");
    }
}
