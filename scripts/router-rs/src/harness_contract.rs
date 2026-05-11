use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const HARNESS_CONTRACT_SCHEMA_VERSION: &str = "router-rs-harness-contract-v1";
pub const HARNESS_SKILL_LINT_SCHEMA_VERSION: &str = "router-rs-harness-skill-contract-lint-v1";
pub const HARNESS_CONTRACT_AUTHORITY: &str = "rust-harness-contract";

pub const FAILURE_TAXONOMY: &[(&str, &str)] = &[
    ("route_miss", "The task routes to the wrong owner/gate or misses an expected owner."),
    ("owner_drift", "The active owner, goal, or scope changes without explicit contract update intent."),
    ("context_rot", "Large or irrelevant context accumulates in the parent thread or hot prompt surface."),
    ("tool_contract_bad", "A tool/skill interface is ambiguous, too verbose, or lacks failure semantics."),
    ("verification_missing", "A completion/pass claim has no executable verifier or evidence reference."),
    ("source_stale", "The answer depends on volatile external truth without an attributed current source."),
    ("side_effect_risk", "A step can mutate external state without explicit approval, idempotency, or recovery notes."),
    ("subagent_misuse", "A subagent lane is used without isolation, bounded scope, digest, or verification."),
    ("trace_gap", "The task cannot be reconstructed from trace/evidence artifacts."),
    ("step_recovery_gap", "A long-running task cannot resume from a step-level durable ledger."),
];

pub fn harness_contract() -> Value {
    json!({
        "schema_version": HARNESS_CONTRACT_SCHEMA_VERSION,
        "authority": HARNESS_CONTRACT_AUTHORITY,
        "failure_taxonomy": failure_taxonomy_values(),
        "trajectory_event_convention": {
            "sink": "TRACE_EVENTS.jsonl via trace_runtime record-event",
            "required_payload_fields": [
                "task_id",
                "owner",
                "gate",
                "overlay",
                "horizon",
                "phase",
                "tool_or_lane",
                "status",
                "failure_class",
                "evidence_ref",
                "context_bytes"
            ],
            "model_context_policy": "Persist full trajectory events; inject only summaries, cursors, or evidence refs."
        },
        "behavioral_eval_tracks": [
            "routing_accuracy",
            "token_efficiency",
            "long_task_continuity",
            "trajectory_health",
            "closeout_integrity",
            "skill_contract_quality",
            "subagent_lane_integrity"
        ],
        "step_recovery": {
            "ledger": "STEP_LEDGER.jsonl",
            "summary_projection": "TASK_STATE.json.step_ledger",
            "canonical_writer": "router-rs framework step-ledger"
        }
    })
}

pub fn lint_skill_contracts(payload: Value) -> Result<Value, String> {
    let skills_root = payload
        .get("skills_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("skills"));
    let slugs = payload
        .get("slugs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(default_high_impact_skill_slugs);
    let mut findings = Vec::new();
    let mut scanned = Vec::new();
    for slug in slugs {
        let path = skills_root.join(&slug).join("SKILL.md");
        if !path.is_file() {
            findings.push(finding(
                &slug,
                "source_stale",
                "major",
                "skill file missing",
                format!("Expected SKILL.md at {}", path.display()),
                Some(path.display().to_string()),
                "Fix the skill path or remove it from the lint target set.",
            ));
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("read skill {} failed: {err}", path.display()))?;
        scanned.push(slug.clone());
        lint_one_skill(&slug, &path, &text, &mut findings);
    }

    let execution_items = findings
        .iter()
        .enumerate()
        .map(|(idx, finding)| {
            let finding_id = finding["finding_id"].as_str().unwrap_or("unknown");
            json!({
                "item_id": format!("exec-{}", idx + 1),
                "finding_ids": [finding_id],
                "action": "Tighten skill/tool contract wording or verification surface for the referenced finding.",
                "scope": finding["location"],
                "priority": if finding["severity"] == "major" { "P1" } else { "P2" }
            })
        })
        .collect::<Vec<_>>();
    let status = if findings.iter().any(|f| f["severity"] == "major") {
        "partial"
    } else {
        "pass"
    };
    Ok(json!({
        "schema_version": HARNESS_SKILL_LINT_SCHEMA_VERSION,
        "authority": HARNESS_CONTRACT_AUTHORITY,
        "skills_root": skills_root.display().to_string(),
        "skills_scanned": scanned,
        "failure_taxonomy": failure_taxonomy_values(),
        "findings": findings,
        "execution_items": execution_items,
        "verification_results": [{
            "item_id": "skill-contract-lint",
            "status": status,
            "evidence": "Static SKILL.md contract lint completed using harness failure taxonomy.",
            "regression": null
        }]
    }))
}

fn lint_one_skill(slug: &str, path: &Path, text: &str, findings: &mut Vec<Value>) {
    let frontmatter = frontmatter_block(text);
    let description_len = frontmatter
        .and_then(|fm| yaml_multiline_value(fm, "description"))
        .map(|s| s.chars().count())
        .unwrap_or(0);
    if description_len > 650 {
        findings.push(finding(
            slug,
            "context_rot",
            "minor",
            "frontmatter description is long",
            format!("description has {description_len} chars; hot routing summaries should stay compact"),
            Some(path.display().to_string()),
            "Move optional detail into references/ and keep routing description compact.",
        ));
    }
    let trigger_count = count_yaml_list_items(frontmatter.unwrap_or_default(), "trigger_hints");
    if trigger_count < 3 {
        findings.push(finding(
            slug,
            "route_miss",
            "major",
            "trigger surface is thin",
            format!("trigger_hints count is {trigger_count}; high-impact skills need concrete user phrasing"),
            Some(path.display().to_string()),
            "Add concrete trigger phrases users actually say.",
        ));
    }
    if !contains_heading(text, "Do not use") {
        findings.push(finding(
            slug,
            "owner_drift",
            "minor",
            "missing negative routing boundary",
            "SKILL.md has no `Do not use` section, so nearby owners may overlap.".to_string(),
            Some(path.display().to_string()),
            "Add a short Do not use section with handoff owners.",
        ));
    }
    let lower = text.to_ascii_lowercase();
    let has_verification =
        lower.contains("validation") || lower.contains("verify") || text.contains("验证");
    if !has_verification {
        findings.push(finding(
            slug,
            "verification_missing",
            "major",
            "missing verification surface",
            "SKILL.md does not mention validation/verification, so closeout evidence may drift into prose.".to_string(),
            Some(path.display().to_string()),
            "Name the expected command, artifact, evidence row, or explicit blocker pattern.",
        ));
    }
    if lower.contains("tool") && !(lower.contains("error") || lower.contains("fail")) {
        findings.push(finding(
            slug,
            "tool_contract_bad",
            "minor",
            "tool-facing guidance lacks failure semantics",
            "The skill mentions tools but not failure/error behavior.".to_string(),
            Some(path.display().to_string()),
            "Specify how tool failures should be surfaced or recorded.",
        ));
    }
}

fn finding(
    slug: &str,
    category: &str,
    severity: &str,
    title: &str,
    description: String,
    location: Option<String>,
    suggestion: &str,
) -> Value {
    json!({
        "finding_id": format!("{slug}-{category}"),
        "severity": severity,
        "category": category,
        "title": title,
        "description": description,
        "location": location,
        "suggestion": suggestion,
        "effort": if severity == "major" { "small" } else { "trivial" }
    })
}

fn failure_taxonomy_values() -> Vec<Value> {
    FAILURE_TAXONOMY
        .iter()
        .map(|(id, description)| json!({"id": id, "description": description}))
        .collect()
}

fn default_high_impact_skill_slugs() -> Vec<String> {
    [
        "skill-framework-developer",
        "plan-mode",
        "agent-swarm-orchestration",
        "research-workbench",
        "openai-docs",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn frontmatter_block(text: &str) -> Option<&str> {
    let trimmed = text.strip_prefix("---\n")?;
    let end = trimmed.find("\n---")?;
    Some(&trimmed[..end])
}

fn yaml_multiline_value(fm: &str, key: &str) -> Option<String> {
    let needle = format!("{key}:");
    let mut capture = false;
    let mut out = Vec::new();
    for line in fm.lines() {
        if line.starts_with(&needle) {
            capture = true;
            let rest = line[needle.len()..].trim();
            if rest != "|" && !rest.is_empty() {
                return Some(rest.trim_matches('"').to_string());
            }
            continue;
        }
        if capture {
            if line.starts_with(char::is_alphanumeric) && line.contains(':') {
                break;
            }
            out.push(line.trim().to_string());
        }
    }
    let value = out.join(" ").trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn count_yaml_list_items(fm: &str, key: &str) -> usize {
    let needle = format!("{key}:");
    let mut capture = false;
    let mut count = 0_usize;
    for line in fm.lines() {
        if line.starts_with(&needle) {
            capture = true;
            continue;
        }
        if capture {
            if line.starts_with(char::is_alphanumeric) && line.contains(':') {
                break;
            }
            if line.trim_start().starts_with("- ") {
                count += 1;
            }
        }
    }
    count
}

fn contains_heading(text: &str, heading: &str) -> bool {
    let heading_lower = heading.to_ascii_lowercase();
    text.lines().any(|line| {
        let trimmed = line.trim_start_matches('#').trim().to_ascii_lowercase();
        trimmed == heading_lower
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_contract_lists_failure_taxonomy() {
        let contract = harness_contract();
        let ids = contract["failure_taxonomy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"verification_missing"));
        assert!(ids.contains(&"step_recovery_gap"));
    }

    #[test]
    fn skill_lint_reports_existing_high_impact_shape() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills");
        let report = lint_skill_contracts(json!({
            "skills_root": root.display().to_string(),
            "slugs": ["skill-framework-developer"]
        }))
        .expect("lint");
        assert_eq!(report["schema_version"], HARNESS_SKILL_LINT_SCHEMA_VERSION);
        assert_eq!(report["skills_scanned"][0], "skill-framework-developer");
        assert!(report["findings"].as_array().is_some());
    }
}
