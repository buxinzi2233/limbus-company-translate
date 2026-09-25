/// 检测字符串中是否包含韩文字符 (Hangul Syllables, Jamo, Compatibility Jamo)
pub fn contains_hangul(text: &str) -> bool {
    text.chars().any(is_hangul_char)
}

/// 判定单个字符是否属于韩文字符集
pub fn is_hangul_char(c: char) -> bool {
    matches!(
        c,
        '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
        | '\u{1100}'..='\u{11FF}' // Hangul Jamo
        | '\u{3130}'..='\u{318F}' // Hangul Compatibility Jamo
        | '\u{A960}'..='\u{A97F}' // Hangul Jamo Extended-A
        | '\u{D7B0}'..='\u{D7FF}' // Hangul Jamo Extended-B
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_hangul() {
        assert!(contains_hangul("균열이 마침내 완전히 찢어지고..."));
        assert!(contains_hangul("단테"));
        assert!(contains_hangul("<몸 상태가 처음 계약했을 때처럼...>"));
        assert!(!contains_hangul("Hello World 123"));
        assert!(!contains_hangul("<color=#ff0000>Critical!</color>"));
        assert!(!contains_hangul("裂隙终于彻底撕裂……"));
    }
}
