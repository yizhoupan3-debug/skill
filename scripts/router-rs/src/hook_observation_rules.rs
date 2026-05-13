//! Compile-time embedded `ROUTER_RS_HOOK_OBSERVATION_RULES.json` from the repo
//! `configs/framework/` tree (`include_str!` path is relative to `scripts/router-rs`).

use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const RULES_EMBED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json"
));

const EXPECTED_SCHEMA: &str = "router-rs-hook-observation-rules-v1";

#[derive(Debug, Clone)]
pub(crate) struct GateClassified {
    pub code: String,
    pub human_prefix: String,
}

struct ParsedRules {
    tokens: HashMap<String, String>,
    unknown_code: String,
    followup_rules: Vec<Value>,
    additional_rules: Vec<Value>,
}

fn parsed_rules() -> &'static ParsedRules {
    static CELL: OnceLock<ParsedRules> = OnceLock::new();
    CELL.get_or_init(|| {
        let root: Value =
            serde_json::from_str(RULES_EMBED).expect("ROUTER_RS_HOOK_OBSERVATION_RULES.json");
        let sv = root
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert_eq!(
            sv, EXPECTED_SCHEMA,
            "ROUTER_RS_HOOK_OBSERVATION_RULES schema_version mismatch"
        );
        let mut tokens = HashMap::new();
        if let Some(obj) = root.get("router_rs_tokens").and_then(Value::as_object) {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    tokens.insert(k.clone(), s.to_string());
                }
            }
        }
        let unknown_code = root
            .get("unknown_router_rs_token_code")
            .and_then(Value::as_str)
            .unwrap_or("unknown_router_rs")
            .to_string();
        let followup_rules = root
            .get("followup_first_line_rules")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let additional_rules = root
            .get("additional_context_line_rules")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        ParsedRules {
            tokens,
            unknown_code,
            followup_rules,
            additional_rules,
        }
    })
}

pub(crate) fn shorten_line(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() <= max {
        return t.to_string();
    }
    let mut cut = max.min(t.len());
    while cut > 0 && !t.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}...", &t[..cut])
}

fn human_prefix_router_rs_leader(line: &str) -> String {
    let rest = line.strip_prefix("router-rs ").unwrap_or("").trim();
    let token = rest.split_whitespace().next().unwrap_or("");
    if token.is_empty() {
        "router-rs".to_string()
    } else {
        format!("router-rs {token}")
    }
}

fn human_prefix_from_spec(line: &str, spec: &Value) -> String {
    let kind = spec
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("trimmed_line");
    match kind {
        "trimmed_line" => {
            let max = spec.get("max_chars").and_then(Value::as_u64).unwrap_or(120) as usize;
            shorten_line(line, max)
        }
        "literal" => spec
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "router_rs_leader" => human_prefix_router_rs_leader(line),
        _ => shorten_line(line, 120),
    }
}

fn classify_router_rs_after_leader_line(line: &str, rules: &ParsedRules) -> Option<GateClassified> {
    let rest = line.strip_prefix("router-rs ").unwrap_or("").trim();
    let token = rest.split_whitespace().next().unwrap_or("");
    let code = if token.is_empty() {
        return None;
    } else if let Some(c) = rules.tokens.get(token) {
        c.clone()
    } else {
        rules.unknown_code.clone()
    };
    Some(GateClassified {
        code,
        human_prefix: human_prefix_router_rs_leader(line),
    })
}

fn followup_rule_matches(line: &str, rule: &Value) -> bool {
    if let Some(s) = rule.get("starts_with").and_then(Value::as_str) {
        if line.starts_with(s) {
            return true;
        }
    }
    if let Some(arr) = rule.get("starts_with_any").and_then(Value::as_array) {
        return arr
            .iter()
            .filter_map(Value::as_str)
            .any(|p| line.starts_with(p));
    }
    false
}

fn additional_subcond_matches(line: &str, cond: &Value) -> bool {
    if let Some(s) = cond.get("starts_with").and_then(Value::as_str) {
        return line.starts_with(s);
    }
    if let Some(s) = cond.get("contains").and_then(Value::as_str) {
        return line.contains(s);
    }
    false
}

fn additional_rule_matches(line: &str, rule: &Value) -> bool {
    if let Some(arr) = rule.get("or").and_then(Value::as_array) {
        return arr.iter().any(|c| additional_subcond_matches(line, c));
    }
    if let Some(s) = rule.get("starts_with").and_then(Value::as_str) {
        return line.starts_with(s);
    }
    if let Some(arr) = rule.get("starts_with_any").and_then(Value::as_array) {
        return arr
            .iter()
            .filter_map(Value::as_str)
            .any(|p| line.starts_with(p));
    }
    if let Some(s) = rule.get("contains").and_then(Value::as_str) {
        return line.contains(s);
    }
    false
}

pub(crate) fn classify_followup_first_line(line: &str) -> Option<GateClassified> {
    let t = line.trim();
    let rules = parsed_rules();
    for rule in &rules.followup_rules {
        if !followup_rule_matches(t, rule) {
            continue;
        }
        if rule
            .get("code_from_router_rs_leader")
            .and_then(Value::as_bool)
            == Some(true)
        {
            return classify_router_rs_after_leader_line(t, rules);
        }
        let code = rule.get("code")?.as_str()?.to_string();
        let hp = human_prefix_from_spec(t, rule.get("human_prefix")?);
        return Some(GateClassified {
            code,
            human_prefix: hp,
        });
    }
    None
}

pub(crate) fn classify_additional_context(text: &str) -> Option<GateClassified> {
    let rules = parsed_rules();
    let mut picked: Option<(u8, GateClassified)> = None;
    for line in text.lines() {
        let t = line.trim();
        let mut line_match: Option<(u8, GateClassified, bool)> = None;
        for rule in &rules.additional_rules {
            if !additional_rule_matches(t, rule) {
                continue;
            }
            let pri = rule.get("priority").and_then(Value::as_u64).unwrap_or(99) as u8;
            let gate_opt = if rule
                .get("code_from_router_rs_leader")
                .and_then(Value::as_bool)
                == Some(true)
            {
                classify_router_rs_after_leader_line(t, rules)
            } else {
                let code = rule
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let hp =
                    human_prefix_from_spec(t, rule.get("human_prefix").unwrap_or(&Value::Null));
                Some(GateClassified {
                    code,
                    human_prefix: hp,
                })
            };
            let Some(gate) = gate_opt else {
                continue;
            };
            let stop = rule.get("stop_scan_after_match").and_then(Value::as_bool) == Some(true);
            line_match = Some((pri, gate, stop));
            break;
        }
        if let Some((pri, gate, stop)) = line_match {
            let replace = picked
                .as_ref()
                .map(|(existing_pri, _)| pri < *existing_pri)
                .unwrap_or(true);
            if replace {
                picked = Some((pri, gate));
            }
            if stop {
                break;
            }
        }
    }
    picked.map(|(_, g)| g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rules_parse_and_schema() {
        let _ = parsed_rules();
    }

    #[test]
    fn golden_followup_review_gate() {
        let line = "router-rs REVIEW_GATE incomplete";
        let g = classify_followup_first_line(line).expect("gate");
        assert_eq!(g.code, "review_gate");
        assert!(g.human_prefix.contains("REVIEW_GATE"));
    }

    #[test]
    fn golden_additional_rfv_priority_over_session() {
        let text = "SESSION_CLOSE_STYLE: x\nRFV_LOOP_CONTINUE y";
        let g = classify_additional_context(text).expect("gate");
        assert_eq!(g.code, "rfv_loop_continue");
    }
}
