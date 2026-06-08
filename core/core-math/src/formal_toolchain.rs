//! Formal-proof / math-tool **ASCII-lowercase** substring heuristics for shell-like text.
//!
//! Callers must pass `command.to_ascii_lowercase()` (or equivalent) as `c`.

fn ascii_lower_contains_word_token(c: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let bytes = c.as_bytes();
    let tlen = token.len();
    let mut i = 0;
    while i + tlen <= bytes.len() {
        if &c[i..i + tlen] == token {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_ok = i + tlen == bytes.len() || !bytes[i + tlen].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// True when `c` (already ASCII-lowercased) contains narrow formal-tool tokens.
pub fn ascii_lower_contains_formal_toolchain_tokens(c: &str) -> bool {
    c.contains("sympy")
        || c.contains("z3")
        || ascii_lower_contains_word_token(c, "lean")
        || c.contains("coqc")
        || c.contains("coqchk")
        || c.contains("lake build")
        || c.contains("lake test")
        || c.contains("lake check")
        || c.contains("lake exe")
        || c.contains("isabelle build")
        || c.contains("agda")
        || c.contains("idris")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lc(s: &str) -> String {
        s.to_ascii_lowercase()
    }

    #[test]
    fn matrix_detects_formal_tools() {
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc(
            "python -c \"import sympy; print(1)\""
        )));
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc("z3 /tmp/proof.smt2")));
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc("lean --version")));
    }

    #[test]
    fn matrix_rejects_benign_substrings() {
        assert!(!ascii_lower_contains_formal_toolchain_tokens(&lc("echo hello")));
        assert!(!ascii_lower_contains_formal_toolchain_tokens(&lc("leaning tower")));
    }
}
