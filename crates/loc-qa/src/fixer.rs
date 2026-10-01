use crate::rules::fix_l02_tilde;

/// Normalize fullwidth tildes without changing keyword or bracket spacing.
pub fn auto_fix_target_zh(text: &str) -> String {
    fix_l02_tilde(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_fix() {
        let dirty = "自身获得2层[呼吸法]状态，造成10～20点伤害";
        let clean = auto_fix_target_zh(dirty);
        assert_eq!(clean, "自身获得2层[呼吸法]状态，造成10~20点伤害");
    }
}
