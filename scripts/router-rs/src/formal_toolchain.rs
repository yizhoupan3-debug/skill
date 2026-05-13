//! Formal-proof / math-tool **ASCII-lowercase** substring heuristics for shell-like text.
//! Single implementation shared by [`crate::harness_context_signals`] and
//! [`crate::framework_runtime`] (`shell_command_looks_like_verification`).
//!
//! Callers must pass `command.to_ascii_lowercase()` (or equivalent) as `c`.

/// True when `c` (already ASCII-lowercased) contains narrow formal-tool tokens
/// (SymPy, Z3, Lean, Coq, Lake, Isabelle) aligned with verification-command detection.
pub(crate) fn ascii_lower_contains_formal_toolchain_tokens(c: &str) -> bool {
    c.contains("sympy")
        || c.contains(" z3 ")
        || c.starts_with("z3 ")
        || c.contains("\tz3 ")
        || c.contains(" z3\t")
        || c.contains("lean4")
        || c.contains(" lean ")
        || c.trim_start().starts_with("lean ")
        || c.contains("coqc")
        || c.contains("coqchk")
        || c.contains("lake build")
        || c.contains("lake test")
        || c.contains("lake check")
        || c.contains("lake exe")
        || c.contains("isabelle build")
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
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc(
            "z3 /tmp/proof.smt2"
        )));
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc(
            "  z3  /tmp/x.smt2"
        )));
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc(
            "lean --version"
        )));
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc(
            "lake build && lake test"
        )));
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc(
            "coqc -Q theories Foo.v"
        )));
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc(
            "coqchk -silent Foo.vo"
        )));
        assert!(ascii_lower_contains_formal_toolchain_tokens(&lc(
            "isabelle build -D ."
        )));
    }

    #[test]
    fn matrix_rejects_benign_substrings() {
        assert!(!ascii_lower_contains_formal_toolchain_tokens(&lc(
            "leaning tower"
        )));
        assert!(!ascii_lower_contains_formal_toolchain_tokens(&lc(
            "echo hello"
        )));
    }
}
