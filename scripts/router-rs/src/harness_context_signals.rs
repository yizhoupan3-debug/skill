//! Heuristics for harness operator nudges (math / formal verification context).
//! Kept separate from `framework_runtime` to avoid module dependency cycles.
//! Formal-tool ASCII substrings are delegated to [`crate::formal_toolchain`].
//!
//! PoC: when `proof of concept` / `proof-of-concept` appears, loose English tokens
//! (theorem/lemma/…) do not fire; toolchain hits still fire earlier. Narrow overrides:
//! `formal proof` / `mathematical proof` still fire inside PoC (e.g. rigorous PoC writeups).

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static PROOF_ASCII_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\bproof\b").expect("proof ascii word regex"));

/// True when natural language or shell-like text suggests math / formal checker work.
pub fn text_signals_math_or_formal_checker(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    const ZH: &[&str] = &[
        "数学",
        "证明",
        "推导",
        "定理",
        "引理",
        "公理",
        "不等式",
        "收敛",
        "极限",
    ];
    if ZH.iter().any(|k| s.contains(k)) {
        return true;
    }
    if formal_tool_tokens(&lower) {
        return true;
    }
    if proof_of_concept_phrase(&lower) {
        return english_math_narrow_when_poc(&lower);
    }
    if english_math_soft_no_poc(&lower) {
        return true;
    }
    lower.contains("lean 4")
}

fn proof_of_concept_phrase(lower: &str) -> bool {
    lower.contains("proof of concept") || lower.contains("proof-of-concept")
}

/// Inside PoC marketing copy: only explicit formal-proof phrases count (not theorem/lemma alone).
fn english_math_narrow_when_poc(lower: &str) -> bool {
    lower.contains("formal proof") || lower.contains("mathematical proof")
}

/// Loose English when no PoC phrase (no `sympy`/`lean4` here — covered by [`formal_toolchain`]).
fn english_math_soft_no_poc(lower: &str) -> bool {
    const SUBSTRING: &[&str] = &[
        "theorem",
        "lemma",
        "corollary",
        "formal proof",
        "mathematical proof",
        "smt solver",
    ];
    if SUBSTRING.iter().any(|k| lower.contains(k)) {
        return true;
    }
    PROOF_ASCII_WORD.is_match(lower)
}

fn formal_tool_tokens(lower: &str) -> bool {
    crate::formal_toolchain::ascii_lower_contains_formal_toolchain_tokens(lower)
}

/// OR of [`text_signals_math_or_formal_checker`] over RFV `goal` and `verify_commands`.
pub fn rfv_state_signals_math(state: &Value) -> bool {
    if state
        .get("goal")
        .and_then(Value::as_str)
        .is_some_and(text_signals_math_or_formal_checker)
    {
        return true;
    }
    let Some(cmds) = state.get("verify_commands").and_then(Value::as_array) else {
        return false;
    };
    cmds.iter()
        .filter_map(Value::as_str)
        .any(text_signals_math_or_formal_checker)
}

/// OR over Autopilot `GOAL_STATE` `goal` and `validation_commands` (mirrors RFV `verify_commands`).
pub fn autopilot_state_signals_math(state: &Value) -> bool {
    if state
        .get("goal")
        .and_then(Value::as_str)
        .is_some_and(text_signals_math_or_formal_checker)
    {
        return true;
    }
    let Some(cmds) = state.get("validation_commands").and_then(Value::as_array) else {
        return false;
    };
    cmds.iter()
        .filter_map(Value::as_str)
        .any(text_signals_math_or_formal_checker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_keywords_trigger() {
        assert!(text_signals_math_or_formal_checker("证明不等式"));
        assert!(text_signals_math_or_formal_checker(" 定理 "));
    }

    #[test]
    fn en_keywords_trigger() {
        assert!(text_signals_math_or_formal_checker("Prove this lemma"));
        assert!(text_signals_math_or_formal_checker("SYMPY simplify"));
    }

    #[test]
    fn proof_word_boundary_and_poc_denylist() {
        assert!(!text_signals_math_or_formal_checker(
            "This is a proof of concept for the API"
        ));
        assert!(!text_signals_math_or_formal_checker(
            "Proof-of-concept milestone"
        ));
        assert!(text_signals_math_or_formal_checker(
            "We need a mathematical proof of convergence"
        ));
        assert!(text_signals_math_or_formal_checker("formal proof sketch"));
        assert!(!text_signals_math_or_formal_checker("waterproof jacket"));
    }

    #[test]
    fn poc_blocks_theorem_lemma_but_allows_narrow_phrases() {
        assert!(!text_signals_math_or_formal_checker(
            "theorem proof of concept for investors"
        ));
        assert!(!text_signals_math_or_formal_checker(
            "lemma proof-of-concept deck"
        ));
        assert!(text_signals_math_or_formal_checker(
            "formal proof of concept writeup"
        ));
        assert!(text_signals_math_or_formal_checker(
            "mathematical proof of concept appendix"
        ));
    }

    #[test]
    fn sympy_with_poc_still_true_via_formal_toolchain() {
        assert!(text_signals_math_or_formal_checker(
            "sympy proof of concept spike"
        ));
    }

    #[test]
    fn serde_derive_does_not_trigger() {
        assert!(!text_signals_math_or_formal_checker(
            "#[derive(Serialize, Deserialize)]"
        ));
        assert!(!text_signals_math_or_formal_checker(
            "add serde derive to the DTO"
        ));
    }

    #[test]
    fn formal_tokens_trigger() {
        assert!(text_signals_math_or_formal_checker("z3 /tmp/x.smt2"));
        assert!(text_signals_math_or_formal_checker("lake build -q"));
        assert!(text_signals_math_or_formal_checker(
            "python -c \"import sympy\""
        ));
    }

    #[test]
    fn benign_text_false() {
        assert!(!text_signals_math_or_formal_checker("cargo test -q"));
        assert!(!text_signals_math_or_formal_checker("leaning tower"));
    }

    #[test]
    fn rfv_state_or_goal_and_commands() {
        let st = serde_json::json!({
            "goal": "refactor hooks",
            "verify_commands": ["python -c \"import sympy; print(1)\""]
        });
        assert!(rfv_state_signals_math(&st));
        let st2 = serde_json::json!({"goal": "数学归纳", "verify_commands": []});
        assert!(rfv_state_signals_math(&st2));
        let st3 = serde_json::json!({"goal": "lint only", "verify_commands": ["ruff check"]});
        assert!(!rfv_state_signals_math(&st3));
    }

    #[test]
    fn autopilot_state_scans_validation_commands() {
        let st = serde_json::json!({
            "goal": "ship feature X",
            "validation_commands": ["python -c \"import sympy; print(2)\""]
        });
        assert!(autopilot_state_signals_math(&st));
        let st2 = serde_json::json!({
            "goal": "proof of concept demo",
            "validation_commands": ["cargo test -q"]
        });
        assert!(!autopilot_state_signals_math(&st2));
    }
}
