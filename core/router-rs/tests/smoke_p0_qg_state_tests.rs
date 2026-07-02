//! `quality_gate_state_signals_math` coverage at router-rs boundary.

use crate::harness_context_signals::quality_gate_state_signals_math;
use serde_json::json;

/// Harness math/formal signal gate over QG `goal` + `verify_commands` (router-rs P0).
#[test]
fn quality_gate_state_signals_math_smoke() {
    let formal = json!({
        "goal": "lint only",
        "verify_commands": ["python -c \"import sympy\""]
    });
    assert!(quality_gate_state_signals_math(&formal));
    let benign = json!({
        "goal": "cargo fmt",
        "verify_commands": ["cargo test -q"]
    });
    assert!(!quality_gate_state_signals_math(&benign));
}
