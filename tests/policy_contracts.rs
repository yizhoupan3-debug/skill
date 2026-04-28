mod common;

use common::{
    cargo_manifest_command, json_from_output, project_root, read_json, read_text, router_rs_json,
    run,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[test]
fn plugin_manifest_exposes_skills_without_mcp_bundle() {
    let plugin_root = project_root().join("plugins/skill-framework-native");
    let manifest = read_json(&plugin_root.join(".codex-plugin/plugin.json"));
    assert_eq!(manifest["name"], "skill-framework-native");
    assert_eq!(manifest["skills"], "./skills/");
    assert!(manifest.get("mcpServers").is_none());
    assert_eq!(
        manifest["interface"]["displayName"],
        "Skill Framework Native"
    );
}

#[test]
fn plugin_mcp_bundle_is_removed() {
    assert!(!project_root()
        .join("plugins/skill-framework-native/.mcp.json")
        .exists());
}

#[test]
fn agents_marketplace_surface_stays_removed() {
    assert!(!project_root().join(".agents").exists());
}

#[test]
fn gitx_skill_exposes_codex_shortcut_and_closeout_flow() {
    let content = read_text(&project_root().join("skills/gitx/SKILL.md"));
    for marker in [
        "name: gitx",
        "$gitx",
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
fn refresh_skill_stays_out_of_project_host_entrypoints() {
    assert!(!project_root().join(".codex/skills/refresh").exists());
    assert!(!project_root()
        .join("artifacts/codex-skill-surface/skills/refresh")
        .exists());
    let controller = read_text(&project_root().join("skills/execution-controller-coding/SKILL.md"));
    assert!(!controller.contains("$autopilot"));
    assert!(!controller.contains("autopilot"));
}

#[test]
fn project_codex_skill_projection_is_generated_outside_host_entrypoints() {
    assert!(!project_root().join(".codex/skills").exists());
    assert!(!project_root().join("AGENT.md").exists());
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    router_rs_json(&["codex", "sync", "--repo-root", repo_root.to_str().unwrap()]);
    let manifest = read_json(&repo_root.join(".codex/host_entrypoints_sync_manifest.json"));
    let manifest_text = manifest.to_string();
    assert!(!manifest_text.contains(".codex/skills/gitx"));
    assert!(!manifest_text.contains(".codex/skills/autopilot"));
    assert!(!project_root().join("CLAUDE.md").exists());
    assert_eq!(
        manifest["shared_system"]["supported_hosts"],
        serde_json::json!(["codex-cli", "codex-app"])
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["codex-cli"],
        "AGENTS.md"
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["codex-app"],
        "AGENTS.md"
    );
    assert!(manifest["full_sync"]["text_files"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("AGENTS.md")));
    assert!(!manifest_text.contains("CLAUDE.md"));
    assert!(!manifest_text.contains("claude-code"));
    assert!(!manifest_text.contains("retired_files"));
    assert!(!manifest_text.contains("retired_directories"));
    assert!(!manifest_text.contains("AGENT.md"));
    assert!(!manifest_text.contains(".codex/README.md"));
    assert!(!manifest_text.contains(".claude/commands"));
    assert!(!manifest_text.contains(".claude/hooks"));
    assert!(!manifest_text.contains(".claude/skills"));
}

#[test]
fn codex_user_skill_surface_stays_lightweight_and_explicit() {
    let surface_root = project_root().join("artifacts/codex-skill-surface/skills");
    let manifest_path = surface_root.join(".codex-skill-surface.json");
    if !manifest_path.exists() {
        return;
    }
    let manifest = read_json(&manifest_path);
    let skills = manifest["skills"].as_array().unwrap();
    assert!(
        skills.len() < 40,
        "surface loaded too many skills: {}",
        skills.len()
    );
    assert!(skills.iter().any(|item| item.as_str() == Some("autopilot")));
    assert!(skills.iter().any(|item| item.as_str() == Some("gitx")));
    assert!(skills
        .iter()
        .any(|item| item.as_str() == Some("deepinterview")));
    assert!(skills.iter().any(|item| item.as_str() == Some("team")));
    assert!(!skills.iter().any(|item| item.as_str() == Some("refresh")));
    assert!(surface_root.join("autopilot/SKILL.md").exists());
    assert!(surface_root.join("gitx/SKILL.md").exists());
    assert!(surface_root.join("deepinterview/SKILL.md").exists());
    assert!(surface_root.join("team/SKILL.md").exists());
    let autopilot = read_text(&surface_root.join("autopilot/SKILL.md"));
    let team = read_text(&surface_root.join("team/SKILL.md"));
    assert!(autopilot.contains("`$autopilot`"));
    assert!(autopilot.contains("`/autopilot`"));
    assert!(team.contains("`$team`"));
    assert!(team.contains("`/team`"));
}

#[test]
fn latex_compile_acceleration_discovery_surface_is_precise() {
    let content = read_text(&project_root().join("skills/latex-compile-acceleration/SKILL.md"));
    for marker in [
        "name: latex-compile-acceleration",
        "session_start: n/a",
        "LaTeX 编译太慢",
        "TikZ externalization",
        "preamble 预编译",
        "Prefer this skill over ppt-beamer",
        "## Do not use",
    ] {
        assert!(content.contains(marker), "missing marker: {marker}");
    }
    assert!(content.lines().count() <= 180);
}

#[test]
fn latex_compile_acceleration_keeps_rust_boundary_clear() {
    let content = read_text(&project_root().join("skills/latex-compile-acceleration/SKILL.md"));
    let techniques = read_text(
        &project_root().join("skills/latex-compile-acceleration/references/techniques.md"),
    );
    for marker in [
        "This skill is **not fully Rust**",
        "Rust owns host entrypoints, alias projection, durable lane orchestration",
        "LaTeX diagnosis and tactic choice stay in this skill",
        "Do not present Rustification as the default fix",
    ] {
        assert!(content.contains(marker), "missing marker: {marker}");
    }
    assert!(techniques.contains("Rust should not hard-code the LaTeX tactic decision"));
}

#[test]
fn latex_compile_acceleration_reference_has_operational_playbook() {
    let techniques = read_text(
        &project_root().join("skills/latex-compile-acceleration/references/techniques.md"),
    );
    for marker in [
        "## Fast measurement pack",
        r#"latexmk -C "$MAIN""#,
        "/usr/bin/time -p latexmk",
        "## Decision tree",
        "## `.latexmkrc` recipes",
        "## Cache invalidation checklist",
        "## Validation checklist",
    ] {
        assert!(techniques.contains(marker), "missing marker: {marker}");
    }
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
fn doc_and_xlsx_skill_docs_point_to_rust_tooling() {
    let docs = markdown_text_under(&[
        project_root().join("skills/doc"),
        project_root().join("skills/primary-runtime/spreadsheets"),
    ]);
    for forbidden in [
        "openpyxl",
        "pandas",
        "python-docx",
        "pdf2image",
        "render_docx.py",
        "render_xlsx.py",
        "inspect_xlsx.py",
    ] {
        assert!(
            !docs.contains(forbidden),
            "forbidden token present: {forbidden}"
        );
    }
    for marker in [
        "ooxml_parser_rs",
        "render-docx",
        "render-xlsx",
        " -- docx <docx>",
    ] {
        assert!(docs.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn doc_and_xlsx_agent_prompts_are_rust_first() {
    let prompts = [
        project_root().join("skills/doc/agents/openai.yaml"),
        project_root().join("skills/primary-runtime/spreadsheets/agents/openai.yaml"),
    ]
    .iter()
    .map(|path| read_text(path))
    .collect::<Vec<_>>()
    .join("\n");
    assert!(prompts.contains("Rust-first"));
    assert!(prompts.contains("Rust OOXML CLI"));
}

#[test]
fn ooxml_rust_cli_owns_docx_and_xlsx_render_commands() {
    let source = read_text(&project_root().join("rust_tools/ooxml_parser_rs/src/main.rs"));
    for marker in [
        "Docx { input, json }",
        "RenderXlsx(RenderXlsxArgs)",
        "RenderDocx(RenderDocxArgs)",
        "fn inspect_docx(",
        "fn render_xlsx(",
        "fn render_docx(",
    ] {
        assert!(source.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn ooxml_cli_help_lists_docx_and_xlsx_render_commands() {
    let output = common::run_ok(cargo_manifest_command(
        &project_root().join("rust_tools/ooxml_parser_rs/Cargo.toml"),
        &["--help"],
    ));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("docx"));
    assert!(stdout.contains("render-docx"));
    assert!(stdout.contains("render-xlsx"));
}

#[test]
fn router_rs_top_level_help_exposes_only_canonical_subcommands() {
    let output = common::run_ok(cargo_manifest_command(
        &project_root().join("scripts/router-rs/Cargo.toml"),
        &["--help"],
    ));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "route",
        "search",
        "framework",
        "codex",
        "claude",
        "trace",
        "storage",
        "browser",
        "profile",
        "migrate",
    ] {
        assert!(stdout.contains(command), "missing command: {command}");
    }
    for retired in [
        "route-json",
        "framework-runtime-snapshot-json",
        "host-integration",
        "browser-mcp-stdio",
        "profile-json",
    ] {
        assert!(!stdout.contains(retired), "retired flag leaked: {retired}");
    }
}

#[test]
fn github_source_gate_python_helpers_are_retired() {
    for skill in ["skills/gh-fix-ci", "skills/gh-address-comments"] {
        let skill_path = project_root().join(skill);
        assert!(!skill_path.join("scripts").exists());
        assert!(collect_files_with_extension(&skill_path, "py").is_empty());
    }
}

#[test]
fn github_source_gate_docs_point_to_rust_cli_only() {
    let docs = markdown_text_under(&[
        project_root().join("skills/gh-fix-ci"),
        project_root().join("skills/gh-address-comments"),
    ]);
    for marker in [
        "gh_source_gate_rs",
        "gh-source-gate",
        "inspect-pr-checks",
        "fetch-comments",
    ] {
        assert!(docs.contains(marker), "missing marker: {marker}");
    }
    assert!(!docs.contains("inspect_pr_checks.py"));
    assert!(!docs.contains("fetch_comments.py"));
    assert!(!docs.to_lowercase().contains("python"));
}

#[test]
fn generated_routing_surfaces_do_not_reference_retired_python_helpers() {
    let generated = [
        "skills/SKILL_MANIFEST.json",
        "skills/SKILL_ROUTING_RUNTIME.json",
        "skills/SKILL_ROUTING_REGISTRY.md",
        "skills/SKILL_ROUTING_INDEX.md",
        "skills/SKILL_APPROVAL_POLICY.json",
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
fn retired_router_flags_are_absent_from_user_docs() {
    let docs = [
        "skills/refresh/SKILL.md",
        "RTK.md",
        "docs/rust_contracts.md",
    ]
    .iter()
    .map(|path| read_text(&project_root().join(path)))
    .collect::<Vec<_>>()
    .join("\n");

    for retired in [
        "--framework-refresh-json",
        "--framework-refresh-verbose",
        "--sync-host-entrypoints-json",
        "router-rs --execute-json",
    ] {
        assert!(!docs.contains(retired), "retired flag leaked: {retired}");
    }
    assert!(docs.contains("framework refresh --repo-root"));
    assert!(docs.contains("codex sync --repo-root"));
    assert!(docs.contains("stdio `execute` operation"));
}

#[test]
fn framework_surface_policy_is_the_activation_source_of_truth() {
    let surface =
        read_json(&project_root().join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"));
    let tiers = read_json(&project_root().join("skills/SKILL_TIERS.json"));
    let loadouts = read_json(&project_root().join("skills/SKILL_LOADOUTS.json"));

    assert_eq!(surface["source_of_truth"], true);
    assert_eq!(
        surface["derived_reports"],
        serde_json::json!(["skills/SKILL_TIERS.json"])
    );
    assert_eq!(
        surface["deprecated_or_foldable_reports"],
        serde_json::json!(["skills/SKILL_LOADOUTS.json"])
    );
    assert_eq!(tiers["source_of_truth"], false);
    assert_eq!(
        tiers["derived_from"],
        "configs/framework/FRAMEWORK_SURFACE_POLICY.json"
    );
    assert_eq!(tiers["report_status"], "generated_debug_report");
    assert_eq!(loadouts["source_of_truth"], false);
    assert_eq!(
        loadouts["derived_from"],
        "configs/framework/FRAMEWORK_SURFACE_POLICY.json"
    );
    assert_eq!(loadouts["report_status"], "foldable_generated_report");
    assert_eq!(
        surface["skill_system"]["activation_counts"],
        tiers["summary"]["activation_counts"]
    );
}

#[test]
fn runtime_protocol_uses_behavior_driven_public_names() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let checklist = runtime["checklist"]
        .as_array()
        .expect("runtime checklist")
        .iter()
        .map(|item| item.as_str().expect("checklist item"))
        .collect::<Vec<_>>()
        .join("\n");
    for marker in ["讨论:", "规划:", "执行:", "验证:"] {
        assert!(
            checklist.contains(marker),
            "missing protocol marker: {marker}"
        );
    }
    for stale in ["规范:", "计划:", "实施:"] {
        assert!(
            !checklist.contains(stale),
            "stale protocol marker leaked: {stale}"
        );
    }
    assert!(checklist.contains(
        "Completion pressure changes route context only; it must not change selected owner."
    ));
}

#[test]
fn runtime_hot_index_keeps_capability_gates_explicit() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let slugs = runtime["skills"]
        .as_array()
        .expect("runtime skills")
        .iter()
        .map(|skill| skill[0].as_str().expect("runtime skill slug"))
        .collect::<Vec<_>>();

    assert_eq!(runtime["scope"]["kind"], "hot");
    assert_eq!(
        runtime["scope"]["fallback_manifest"],
        "skills/SKILL_MANIFEST.json"
    );
    for expected in [
        "subagent-delegation",
        "skill-creator",
        "skill-installer",
        "idea-to-plan",
        "plan-to-code",
        "skill-framework-developer",
    ] {
        assert!(
            slugs.contains(&expected),
            "missing hot runtime slug: {expected}"
        );
    }
    assert_eq!(runtime["scope"]["hot_skill_count"], slugs.len());
}

#[test]
fn github_source_gate_rust_cli_is_workspace_member() {
    let manifest = read_text(&project_root().join("rust_tools/Cargo.toml"));
    assert!(manifest.contains(r#""gh_source_gate_rs""#));
    assert!(project_root()
        .join("rust_tools/gh_source_gate_rs/Cargo.toml")
        .exists());
}

#[test]
fn github_source_gate_rust_cli_owns_both_commands() {
    let source = read_text(&project_root().join("rust_tools/gh_source_gate_rs/src/main.rs"));
    for marker in [
        "InspectPrChecks(InspectPrChecksArgs)",
        "FetchComments(FetchCommentsArgs)",
        "fn inspect_pr_checks(",
        "fn fetch_comments(",
        "REVIEW_THREADS_QUERY",
    ] {
        assert!(source.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn github_source_gate_help_lists_commands() {
    let mut command = cargo_manifest_command(
        &project_root().join("rust_tools/gh_source_gate_rs/Cargo.toml"),
        &[],
    );
    command.args(["--bin", "gh-source-gate", "--", "--help"]);
    let output = run(command);
    common::assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("inspect-pr-checks"));
    assert!(stdout.contains("fetch-comments"));
}

#[test]
fn retired_python_adapter_bridges_stay_removed() {
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
fn autoresearch_uses_rust_only_controller() {
    let skill_dir = project_root().join("skills/autoresearch");
    let skill_doc = read_text(&skill_dir.join("SKILL.md"));
    assert!(project_root()
        .join("scripts/autoresearch-rs/src/main.rs")
        .exists());
    assert!(!skill_dir.join("scripts").exists());
    assert!(skill_doc.contains("scripts/autoresearch-rs"));
    assert!(!skill_doc.contains("research_ctl.py"));
    assert!(!skill_doc.contains("init_research.py"));
}

#[test]
fn installed_project_hooks_stay_disabled() {
    assert!(!project_root().join(".codex/hooks.json").exists());
    let config = read_text(&project_root().join(".codex/config.toml"));
    assert!(config.contains("codex_hooks = false"));
    assert!(!config.contains("codex_hooks = true"));
    let manifest = read_json(&project_root().join(".codex/host_entrypoints_sync_manifest.json"));
    assert!(!manifest.to_string().contains(".codex/hooks.json"));
}

#[test]
fn claude_hooks_are_rust_owned_and_low_token() {
    let settings = read_json(&project_root().join(".claude/settings.json"));
    let settings_text = settings.to_string();
    for event in ["PreToolUse", "UserPromptSubmit", "PostToolUse", "Stop"] {
        assert!(settings["hooks"].get(event).is_some(), "missing hook event: {event}");
    }
    assert!(settings_text.contains("scripts/router-rs/run_router_rs.sh"));
    assert!(settings_text.contains("Claude hook router launcher not found"));
    assert!(settings_text.contains("claude hook pre-tool-use"));
    assert!(settings_text.contains("claude hook user-prompt-submit"));
    assert!(settings_text.contains("claude hook post-tool-use"));
    assert!(settings_text.contains("claude hook stop"));
    assert!(!settings_text.contains("cargo run"));
    assert!(!settings_text.contains(".codex/hooks.json"));

    let output = common::run_ok({
        let mut command = common::router_rs_command([
            "claude",
            "hook",
            "pre-tool-use",
            "--repo-root",
            project_root().to_str().unwrap(),
        ]);
        command.stdin(std::process::Stdio::piped());
        command
    });
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["suppressOutput"], true);
}

#[test]
fn claude_pre_tool_hook_denies_destructive_git() {
    let mut command = common::router_rs_command([
        "claude",
        "hook",
        "pre-tool-use",
        "--repo-root",
        project_root().to_str().unwrap(),
    ]);
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(
                br#"{"tool_name":"Bash","tool_input":{"command":"git push -f origin main"}}"#,
            )
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    common::assert_success(&output);
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        payload["hookSpecificOutput"]["permissionDecision"],
        "deny"
    );
}

#[test]
fn claude_post_tool_state_blocks_stop_until_validation() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path();
    std::fs::create_dir_all(repo_root.join(".claude")).unwrap();

    let mut post = common::router_rs_command([
        "claude",
        "hook",
        "post-tool-use",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    post.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let mut child = post.spawn().unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(br#"{"tool_name":"Write","tool_input":{"file_path":".claude/settings.json"}}"#)
            .unwrap();
    }
    common::assert_success(&child.wait_with_output().unwrap());

    let output = router_rs_json(&[
        "claude",
        "hook",
        "stop",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    assert_eq!(output["decision"], "block");
    assert!(output["reason"].as_str().unwrap().contains("Validate Claude"));
}

#[test]
fn repo_local_codex_omits_framework_mcp_entrypoint() {
    let source = read_text(&project_root().join(".codex/config.toml"));
    assert!(!source.contains("python3"));
    assert!(!source.contains("scripts.framework_mcp"));
    assert!(!source.contains(r#"command = "cargo""#));
    assert!(!source.contains("[mcp_servers.framework-mcp]"));
    assert!(!source.contains("--framework-mcp-stdio"));
}

#[test]
fn browser_mcp_live_config_never_points_to_node_runtime() {
    let surfaces = [
        ".codex/config.toml",
        "tools/browser-mcp/scripts/start_browser_mcp.sh",
        "tools/browser-mcp/README.md",
    ];
    let joined = surfaces
        .iter()
        .map(|path| read_text(&project_root().join(path)))
        .collect::<Vec<_>>()
        .join("\n");
    let dist_entrypoint = format!("{}/{}.{}", "dist", "index", "js");
    let node_entrypoint = ["node".to_string(), dist_entrypoint.clone()].join(" ");
    let quoted_dist_entrypoint = [dist_entrypoint, "\"".to_string()].concat();
    assert!(!joined.contains(&node_entrypoint));
    assert!(!joined.contains(&quoted_dist_entrypoint));
    assert!(!joined.contains("npm run dev"));
}

#[test]
fn install_skills_uses_rust_only_entrypoints() {
    assert!(!project_root().join("scripts/install_skills.sh").exists());
    let source = read_text(&project_root().join("scripts/router-rs/src/host_integration.rs"));
    for marker in [
        "InstallSkills",
        "InstallNativeIntegration",
        "validate_default_bootstrap",
    ] {
        assert!(source.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn sync_skills_uses_router_rs_directly() {
    assert!(!project_root().join("scripts/sync_skills.py").exists());
    let source = read_text(&project_root().join("scripts/router-rs/src/codex_hooks.rs"));
    assert!(source.contains("sync_host_entrypoints"));
}

#[test]
fn memory_automation_lives_in_rust_host_integration() {
    let source = read_text(&project_root().join("scripts/router-rs/src/host_integration.rs"));
    assert!(source.contains("RunMemoryAutomation"));
    assert!(source.contains("run_memory_automation("));
}

#[test]
fn memory_and_prompt_policy_are_rust_owned() {
    let source = read_text(&project_root().join("scripts/router-rs/src/framework_runtime.rs"));
    assert!(source.contains("FRAMEWORK_MEMORY_POLICY_AUTHORITY"));
    assert!(source.contains("rust-framework-memory-policy"));
    assert!(source.contains("build_framework_memory_policy_envelope"));
    assert!(source.contains("build_framework_prompt_compression_envelope"));
    assert!(source.contains("prompt_policy_owner"));
}

#[test]
fn screenshot_skill_uses_workspace_rust_binary_entrypoint() {
    let skill_doc = read_text(&project_root().join("skills/screenshot/SKILL.md"));
    let reference_doc =
        read_text(&project_root().join("skills/screenshot/references/os_commands.md"));
    let manifest = read_text(&project_root().join("rust_tools/screenshot_rs/Cargo.toml"));
    assert!(manifest.contains("[[bin]]\nname = \"screenshot\""));
    assert!(!manifest.contains("[[bin]]\nname = \"screenshot_rs\""));
    assert!(skill_doc.contains("rust_tools/Cargo.toml --release --bin screenshot"));
    assert!(reference_doc.contains("rust_tools/Cargo.toml --release --bin screenshot"));
    assert!(!skill_doc.contains("rust_tools/screenshot_rs/Cargo.toml --release"));
    assert!(!reference_doc.contains("rust_tools/screenshot_rs/Cargo.toml --release"));
}

#[test]
fn openai_proxy_config_does_not_commit_plaintext_api_keys() {
    let config = read_text(&project_root().join("openai_proxy/config.yaml"));
    let start_script = read_text(&project_root().join("openai_proxy/start.sh"));
    assert!(config.contains("__OPENAI_PROXY_API_KEY__"));
    assert!(!config.contains("qscxzaq75321470"));
    assert!(!config.contains("sk-aggregator-"));
    assert!(start_script.contains("OPENAI_PROXY_API_KEY"));
}

#[test]
fn ppt_skill_has_no_node_package_runtime() {
    let root = project_root().join("skills/ppt-pptx");
    assert!(!root.join("package.json").exists());
    assert!(!root.join("package-lock.json").exists());
    assert!(!root.join("assets/package.template.json").exists());
    assert!(!root.join("assets/ppt.commands.json").exists());
    assert!(collect_files_with_extension(&root, "js").is_empty());
    assert!(collect_files_with_extension(&root, "ts").is_empty());
}

#[test]
fn ppt_skill_scripts_are_not_runtime_contract() {
    assert!(
        collect_files_with_extension(&project_root().join("skills/ppt-pptx/scripts"), "py")
            .is_empty()
    );
    let skill = read_text(&project_root().join("skills/ppt-pptx/SKILL.md"));
    for forbidden in ["node", "npm", "PptxGenJS", "deck.js"] {
        assert!(
            !skill.contains(forbidden),
            "forbidden token present: {forbidden}"
        );
    }
}

#[test]
fn ppt_rust_manifest_exposes_direct_cli() {
    let manifest = read_text(&project_root().join("rust_tools/pptx_tool_rs/Cargo.toml"));
    assert!(manifest.contains("name = \"ppt\""));
    assert!(manifest.contains("path = \"src/bin/ppt.rs\""));
    assert!(project_root()
        .join("rust_tools/pptx_tool_rs/src/bin/ppt.rs")
        .exists());
}

#[test]
fn ppt_rust_cli_owns_workspace_and_outline_commands() {
    let source = read_text(&project_root().join("rust_tools/pptx_tool_rs/src/main.rs"));
    assert!(source.contains("Init(InitArgs)"));
    assert!(source.contains("Outline(OutlineArgs)"));
    assert!(source.contains("BuildQa(BuildQaArgs)"));
    assert!(source.contains("fn init_workspace("));
    assert!(source.contains("default_value = \"deck.plan.json\""));
    assert!(source.contains("workdir.join(\"deck.pptx\")"));
    assert!(source.contains("QualityMode::Strict"));
    assert!(source.contains("fn strict_quality_gate("));
    assert!(source.contains("fn write_pptx_package("));
    assert!(source.contains("fn build_pptx_slide_specs("));
    assert!(source.contains("fn rust_office_outline_value("));
    assert!(source.contains("fn rust_office_issues_value("));
    assert!(source.contains("fn rust_office_validate_value("));
    assert!(source.contains("rust-pptx-inspector"));
    assert!(source.contains("fn font_check_ok("));
    assert!(source.contains("fn inspector_ok("));
    assert!(source.contains("ok: bool"));
    assert!(!source.contains("officecli"));
}

#[test]
fn ppt_rust_cli_builds_editable_deck_without_node_assets() {
    let temp = tempdir().unwrap();
    let manifest = project_root().join("rust_tools/pptx_tool_rs/Cargo.toml");

    let mut init = cargo_manifest_command(&manifest, &[]);
    init.args(["--bin", "ppt", "--", "init"])
        .arg(temp.path())
        .arg("--json");
    common::assert_success(&run(init));

    let outline = temp.path().join("outline.json");
    assert!(temp.path().join("sources.md").is_file());
    let mut build = cargo_manifest_command(&manifest, &[]);
    build
        .args(["--bin", "ppt", "--", "outline"])
        .arg(&outline)
        .args(["--bootstrap", "--build", "--json"]);
    common::assert_success(&run(build));

    assert!(temp.path().join("deck.plan.json").is_file());
    assert!(temp.path().join("deck.pptx").is_file());
    assert!(temp.path().join("ppt.commands.json").is_file());
    assert!(!temp.path().join("deck.js").exists());
    assert!(!temp.path().join("package-lock.json").exists());

    let commands_manifest = read_json(&temp.path().join("ppt.commands.json"));
    assert_eq!(commands_manifest["runtime"].as_str(), Some("ppt"));
    let commands = commands_manifest["commands"].as_object().unwrap();
    assert!(commands
        .values()
        .all(|command| command.as_str().unwrap().starts_with("ppt ")));
    assert!(commands.contains_key("check_inspector"));
    assert!(commands.contains_key("watch_rust"));
    assert!(commands.contains_key("build_strict"));
    assert!(commands["check_rust"]
        .as_str()
        .unwrap()
        .contains("--fail-on-issues"));
    assert!(commands["build_strict"]
        .as_str()
        .unwrap()
        .contains("--quality strict"));

    let mut extract = cargo_manifest_command(&manifest, &[]);
    extract
        .args(["--bin", "ppt", "--", "extract-structure"])
        .arg(temp.path().join("deck.pptx"));
    let structure = json_from_output(&run(extract));
    assert_eq!(structure["slide_count"].as_u64(), Some(3));
    assert!(structure["slides"][0]["notes"]
        .as_str()
        .unwrap_or_default()
        .contains("Cover slide generated by the Rust ppt CLI."));

    let mut doctor = cargo_manifest_command(&manifest, &[]);
    doctor
        .args(["--bin", "ppt", "--", "office", "doctor"])
        .arg(temp.path().join("deck.pptx"))
        .arg("--json");
    let doctor_payload = json_from_output(&run(doctor));
    assert_eq!(doctor_payload["inspector_version"].as_str(), Some("0.1.0"));
    assert_eq!(doctor_payload["outline"]["total_slides"].as_u64(), Some(3));
    assert_eq!(doctor_payload["validation"]["ok"].as_bool(), Some(true));

    let mut strict = cargo_manifest_command(&manifest, &[]);
    strict
        .args(["--bin", "ppt", "--", "build-qa"])
        .arg("--workdir")
        .arg(temp.path())
        .args(["--quality", "strict", "--json"]);
    let strict_payload = json_from_output(&run(strict));
    assert_eq!(strict_payload["ok"].as_bool(), Some(true));

    let mut fonts = cargo_manifest_command(&manifest, &[]);
    fonts
        .args(["--bin", "ppt", "--", "detect-fonts"])
        .arg(temp.path().join("deck.pptx"))
        .arg("--json");
    let fonts_payload = json_from_output(&run(fonts));
    assert!(fonts_payload["ok"].is_boolean());

    let mut query = cargo_manifest_command(&manifest, &[]);
    query
        .args(["--bin", "ppt", "--", "office", "query"])
        .arg(temp.path().join("deck.pptx"))
        .args(["shape", "--text", "Rust", "--json"]);
    let query_payload = json_from_output(&run(query));
    assert!(query_payload["count"].as_u64().unwrap_or(0) > 0);

    let mut query_text = cargo_manifest_command(&manifest, &[]);
    query_text
        .args(["--bin", "ppt", "--", "office", "query"])
        .arg(temp.path().join("deck.pptx"))
        .args(["shape", "--text", "Rust"]);
    let query_text_output = common::run_ok(query_text);
    let query_stdout = String::from_utf8_lossy(&query_text_output.stdout);
    assert!(query_stdout.contains("success: true"));
    assert!(!query_stdout.trim_start().starts_with('{'));

    let mut batch = cargo_manifest_command(&manifest, &[]);
    batch
        .args(["--bin", "ppt", "--", "office", "batch"])
        .arg(temp.path().join("deck.pptx"))
        .args(["--commands", "set title"]);
    let batch_output = run(batch);
    assert!(!batch_output.status.success());
    assert!(String::from_utf8_lossy(&batch_output.stderr).contains("read-only Rust inspector"));
}

#[test]
fn ppt_skill_documents_design_and_aigc_gates() {
    let skill = read_text(&project_root().join("skills/ppt-pptx/SKILL.md"));
    let workflow = read_text(&project_root().join("skills/ppt-pptx/references/workflow.md"));
    let design_system =
        read_text(&project_root().join("skills/ppt-pptx/references/design-system.md"));
    let checklist = read_text(&project_root().join("skills/ppt-pptx/references/checklist.md"));

    for token in [
        "$design-md",
        "$frontend-design",
        "$visual-review",
        "$design-output-auditor",
        "$design-workflow-protocol",
        "$humanizer",
        "$copywriting",
        "$paper-writing",
        "Source Contract",
        "Text And Design Polishing Chain",
        "Rust inspection boost",
        "`deck.plan.json` stays the source of truth",
    ] {
        assert!(skill.contains(token), "missing skill token: {token}");
    }
    assert!(skill.contains(
        "outline -> text-owner polish -> DESIGN.md or visual contract -> deck.plan.json -> deck.pptx -> rendered\n\
PNG -> visual-review evidence -> design-output-auditor verdict -> ppt\n\
qa/build-qa sign-off"
    ));
    for marker in [
        "Copy Naturalization First",
        "Text Skill Loop",
        "$copywriting",
        "$paper-writing",
        "DESIGN.md / visual contract",
        "$visual-review",
        "$design-output-auditor",
        "match / minor drift / material drift /\nhard fail",
        "Run `ppt office doctor` for Rust outline",
        "Do not introduce a parallel authoring engine",
        "rendered PNGs or montage when visual QA mattered",
    ] {
        assert!(
            workflow.contains(marker),
            "missing workflow marker: {marker}"
        );
    }
    for field in [
        "Visual Theme & Atmosphere",
        "Color Palette & Roles",
        "Typography Rules",
        "Layout Principles",
        "Generation Guardrails",
        "Anti-Patterns",
        "$frontend-design",
        "deck.plan.json",
        "match",
        "minor drift",
        "material drift",
        "hard fail",
        "Rust builder can regenerate the deck without guessing",
        "prefer shapes, text, and simple structured chart/table",
    ] {
        assert!(
            design_system.contains(field),
            "missing design field: {field}"
        );
    }
    for marker in [
        "本页展示",
        "AI-slop",
        "$humanizer",
        "$copywriting",
        "$paper-writing",
        "Rendered slides reviewed through `$visual-review`",
        "Design audit verdict is `match` or only acceptable `minor drift`",
        "Run `ppt office doctor`",
        "Do not use alternate package wrappers, script templates, or external Office inspectors",
    ] {
        assert!(
            checklist.contains(marker),
            "missing checklist marker: {marker}"
        );
    }
}

#[test]
fn ppt_docs_are_rust_runtime_first() {
    let docs = markdown_text_under(&[project_root().join("skills/ppt-pptx")]);
    for forbidden in [
        "node scripts/smoke_test.js",
        "npm install",
        "PptxGenJS",
        "deck.js",
        "outline_to_deck.js",
        "officecli",
        "OfficeCLI",
    ] {
        assert!(!docs.contains(forbidden), "{forbidden}");
    }
    assert!(docs.contains("Rust CLI"));
    assert!(docs.contains("deck.plan.json"));
    assert!(docs.contains("deck.pptx"));
    assert!(docs.contains("Rust Inspector"));
    assert!(docs.contains("ppt.commands.json"));
    assert!(docs.contains("No separate inspector install is required"));
}

#[test]
fn ppt_skill_references_source_first_and_editable_rules() {
    let layout = read_text(&project_root().join("skills/ppt-pptx/references/layout-patterns.md"));
    let method = read_text(&project_root().join("skills/ppt-pptx/references/method.md"));
    let rust_cli = read_text(&project_root().join("skills/ppt-pptx/references/rust-cli.md"));
    let visualization =
        read_text(&project_root().join("skills/ppt-pptx/references/visualization_patterns.md"));
    let install = read_text(&project_root().join("skills/ppt-pptx/references/install.md"));

    assert!(layout.contains("Auto-Selection Rules"));
    assert!(layout.contains("choose the pattern that creates the clearest reading path"));
    assert!(method.contains("Rust Source-First Habit"));
    assert!(method.contains("change `deck.plan.json`, then rebuild"));
    assert!(rust_cli.contains("Rust `ppt office ...` owns inspection"));
    assert!(rust_cli.contains("not a package wrapper or\na second runtime"));
    assert!(rust_cli.contains("$humanizer` / `$copywriting` / `$paper-writing"));
    assert!(visualization.contains("Prefer editable primitives"));
    assert!(install.contains("There is no skill-local package install step"));
    assert!(install.contains("these companion skills make the text and design intentional"));
}

#[test]
fn slides_gate_is_executable_and_evidence_closed() {
    let skill = read_text(&project_root().join("skills/slides/SKILL.md"));
    for marker in [
        "Do not stop to ask for goal, audience, visual bar, or format when a safe default exists",
        "Re-run routing or consult the fallback manifest for that exact owner",
        "Rust `ppt` CLI",
        "cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/pptx_tool_rs/Cargo.toml --bin ppt -- <command>",
        "ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json",
        "## Existing PPTX Safety",
        "Preserve the original file by writing a new output path",
        "## Verification Standard",
        "ppt slides-test --fail-on-overflow",
        "ppt detect-fonts --json",
        "## Evidence Index",
        "EVIDENCE_INDEX.json",
        "Final response stays concise but includes the `.pptx` link and the verification evidence used",
        "workspace",
        "temp",
        "artifacts/scratch",
    ] {
        assert!(skill.contains(marker), "missing slides gate marker: {marker}");
    }
    assert!(!skill.contains("@oai/artifact-tool"));
    assert!(!skill.contains("compact verification pass"));
    assert!(!skill.contains("Final response contains only"));
}

#[test]
fn ppt_rust_outline_generation_naturalizes_copy_and_design_chain() {
    let source = read_text(&project_root().join("rust_tools/pptx_tool_rs/src/main.rs"));
    for marker in [
        "fn naturalize_outline_value(",
        "fn naturalize_copy_text(",
        "let outline = naturalize_outline_value(outline);",
        "generic AI filler",
        "$humanizer",
        "$copywriting",
        "$paper-writing",
        "design-output-auditor drift verdict",
        r#""本页展示""#,
        r#""赋能""#,
    ] {
        assert!(source.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn direct_ppt_cli_help_lists_authoring_commands() {
    let mut command = cargo_manifest_command(
        &project_root().join("rust_tools/pptx_tool_rs/Cargo.toml"),
        &[],
    );
    command.args(["--bin", "ppt", "--", "--help"]);
    let output = run(command);
    common::assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("init"));
    assert!(stdout.contains("outline"));
}

#[test]
fn direct_ppt_cli_outline_help_lists_quality_mode() {
    let mut command = cargo_manifest_command(
        &project_root().join("rust_tools/pptx_tool_rs/Cargo.toml"),
        &[],
    );
    command.args(["--bin", "ppt", "--", "outline", "--help"]);
    let output = run(command);
    common::assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--quality"));
    assert!(stdout.contains("--rendered-dir"));
}

#[test]
fn direct_ppt_cli_qa_help_lists_fail_gate() {
    let mut command = cargo_manifest_command(
        &project_root().join("rust_tools/pptx_tool_rs/Cargo.toml"),
        &[],
    );
    command.args(["--bin", "ppt", "--", "qa", "--help"]);
    let output = run(command);
    common::assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--fail-on-issues"));
}

#[test]
fn direct_ppt_cli_build_qa_help_lists_quality_mode() {
    let mut command = cargo_manifest_command(
        &project_root().join("rust_tools/pptx_tool_rs/Cargo.toml"),
        &[],
    );
    command.args(["--bin", "ppt", "--", "build-qa", "--help"]);
    let output = run(command);
    common::assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--quality"));
}

#[test]
fn repo_stays_free_of_python_source_and_pytest_entrypoints() {
    let root = project_root();
    let mut violations = Vec::new();
    collect_files(&root, &mut |path| {
        let extension = path.extension().and_then(|ext| ext.to_str());
        let file_name = path.file_name().and_then(|name| name.to_str());
        if matches!(extension, Some("py" | "pyc")) || file_name == Some("pytest.ini") {
            violations.push(
                path.strip_prefix(&root)
                    .unwrap_or(path)
                    .display()
                    .to_string(),
            );
        }
    });
    violations.sort();
    assert!(
        violations.is_empty(),
        "Python source/cache/test entrypoints must stay removed:\n{}",
        violations.join("\n")
    );
}

fn collect_files_with_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut results = Vec::new();
    collect_files(root, &mut |path| {
        if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            results.push(path.to_path_buf());
        }
    });
    results
}

fn markdown_text_under(roots: &[PathBuf]) -> String {
    let mut chunks = Vec::new();
    for root in roots {
        collect_files(root, &mut |path| {
            if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                chunks.push(read_text(path));
            }
        });
    }
    chunks.join("\n")
}

fn collect_files(root: &Path, visitor: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let directory_name = path.file_name().and_then(|name| name.to_str());
            if matches!(
                directory_name,
                Some(".git" | "target" | "node_modules" | ".venv" | "venv")
            ) {
                continue;
            }
            collect_files(&path, visitor);
        } else if path.is_file() {
            visitor(&path);
        }
    }
}
