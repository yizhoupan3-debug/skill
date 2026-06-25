// Test helper functions that may not all be called from every test.
#![allow(dead_code)]

use crate::common::{project_root, read_json, read_text};
use crate::policy::policy_helpers::{
    allowed_python_control_plane_path, collect_files, collect_files_with_extension,
    RETIRED_RUNTIME_OWNED_SKILL_SLUGS,
};
use core_policy::doc_registry;
use std::collections::HashSet;
use std::path::PathBuf;

fn retired_runtime_owned_skill_slugs() -> HashSet<&'static str> {
    RETIRED_RUNTIME_OWNED_SKILL_SLUGS.iter().copied().collect()
}

#[test]
fn repo_local_plugin_wrapper_stays_removed() {
    assert!(
        !project_root()
            .join("plugins/skill-framework-native")
            .exists()
    );
    assert!(
        !project_root()
            .join("plugins/skill-framework-native/.mcp.json")
            .exists()
    );
}

#[test]
fn agents_marketplace_surface_stays_removed() {
    assert!(!project_root().join(".agents").exists());
}

#[test]
fn readme_does_not_revive_cursor_hook_shell_shims_in_cursor_section() {
    let readme = read_text(&project_root().join("README.md"));
    for forbidden in [
        "review-gate.sh",
        "post-tool-use.sh",
        "session-start.sh",
        "precompact-full.sh",
        "rustfmt.sh",
        "resolve-router-rs.sh",
    ] {
        assert!(
            !readme.contains(forbidden),
            "README.md must not reference retired Cursor hook shim {forbidden}; use router-rs cursor hook"
        );
    }
}

#[test]
fn refresh_skill_stays_out_of_project_host_entrypoints() {
    assert!(!project_root().join("skills/refresh/SKILL.md").exists());
    assert!(!project_root().join(".codex/skills/refresh").exists());
    let registry = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    assert!(registry["framework_commands"]["refresh"].is_null());
}

#[test]
fn rfv_harness_reference_moved_to_docs() {
    assert!(
        !project_root()
            .join("skills/review-fix-verify-loop/SKILL.md")
            .exists()
    );
    // RFV harness 文档已整合入 codebase 模块文档，不再作为独立文件存在。
    // 实现逻辑见 core/runtime-core/src/rfv_loop.rs。
}

#[test]
fn retired_runtime_owned_skill_directories_stay_removed() {
    let existing = retired_runtime_owned_skill_slugs()
        .into_iter()
        .map(|slug| project_root().join("skills").join(slug).join("SKILL.md"))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    assert_eq!(existing, Vec::<PathBuf>::new());
}

#[test]
fn doc_and_xlsx_skills_have_no_python_scripts() {
    for skill in ["skills/doc", "skills/primary-runtime/spreadsheets"] {
        assert!(
            collect_files_with_extension(&project_root().join(skill), "py").is_empty(),
            "{skill} still contains Python scripts"
        );
    }
}

#[test]
fn github_source_gate_python_helpers_stay_removed() {
    for skill in ["skills/gh-fix-ci", "skills/gh-address-comments"] {
        let skill_path = project_root().join(skill);
        assert!(!skill_path.join("scripts").exists());
        assert!(collect_files_with_extension(&skill_path, "py").is_empty());
    }
}

#[test]
fn generated_routing_surfaces_do_not_reference_removed_python_helpers() {
    let generated = [
        "skills/SKILL_ROUTING_RUNTIME.json",
    ]
    .iter()
    .map(|path| read_text(&project_root().join(path)))
    .collect::<Vec<_>>()
    .join("\n");
    assert!(!generated.contains("inspect_pr_checks.py"));
    assert!(!generated.contains("fetch_comments.py"));
    assert!(generated.contains("gh-source-gate"));
}

#[test]
fn removed_router_flags_are_absent_from_user_docs() {
    let docs = doc_registry::all_keys()
    .iter()
    .map(|path| read_text(&project_root().join(path)))
    .collect::<Vec<_>>()
    .join("\n");

    for removed_flag in [
        "--framework-refresh-json",
        "--framework-refresh-verbose",
        "--sync-host-entrypoints-json",
        "router-rs --execute-json",
    ] {
        assert!(
            !docs.contains(removed_flag),
            "removed flag leaked: {removed_flag}"
        );
    }
}

#[test]
fn gsd_slash_commands_removed_from_runtime_and_hooks() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let registry_text = read_text(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    assert!(
        registry
            .get("framework_commands")
            .and_then(|v| v.get("gsd"))
            .is_none(),
        "framework_commands.gsd must stay removed"
    );
    assert!(
        !registry_text.contains("/gsd-"),
        "RUNTIME_REGISTRY must not reference /gsd- commands"
    );
    let hook_common = read_text(&root.join("core/core-policy/src/hook_common/mod.rs"));
    assert!(
        !hook_common.contains("/gsd-"),
        "hook_common must not recognize /gsd- entrypoints"
    );
}

#[test]
fn removed_python_adapter_bridges_stay_removed() {
    let removed_legacy_files = [
        "scripts/route.py",
        "scripts/router_rs_runner.py",
        "scripts/codex_omx_hook_bridge.py",
        "scripts/install_codex_framework_default.py",
        "scripts/runtime_background_cli.py",
        "scripts/rust_binary_runner",
        "scripts/rust_binary_runner.py",
        "configs/codex/model_instructions.md",
        "scripts/materialize_cli_host_entrypoints.py",
        "scripts/install_codex_native_integration.py",
        "scripts/write_session_artifacts.py",
        "scripts/host_integration_runner.py",
        "skills/autoresearch/scripts/research_ctl.py",
        "skills/autoresearch/scripts/init_research.py",
    ];
    let existing: Vec<_> = removed_legacy_files
        .iter()
        .map(|path| project_root().join(path))
        .filter(|path| path.exists())
        .collect();
    assert_eq!(existing, Vec::<PathBuf>::new());
}

#[test]
fn framework_runtime_python_package_stays_removed() {
    assert!(!project_root().join("framework_runtime").exists());
}

#[test]
fn repo_local_codex_omits_framework_mcp_entrypoint() {
    // .codex/ 目录在未运行 install-skills codex 时不存在
    assert!(!project_root().join(".codex").exists(), ".codex/ directory must not exist in repo (only generated on install)");
}

#[test]
fn repo_stays_free_of_legacy_python_source_and_pytest_entrypoints() {
    let root = project_root();
    let mut violations = Vec::new();
    collect_files(&root, &mut |path| {
        let extension = path.extension().and_then(|ext| ext.to_str());
        let file_name = path.file_name().and_then(|name| name.to_str());
        if matches!(extension, Some("py" | "pyc")) || file_name == Some("pytest.ini") {
            let rel = path.strip_prefix(&root).unwrap_or(path);
            if allowed_python_control_plane_path(rel) {
                return;
            }
            violations.push(rel.display().to_string());
        }
    });
    violations.sort();
    assert!(
        violations.is_empty(),
        "Python source/cache/test entrypoints must stay removed:\n{}",
        violations.join("\n")
    );
}
