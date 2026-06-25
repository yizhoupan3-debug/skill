//! Shared text utilities: CJK detection, tokenization, ASCII helpers.
//!
//! These functions are used by routing-engine, mcp-tool-registry, and research-harness.
//! Single source of truth for CJK Unicode range coverage (Extension A-G + Compatibility + Hiragana + Katakana + Hangul).

/// Check if a character is CJK (Han, Hiragana, Katakana, Hangul).
/// Covers Unicode 15.0 ranges: CJK Unified Ideographs (basic + Extension A-G),
/// CJK Compatibility Ideographs, Hiragana, Katakana, Hangul Syllables.
pub fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4e00}'..='\u{9fff}'     // CJK Unified Ideographs (basic, ~20k)
        | '\u{3400}'..='\u{4dbf}'   // CJK Extension A (~6.5k)
        | '\u{20000}'..='\u{2a6df}' // CJK Extension B (~42k)
        | '\u{2a700}'..='\u{2b73f}' // CJK Extension C
        | '\u{2b740}'..='\u{2b81f}' // CJK Extension D
        | '\u{2b820}'..='\u{2ceaf}' // CJK Extension E
        | '\u{2ceb0}'..='\u{2ebef}' // CJK Extension F
        | '\u{30000}'..='\u{3134f}' // CJK Extension G
        | '\u{f900}'..='\u{faff}'   // CJK Compatibility Ideographs
        | '\u{2f80}'..='\u{2faf}'   // CJK Compatibility Supplement
        | '\u{3040}'..='\u{309f}'   // Hiragana
        | '\u{30a0}'..='\u{30ff}'   // Katakana
        | '\u{ac00}'..='\u{d7af}'   // Hangul Syllables
    )
}

/// Check if a string is purely ASCII word characters (alphanumeric, no CJK, no spaces).
pub fn is_ascii_word(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Tokenize text into meaningful tokens: words separated by whitespace/commas/punctuation,
/// plus individual CJK characters (each CJK char is a meaningful unit in Chinese/Japanese/Korean).
///
/// Examples:
/// - `"hello world"` → `["hello", "world"]`
/// - `"帮我截图"` → `["帮", "我", "截", "图"]`
/// - `"browser-screenshot"` → `["browser", "screenshot"]`
/// - `"PDF 文档提取"` → `["PDF", "文", "档", "提", "取"]`
pub fn tokenize_cjk_aware(text: &str) -> Vec<String> {
    text.split(|c: char| {
        c.is_ascii_whitespace()
            || c == '-'
            || c == '_'
            || c == ','
            || c == '，'
            || c == '。'
            || c == '.'
            || c == '?'
            || c == '！'
            || c == '!'
            || c == ':'
            || c == '：'
            || c == ';'
            || c == '；'
            || c == '('
            || c == ')'
            || c == '（'
            || c == '）'
            || c == '['
            || c == ']'
            || c == '【'
            || c == '】'
            || c == '、'
            || c == '\u{3000}' // 全角空格
            || c == '《'
            || c == '》'
            || c == '「'
            || c == '」'
            || c == '『'
            || c == '』'
            || c == '\u{2014}'
            || c == '\u{2015}'
            || c == '·'
    })
    .filter(|t| !t.is_empty())
    .flat_map(|token| {
        let mut result = Vec::new();
        let mut current = String::new();
        for ch in token.chars() {
            if is_cjk(ch) {
                if !current.is_empty() {
                    result.push(current.clone());
                    current.clear();
                }
                result.push(ch.to_string());
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            result.push(current);
        }
        result
    })
    .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn is_cjk_basic() {
        assert!(is_cjk('中'));
        assert!(is_cjk('あ'));
        assert!(is_cjk('ア'));
        assert!(is_cjk('한'));
        assert!(!is_cjk('a'));
        assert!(!is_cjk('1'));
    }

    #[test]
    fn tokenize_ascii() {
        assert_eq!(tokenize_cjk_aware("hello world"), vec!["hello", "world"]);
    }

    #[test]
    fn tokenize_cjk() {
        assert_eq!(
            tokenize_cjk_aware("帮我截图"),
            vec!["帮", "我", "截", "图"]
        );
    }

    #[test]
    fn tokenize_mixed() {
        assert_eq!(
            tokenize_cjk_aware("PDF 文档"),
            vec!["PDF", "文", "档"]
        );
    }

    #[test]
    fn is_ascii_word_basic() {
        assert!(is_ascii_word("hello"));
        assert!(is_ascii_word("123"));
        assert!(!is_ascii_word("hello world"));
        assert!(!is_ascii_word(""));
        assert!(!is_ascii_word("中文"));
    }
}
