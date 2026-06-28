// Test helper functions that may not all be called from every test.

use crate::common::{project_root, read_json, read_text};
use serde_json::Value;
use std::collections::HashSet;

const FRAMEWORK_COMMAND_IDS: &[&str] = &["deepinterview", "gitx", "update"];

fn manifest_or_runtime_lane_contains(runtime_slugs: &HashSet<&str>, slug: &str) -> bool {
    slug == "none"
        || runtime_slugs.contains(slug)
        || FRAMEWORK_COMMAND_IDS.contains(&slug)
        || project_root()
            .join("skills")
            .join(slug)
            .join("SKILL.md")
            .is_file()
}

fn key_index(keys: &[Value], name: &str) -> usize {
    keys.iter()
        .position(|key| key.as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing key {name}"))
}

fn key_index_first(keys: &[Value], names: &[&str]) -> usize {
    names
        .iter()
        .find_map(|name| keys.iter().position(|key| key.as_str() == Some(*name)))
        .unwrap_or_else(|| panic!("missing keys {:?}", names))
}

/// Hot runtime rows store per-skill hosts under `host_platforms`.
fn runtime_host_platforms_index(keys: &[Value]) -> usize {
    key_index_first(keys, &["host_platforms"])
}

fn runtime_description_index(keys: &[Value]) -> usize {
    key_index_first(keys, &["description", "summary"])
}

#[test]
fn gitx_skill_exposes_codex_shortcut_and_closeout_flow() {
    let content = read_text(&project_root().join("skills/gitx/SKILL.md"));
    for marker in [
        "name: gitx",
        "推荐显式入口：`/gitx`",
        "/gitx plan",
        "review、修复、整理、提交、合并分支、合并 worktree、推送",
        "git status --short --branch",
        "git worktree list --porcelain",
        "git diff --stat",
        "不要依赖已移除的 Python git helper",
        "RTK",
    ] {
        assert!(content.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn plan_mode_keeps_review_optional_and_review_only() {
    let plan_mode = read_text(&project_root().join("skills/plan-mode/SKILL.md"));
    for forbidden in [
        "调研 + review 先于计划",
        "初稿后：独立上下文 subagent 审 plan",
    ] {
        assert!(
            !plan_mode.contains(forbidden),
            "plan-mode must not make review a default plan step: {forbidden}"
        );
    }
    let marker = "仅当用户明确要求 review plan / 审计划";
    assert!(plan_mode.contains(marker), "missing marker: {marker}");

    let review_gate = read_text(&project_root().join(".rules/review-subagent-gate.mdc"));
    for marker in [
        "review lane **只读**",
        "纯 review 禁止默认改代码",
        "skills/code-review-deep/SKILL.md",
    ] {
        assert!(review_gate.contains(marker), "missing marker: {marker}");
    }

    let agents = read_text(&project_root().join("AGENTS.md"));
    for marker in [
        "Review findings-only",
        "skills/code-review-deep/SKILL.md",
        "面向用户的回复必须使用简体中文",
        "Closeout",
        "Skill Routing",
        "Goal state",
    ] {
        assert!(agents.contains(marker), "missing AGENTS marker: {marker}");
    }

    let code_review = read_text(&project_root().join("skills/code-review-deep/SKILL.md"));
    assert!(
        code_review.contains("Findings-only by default"),
        "code-review-deep must forbid default execution on review"
    );
}

#[test]
fn update_skill_exposes_explicit_entrypoint_like_gitx() {
    let content = read_text(&project_root().join("skills/update/SKILL.md"));
    for marker in [
        "name: update",
        "推荐显式写法：`/update`",
        "document refresh",
        "git tracking audit",
        "stale/dead inventory",
        "cleanup + verification",
        "科研文档是一等维护对象",
        "git 跟踪面",
        "死代码",
        "旧文档",
        "cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-audit",
        "policy_contracts",
        "cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-one-shot",
        "documentation_contracts",
        "tracked_markdown_utf8_contract",
        "generated-artifacts-status",
        "不直接删除：无法证明废弃的科研资料",
    ] {
        assert!(content.contains(marker), "missing marker: {marker}");
    }
    let registry = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let update = &registry["framework_commands"]["update"];
    assert_eq!(
        update["skill_path"].as_str().expect("update skill_path"),
        "skills/update/SKILL.md"
    );
    let entrypoints = update["interaction_invariants"]["explicit_entrypoints"]
        .as_array()
        .expect("explicit entrypoints");
    assert!(
        entrypoints
            .iter()
            .filter_map(|v| v.as_str())
            .any(|e| e == "/update"),
        "expected /update explicit entrypoint: {entrypoints:?}"
    );
    let description = update["lineage"]["description"]
        .as_str()
        .expect("update lineage description");
    assert!(
        description.contains("Refresh key docs, git tracking, and stale/dead repo surfaces"),
        "update description should describe repo knowledge/hygiene maintenance: {description}"
    );
    let trigger_hints = update["trigger_hints"]
        .as_array()
        .expect("update trigger_hints");
    for hint in [
        "更新关键文档",
        "科研文档更新",
        "git 跟踪文件",
        "死代码清理",
        "旧文档清理",
        "stale files",
        "dead code",
    ] {
        assert!(
            trigger_hints
                .iter()
                .filter_map(|v| v.as_str())
                .any(|v| v == hint),
            "missing update trigger hint: {hint}"
        );
    }
}

#[test]
fn framework_command_slugs_in_manifest() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let keys = runtime["keys"].as_array().expect("runtime keys");
    let slug_idx = key_index(keys, "slug");
    let runtime_slugs: HashSet<String> = runtime["skills"]
        .as_array()
        .expect("runtime skills")
        .iter()
        .filter_map(|row| row.get(slug_idx).and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    for slug in FRAMEWORK_COMMAND_IDS {
        assert!(
            runtime_slugs.contains(*slug),
            "SKILL_ROUTING_RUNTIME must contain framework command `{slug}` (not runtime-only)"
        );
    }
}

#[test]
fn runtime_framework_command_rows_match_manifest() {
    let root = project_root();
    let runtime = read_json(&root.join("skills/SKILL_ROUTING_RUNTIME.json"));
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let supported_host_set: HashSet<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();
    let runtime_keys = runtime["keys"].as_array().expect("runtime keys");
    let r_slug = key_index(runtime_keys, "slug");
    let r_layer = key_index(runtime_keys, "layer");
    let r_kind = key_index(runtime_keys, "kind");
    let r_summary = runtime_description_index(runtime_keys);
    let r_hosts = runtime_host_platforms_index(runtime_keys);
    let r_skill_path = key_index(runtime_keys, "skill_path");
    let r_trigger_hints = key_index(runtime_keys, "trigger_hints");

    for row in runtime["skills"].as_array().expect("runtime skills") {
        let row = row.as_array().expect("runtime row");
        let slug = row[r_slug].as_str().expect("runtime slug");
        if !FRAMEWORK_COMMAND_IDS.contains(&slug) {
            continue;
        }
        // Verify basic field existence (no cross-source comparison)
        assert!(
            row[r_layer].as_str().is_some(),
            "{slug}: layer must be present"
        );
        assert!(
            row[r_kind].as_str().is_some(),
            "{slug}: kind must be present"
        );
        assert!(
            row[r_summary].as_str().is_some(),
            "{slug}: description/summary must be present"
        );
        let runtime_hosts: HashSet<String> = row[r_hosts]
            .as_array()
            .expect("runtime host_platforms")
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        for host in &runtime_hosts {
            assert!(
                supported_host_set.contains(host),
                "{slug}: host `{host}` not in RUNTIME_REGISTRY"
            );
        }
        assert!(
            row[r_skill_path].as_str().is_some(),
            "{slug}: skill_path must be present"
        );
        assert!(
            row[r_trigger_hints].as_array().is_some(),
            "{slug}: trigger_hints must be present"
        );
    }
}

#[test]
fn host_projection_narrative_covers_installable_hosts() {
    let root = project_root();
    let narrative = read_json(&root.join("configs/framework/host_projection_narrative.json"));
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let default = narrative["default_lifecycle_paragraph"]
        .as_str()
        .expect("default_lifecycle_paragraph");
    assert!(
        default.contains("My lifecycle") || default.contains("Default lifecycle"),
        "default_lifecycle_paragraph must reference My lifecycle or Default lifecycle"
    );
    let by_host = narrative["lifecycle_by_host"]
        .as_object()
        .expect("lifecycle_by_host object");
    let host_targets = registry["host_targets"]["metadata"]
        .as_object()
        .expect("host_targets.metadata");
    for (host_id, meta) in host_targets {
        if meta.get("installable").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        if meta
            .get("deprecated_alias_of")
            .and_then(Value::as_str)
            .is_some()
        {
            continue;
        }
        let paragraph = by_host
            .get(host_id)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("lifecycle_by_host missing installable host {host_id}"));
        assert!(
            paragraph.contains("My lifecycle") || paragraph.contains("Default lifecycle"),
            "{host_id}: lifecycle paragraph must reference My lifecycle or Default lifecycle"
        );
    }
}

#[test]
fn framework_aliases_reference_manifest_skills() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let runtime_keys = runtime["keys"].as_array().expect("runtime keys");
    let runtime_slug_idx = key_index(runtime_keys, "slug");
    let runtime_slugs = runtime["skills"]
        .as_array()
        .expect("runtime skills")
        .iter()
        .map(|row| row[runtime_slug_idx].as_str().expect("runtime slug"))
        .collect::<HashSet<_>>();

    let registry = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    for (alias, record) in registry["framework_commands"]
        .as_object()
        .expect("framework commands")
    {
        if let Some(owner) = record
            .get("canonical_owner")
            .and_then(|value| value.as_str())
        {
            assert!(
                manifest_or_runtime_lane_contains(&runtime_slugs, owner),
                "framework alias {alias} canonical_owner references missing slug {owner}"
            );
        }
        for slug in record
            .get("execution_owners")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
        {
            assert!(
                manifest_or_runtime_lane_contains(&runtime_slugs, slug),
                "framework alias {alias} execution_owners references missing slug {slug}"
            );
        }
    }
}
