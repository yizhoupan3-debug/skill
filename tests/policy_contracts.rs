mod common;
mod host_platforms;

use common::{
    assert_success, cargo_manifest_command, json_from_output, project_root, read_json, read_text,
    router_rs_json, run, seed_framework_markers,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use tempfile::tempdir;

const RETIRED_RUNTIME_OWNED_SKILL_SLUGS: &[&str] = &[
    "api-design",
    "api-integration-debugging",
    "api-load-tester",
    "architect-review",
    "auth-implementation",
    "backend-runtime-debugging",
    "build-tooling",
    "code-acceleration",
    "code-review",
    "codex-hook-builder",
    "coding-standards",
    "datastore-cache-queue",
    "dependency-migration",
    "docker",
    "env-config-management",
    "error-handling-patterns",
    "github-actions-authoring",
    "idea-to-plan",
    "linux-server-ops",
    "monorepo-tooling",
    "observability",
    "plan-to-code",
    "refactoring",
    "release-engineering",
    "security-audit",
    "security-threat-model",
    "shell-cli",
    "tdd-workflow",
    "test-engineering",
    "accessibility-auditor",
    "chrome-extension-dev",
    "css-pro",
    "frontend-debugging",
    "frontend-design",
    "go-pro",
    "i18n-l10n",
    "javascript-pro",
    "native-app-debugging",
    "nextjs",
    "node-backend",
    "npm-package-authoring",
    "python-pro",
    "react",
    "rust-pro",
    "seo-web",
    "sql-pro",
    "svelte",
    "typescript-pro",
    "vue",
    "web-platform-basics",
    "ai-research",
    "autoresearch",
    "chatgpt-apps",
    "cloudflare-deploy",
    "data-wrangling",
    "information-retrieval",
    "literature-synthesis",
    "mcp-builder",
    "performance-expert",
    "prompt-engineer",
    "research-engineer",
    "web-scraping",
];

const FRAMEWORK_COMMAND_IDS: &[&str] = &[
    "discussx",
    "planx",
    "implementx",
    "verifyx",
    "deepinterview",
    "gitx",
    "update",
];

fn retired_runtime_owned_skill_slugs() -> HashSet<&'static str> {
    RETIRED_RUNTIME_OWNED_SKILL_SLUGS.iter().copied().collect()
}

fn manifest_or_runtime_lane_contains(manifest_slugs: &HashSet<&str>, slug: &str) -> bool {
    slug == "none" || manifest_slugs.contains(slug) || FRAMEWORK_COMMAND_IDS.contains(&slug)
}

#[test]
fn router_rs_main_binary_compiles() {
    let mut command = Command::new("cargo");
    command
        .args([
            "check",
            "--manifest-path",
            "core/router-rs/Cargo.toml",
            "--bin",
            "router-rs",
        ])
        .current_dir(project_root());
    assert_success(&run(command));
}

#[test]
fn repo_local_plugin_wrapper_stays_removed() {
    assert!(!project_root()
        .join("plugins/skill-framework-native")
        .exists());
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
    for marker in [
        "仅当用户明确要求 review plan / 审计划",
        "只找问题，不改代码",
    ] {
        assert!(plan_mode.contains(marker), "missing marker: {marker}");
    }

    let review_gate = read_text(&project_root().join(".cursor/rules/review-subagent-gate.mdc"));
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
        "docs/references/EXECUTION_LADDER.md",
        "面向用户的回复必须使用简体中文",
        "Continuity artifacts",
        "Closeout",
        "Skill Routing",
        "/discussx",
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
fn refresh_skill_stays_out_of_project_host_entrypoints() {
    assert!(!project_root().join("skills/refresh/SKILL.md").exists());
    assert!(!project_root().join(".codex/skills/refresh").exists());
    assert!(!project_root()
        .join("artifacts/codex-skill-surface/skills/refresh")
        .exists());
    let registry = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    assert!(registry["framework_commands"]["refresh"].is_null());
}

#[test]
fn rfv_harness_reference_moved_to_docs() {
    assert!(!project_root()
        .join("skills/review-fix-verify-loop/SKILL.md")
        .exists());
    assert!(project_root().join("docs/rfv_loop_harness.md").exists());
    assert!(project_root()
        .join("docs/references/rfv-loop/reasoning-depth-contract.md")
        .exists());
    assert!(project_root()
        .join("docs/references/rfv-loop/external-research-harness.md")
        .exists());
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
fn project_host_skill_projection_is_generated_outside_host_entrypoints() {
    assert!(!project_root().join(".codex/skills").exists());
    assert!(!project_root().join("AGENT.md").exists());
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    seed_framework_markers(&repo_root);
    let sync_report = router_rs_json(&[
        "framework",
        "sync-entrypoints",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    let manifest = read_json(&repo_root.join(".codex/host_entrypoints_sync_manifest.json"));
    assert!(
        sync_report["written"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "expected codex sync to write host entrypoints: {sync_report}"
    );
    let manifest_text = manifest.to_string();
    assert!(!manifest_text.contains(".codex/skills/gitx"));
    assert!(!manifest_text.contains(".codex/skills/autopilot"));
    assert!(!manifest_text.contains(".codex/prompts/"));
    assert!(!repo_root.join(".codex/prompts/autopilot.md").exists());
    assert!(!repo_root.join(".codex/prompts/gitx.md").exists());
    assert_eq!(
        manifest["shared_system"]["supported_hosts"],
        serde_json::json!([
            "codex-cli",
            "codex-app",
            "cursor",
            "claude-code",
            "claude-desktop",
            "antigravity-cli",
            "antigravity-app",
            "antigravity",
            "opencode"
        ])
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["codex-cli"],
        "AGENTS_CODEX.md"
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["cursor"],
        serde_json::json!(["AGENTS_CURSOR.md", ".cursor/rules/*.mdc"])
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["codex-app"],
        "AGENTS_CODEX.md"
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["claude-code"],
        serde_json::json!([
            "AGENTS_CLAUDE.md",
            ".claude/rules/framework.md",
            ".claude/settings.json"
        ])
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["claude-desktop"],
        serde_json::json!(["AGENTS_CLAUDE.md", ".claude/CLAUDE.md"])
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["antigravity-cli"],
        "AGENTS_ANTIGRAVITY.md"
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["antigravity-app"],
        serde_json::json!([
            "AGENTS_ANTIGRAVITY.md",
            ".gemini/antigravity/rules/framework.md"
        ])
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["antigravity"],
        serde_json::json!([
            "AGENTS_ANTIGRAVITY.md",
            ".gemini/antigravity/rules/framework.md"
        ])
    );
    assert_eq!(
        manifest["shared_system"]["policy"],
        "host-specific-agent-policy-v1"
    );
    assert_eq!(
        manifest["shared_system"]["routing_source_of_truth"],
        "skills/"
    );
    assert_eq!(
        manifest["shared_system"]["agent_policy_entrypoint"],
        "AGENTS_CODEX.md"
    );
    let codex_policy = read_text(&repo_root.join("AGENTS_CODEX.md"));
    assert!(codex_policy.contains("Codex Agent Policy"));
    assert!(codex_policy.contains("AGENTS.md"));
    assert!(manifest["full_sync"]["text_files"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("AGENTS_CODEX.md")));
    assert!(manifest["full_sync"]["text_files"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!(".codex/README.md")));
    assert!(manifest["full_sync"]["json_files"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!(".codex/hooks.json")));
    assert!(manifest["partial_sync"]["json_files"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!(
            ".codex/host_entrypoints_sync_manifest.json"
        )));
    assert_eq!(
        manifest["partial_sync"]["text_files"],
        serde_json::json!([])
    );
    assert!(!manifest_text.contains("retired_files"));
    assert!(!manifest_text.contains("retired_directories"));
    assert!(!manifest_text.contains("AGENT.md"));
}

#[test]
fn codex_sync_does_not_write_root_agents_md() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    seed_framework_markers(&repo_root);
    let policy = "custom kernel policy from disk\n";
    std::fs::write(repo_root.join("AGENTS.md"), policy).unwrap();

    let sync_report = router_rs_json(&[
        "framework",
        "sync-entrypoints",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    assert!(
        !sync_report["written"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("AGENTS.md")),
        "codex sync must not write repo-root AGENTS.md: {sync_report}"
    );
    assert_eq!(read_text(&repo_root.join("AGENTS.md")), policy);
}

#[test]
fn codex_sync_preserves_existing_agents_codex_delta_file() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    seed_framework_markers(&repo_root);
    let delta = "custom codex delta from disk\nReview findings-only\n";
    std::fs::write(repo_root.join("AGENTS_CODEX.md"), delta).unwrap();

    let sync_report = router_rs_json(&[
        "framework",
        "sync-entrypoints",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    let written = sync_report["written"].as_array().unwrap();
    assert!(
        !written.contains(&serde_json::json!("AGENTS_CODEX.md")),
        "sync must not rewrite unchanged AGENTS_CODEX.md: {sync_report}"
    );
    assert_eq!(read_text(&repo_root.join("AGENTS_CODEX.md")), delta);
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
    assert!(skills.iter().any(|item| item.as_str() == Some("discussx")));
    assert!(skills.iter().any(|item| item.as_str() == Some("implementx")));
    assert!(!skills.iter().any(|item| item.as_str() == Some("gsd")));
    assert!(skills.iter().any(|item| item.as_str() == Some("gitx")));
    assert!(skills
        .iter()
        .any(|item| item.as_str() == Some("deepinterview")));
    assert!(!skills.iter().any(|item| item.as_str() == Some("team")));
    assert!(!skills.iter().any(|item| item.as_str() == Some("refresh")));
    assert!(!skills.iter().any(|item| item.as_str() == Some("autopilot")));
    assert!(surface_root.join("discussx/SKILL.md").exists());
    assert!(surface_root.join("implementx/SKILL.md").exists());
    assert!(!surface_root.join("gsd").exists());
    assert!(surface_root.join("gitx/SKILL.md").exists());
    assert!(surface_root.join("deepinterview/SKILL.md").exists());
    assert!(!surface_root.join("team/SKILL.md").exists());
    let my_impl = read_text(&surface_root.join("implementx/SKILL.md"));
    assert!(my_impl.contains("/implementx"));
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
        &project_root().join("core/router-rs/Cargo.toml"),
        &["--help"],
    ));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "route",
        "search",
        "framework",
        "codex",
        "trace",
        "storage",
        "browser",
        "profile",
        "migrate",
    ] {
        assert!(stdout.contains(command), "missing command: {command}");
    }
    for removed_flag in [
        "route-json",
        "framework-runtime-snapshot-json",
        "host-integration",
        "browser-mcp-stdio",
        "profile-json",
    ] {
        assert!(
            !stdout.contains(removed_flag),
            "removed flag leaked: {removed_flag}"
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
fn generated_routing_surfaces_do_not_reference_removed_python_helpers() {
    let generated = [
        "skills/SKILL_MANIFEST.json",
        "skills/SKILL_ROUTING_RUNTIME.json",
        "skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json",
        "skills/SKILL_PLUGIN_CATALOG.json",
        "skills/SKILL_ROUTING_METADATA.json",
        "skills/SKILL_HEALTH_MANIFEST.json",
        "skills/SKILL_ROUTING_INDEX.md",
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
fn framework_naming_conventions_has_no_router_rs_default_value_table() {
    let text = read_text(&project_root().join("docs/framework_naming_conventions.md"));
    assert!(
        !text.contains("Known Env Vars"),
        "framework_naming_conventions must not host a second ROUTER_RS defaults table"
    );
    for forbidden_default in [
        "ROUTER_RS_DEPTH_SCORE_MODE` | off",
        "ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` | false",
    ] {
        assert!(
            !text.contains(forbidden_default),
            "framework_naming_conventions leaked env default row: {forbidden_default}"
        );
    }
    assert!(
        text.contains("harness_architecture.md"),
        "framework_naming_conventions must link harness §5 for env defaults"
    );
}

#[test]
fn removed_router_flags_are_absent_from_user_docs() {
    let docs = ["RTK.md", "docs/rust_contracts.md"]
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
    assert!(docs.contains("router-rs framework snapshot"));
    assert!(docs.contains("codex sync --repo-root"));
    assert!(docs.contains("stdio `execute` operation"));
}

#[test]
fn framework_surface_policy_is_the_activation_source_of_truth() {
    let surface =
        read_json(&project_root().join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"));
    let tiers = read_json(&project_root().join("skills/SKILL_TIERS.json"));

    assert_eq!(surface["source_of_truth"], true);
    assert_eq!(
        surface["derived_reports"],
        serde_json::json!(["skills/SKILL_TIERS.json"])
    );
    assert_eq!(
        surface["deprecated_or_foldable_reports"],
        serde_json::json!([])
    );
    assert_eq!(
        surface["kernel"]["canonical_axes"],
        serde_json::json!(["routing", "memory", "continuity", "host_projection"])
    );
    assert_eq!(tiers["source_of_truth"], false);
    assert_eq!(
        tiers["derived_from"],
        "configs/framework/FRAMEWORK_SURFACE_POLICY.json"
    );
    assert_eq!(tiers["report_status"], "generated_debug_report");
    assert_eq!(
        surface["skill_system"]["activation_counts"],
        tiers["summary"]["activation_counts"]
    );
}

#[test]
fn runtime_hot_index_is_minimal() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let runtime_obj = runtime.as_object().expect("runtime object");
    let keys = runtime_obj.keys().cloned().collect::<HashSet<_>>();
    assert_eq!(
        keys,
        HashSet::from([
            "version".to_string(),
            "schema_version".to_string(),
            "scope".to_string(),
            "keys".to_string(),
            "skills".to_string(),
        ])
    );
    assert!(runtime.get("checklist").is_none());
    assert!(runtime.get("records").is_none());
    assert!(runtime.get("plugin_abi_version").is_none());
    assert!(runtime.get("vnext").is_none());
}

#[test]
fn runtime_hot_index_keeps_capability_gates_explicit() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let keys = runtime["keys"].as_array().expect("runtime keys");
    let slug_idx = key_index(keys, "slug");
    assert_eq!(runtime["version"], 3);
    assert!(
        !keys.iter().any(|key| key == "health"),
        "runtime schema v3 must not expose the retired health column"
    );
    let slugs = runtime["skills"]
        .as_array()
        .expect("runtime skills")
        .iter()
        .map(|skill| skill[slug_idx].as_str().expect("runtime skill slug"))
        .collect::<Vec<_>>();

    assert_eq!(runtime["scope"]["kind"], "hot");
    assert_eq!(
        runtime["scope"]["fallback_manifest"],
        "skills/SKILL_MANIFEST.json"
    );
    for expected in [
        "gh-address-comments",
        "gh-fix-ci",
        "citation-management",
        "paper-workbench",
        "paper-writing",
        "plan-mode",
        "code-review-deep",
        "statistical-analysis",
        "experiment-reproducibility",
        "math-derivation",
        "scientific-figure-plotting",
        "openai-docs",
        "pdf",
        "skill-framework-developer",
        "visual-review",
    ] {
        assert!(
            slugs.contains(&expected),
            "missing hot runtime slug: {expected}"
        );
    }
    for excluded in [
        "systematic-debugging",
        "idea-to-plan",
        "plan-to-code",
        "plugin-creator",
        "skill-creator",
        "skill-installer",
    ] {
        assert!(
            !slugs.contains(&excluded),
            "broad first-turn owner should stay out of hot runtime: {excluded}"
        );
    }
    assert!(
        slugs.len() <= 44,
        "hot runtime surface should stay bounded; got {}",
        slugs.len()
    );
    assert_eq!(runtime["scope"]["hot_skill_count"], slugs.len());
}

#[test]
fn runtime_hot_index_stays_separate_from_plugin_and_routing_catalogs() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let plugin_catalog = read_json(&project_root().join("skills/SKILL_PLUGIN_CATALOG.json"));
    let routing_metadata = read_json(&project_root().join("skills/SKILL_ROUTING_METADATA.json"));
    assert_eq!(runtime["version"], 3);
    assert_eq!(runtime["schema_version"], "skill-routing-runtime-v3");
    let rows = runtime["skills"].as_array().expect("runtime rows");
    let framework_row = rows
        .iter()
        .find(|record| record[0] == "skill-framework-developer")
        .expect("skill-framework-developer runtime row");
    assert_eq!(framework_row[0], "skill-framework-developer");
    assert_eq!(
        plugin_catalog["skills"]["skill-framework-developer"]["kind"],
        "skill"
    );
    assert_eq!(
        routing_metadata["skills"]["skill-framework-developer"]["selection_reason"],
        "allowlisted first-turn owner"
    );
}

fn parse_skill_md_frontmatter_map(path: &Path) -> Map<String, Value> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let rest = text
        .strip_prefix("---")
        .unwrap_or_else(|| panic!("{}: missing opening ---", path.display()));
    let rest = rest.trim_start_matches(['\n', '\r']);
    let end = rest
        .find("\n---\n")
        .or_else(|| rest.find("\r\n---\r\n"))
        .or_else(|| rest.find("\n---\r\n"))
        .unwrap_or_else(|| panic!("{}: missing closing ---", path.display()));
    let yaml_txt = &rest[..end];
    let yaml_val: serde_yml::Value =
        serde_yml::from_str(yaml_txt).unwrap_or_else(|e| panic!("{}: yaml: {e}", path.display()));
    serde_json::to_value(yaml_val)
        .expect("yaml to json")
        .as_object()
        .expect("frontmatter must be a mapping")
        .clone()
}

fn value_string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        Some(other) => other
            .as_str()
            .map(|s| vec![s.to_string()])
            .unwrap_or_default(),
    }
}

fn raw_platforms_from_skill_frontmatter(meta: &Map<String, Value>) -> Vec<String> {
    let mut raw = value_string_list(meta.get("platforms"));
    if raw.is_empty() {
        if let Some(Value::Object(inner)) = meta.get("metadata") {
            raw = value_string_list(inner.get("platforms"));
        }
    }
    raw
}

#[test]
fn runtime_host_support_platforms_are_registry_closed_and_match_skill_md() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let allowed: HashSet<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();
    let plugin_catalog = read_json(&root.join("skills/SKILL_PLUGIN_CATALOG.json"));
    for (slug, record) in plugin_catalog["skills"]
        .as_object()
        .expect("plugin catalog skills")
    {
        let platforms = record["host_support"]["platforms"]
            .as_array()
            .expect("host_support.platforms");
        for p in platforms {
            let id = p.as_str().expect("platform string");
            assert!(
                allowed.contains(id),
                "{slug}: platform `{id}` not in RUNTIME_REGISTRY.host_targets.supported"
            );
        }
        let kind = record["kind"].as_str().expect("plugin.kind");
        if kind != "skill" {
            continue;
        }
        let skill_path = root.join(record["skill_path"].as_str().expect("skill_path"));
        let meta = parse_skill_md_frontmatter_map(&skill_path);
        let raw = raw_platforms_from_skill_frontmatter(&meta);
        let mut supported_ids: Vec<String> = allowed.iter().cloned().collect();
        supported_ids.sort();
        let normalized =
            host_platforms::normalize_skill_host_platforms(&raw, &supported_ids)
                .unwrap_or_else(|e| panic!("{slug}: normalize_skill_host_platforms: {e}"));
        let from_catalog: Vec<String> = platforms
            .iter()
            .map(|v| v.as_str().expect("platform").to_string())
            .collect();
        assert_eq!(
            normalized,
            from_catalog,
            "host_support.platforms drift for slug={slug} path={}",
            skill_path.display()
        );
    }
}

#[test]
fn skill_host_platform_aliases_cover_runtime_registry_supported_hosts() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let allowed: HashSet<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();

    let mut supported: Vec<String> = allowed.iter().cloned().collect();
    supported.sort();

    let normalized = host_platforms::normalize_skill_host_platforms(
        &[
            "codex".to_string(),
            "cursor".to_string(),
            "claude".to_string(),
            "claude-desktop".to_string(),
            "antigravity-cli".to_string(),
            "antigravity-app".to_string(),
            "antigravity".to_string(),
            "opencode".to_string(),
        ],
        &supported,
    )
    .expect("stock aliases should normalize");
    let normalized_set: HashSet<String> = normalized.into_iter().collect();

    assert_eq!(
        normalized_set, allowed,
        "host_platforms alias coverage must stay aligned with RUNTIME_REGISTRY.host_targets.supported"
    );
}

/// Host-agnostic hot-route skills must list every closed-set host id; Codex-installer-only skills are exempt.
const HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS: &[&str] =
    &["plugin-creator", "skill-installer", "openai-docs", "tao-ci"];

#[test]
fn hot_runtime_skill_records_cover_all_supported_hosts() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let supported: Vec<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();
    let runtime = read_json(&root.join("skills/SKILL_ROUTING_RUNTIME.json"));
    let skills = runtime["skills"].as_array().expect("runtime skills");
    for row in skills.iter().filter_map(Value::as_array) {
        let slug = row.first().and_then(|v| v.as_str()).expect("slug");
        if HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS.contains(&slug) {
            continue;
        }
        let platforms = row
            .get(9)
            .and_then(|v| v.as_array())
            .expect("host_platforms");
        let set: HashSet<String> = platforms
            .iter()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect();
        for host in &supported {
            assert!(
                set.contains(host),
                "hot runtime skill `{slug}` must include host_platform `{host}` (set `metadata.platforms: [supported]` or list all ids); exempt slugs: {:?}",
                HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS
            );
        }
    }
}

#[test]
fn hot_runtime_codex_only_slugs_have_no_extra_hosts() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let allowed: HashSet<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();
    let mut supported_ids: Vec<String> = allowed.iter().cloned().collect();
    supported_ids.sort();

    let runtime = read_json(&root.join("skills/SKILL_ROUTING_RUNTIME.json"));
    let skills = runtime["skills"].as_array().expect("runtime skills");
    for row in skills.iter().filter_map(Value::as_array) {
        let slug = row.first().and_then(|v| v.as_str()).expect("slug");
        if !HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS.contains(&slug) {
            continue;
        }
        let skill_path = row
            .get(8)
            .and_then(|v| v.as_str())
            .map(|rel| root.join(rel))
            .expect("skill_path");
        let meta = parse_skill_md_frontmatter_map(&skill_path);
        let raw = raw_platforms_from_skill_frontmatter(&meta);
        let allowed_platforms = host_platforms::normalize_skill_host_platforms(
            &raw,
            &supported_ids,
        )
        .unwrap_or_else(|e| panic!("{slug}: normalize_skill_host_platforms: {e}"));
        let allowed_set: HashSet<String> = allowed_platforms.into_iter().collect();

        let runtime_platforms = row
            .get(9)
            .and_then(|v| v.as_array())
            .expect("host_platforms");
        for platform in runtime_platforms {
            let id = platform.as_str().expect("host platform");
            assert!(
                allowed_set.contains(id),
                "codex-only hot runtime skill `{slug}` must not list extra host `{id}` in runtime host_platforms; allowed={allowed_set:?}"
            );
        }
    }
}

#[test]
fn framework_command_slugs_in_manifest() {
    let manifest = read_json(&project_root().join("skills/SKILL_MANIFEST.json"));
    let keys = manifest["keys"].as_array().expect("manifest keys");
    let slug_idx = key_index(keys, "slug");
    let manifest_slugs: HashSet<String> = manifest["skills"]
        .as_array()
        .expect("manifest skills")
        .iter()
        .filter_map(|row| row.get(slug_idx).and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    for slug in FRAMEWORK_COMMAND_IDS {
        assert!(
            manifest_slugs.contains(*slug),
            "SKILL_MANIFEST must contain framework command `{slug}` (not runtime-only)"
        );
    }
}

#[test]
fn runtime_framework_command_rows_match_manifest() {
    let root = project_root();
    let runtime = read_json(&root.join("skills/SKILL_ROUTING_RUNTIME.json"));
    let manifest = read_json(&root.join("skills/SKILL_MANIFEST.json"));
    let runtime_keys = runtime["keys"].as_array().expect("runtime keys");
    let manifest_keys = manifest["keys"].as_array().expect("manifest keys");
    let r_slug = key_index(runtime_keys, "slug");
    let r_layer = key_index(runtime_keys, "layer");
    let r_kind = key_index(runtime_keys, "kind");
    let r_summary = key_index(runtime_keys, "summary");
    let r_hosts = key_index(runtime_keys, "host_platforms");
    let r_skill_path = key_index(runtime_keys, "skill_path");
    let r_trigger_hints = key_index(runtime_keys, "trigger_hints");
    let m_slug = key_index(manifest_keys, "slug");
    let m_layer = key_index(manifest_keys, "layer");
    let m_kind = key_index(manifest_keys, "kind");
    let m_desc = key_index(manifest_keys, "description");
    let m_hosts = key_index(manifest_keys, "host_platforms");
    let m_skill_path = key_index(manifest_keys, "skill_path");
    let m_trigger_hints = key_index(manifest_keys, "trigger_hints");

    let manifest_by_slug: HashMap<String, &Vec<Value>> = manifest["skills"]
        .as_array()
        .expect("manifest skills")
        .iter()
        .filter_map(|row| row.as_array())
        .filter_map(|row| {
            let slug = row.get(m_slug)?.as_str()?.to_string();
            Some((slug, row))
        })
        .collect();

    for row in runtime["skills"].as_array().expect("runtime skills") {
        let row = row.as_array().expect("runtime row");
        let slug = row[r_slug].as_str().expect("runtime slug");
        if !FRAMEWORK_COMMAND_IDS.contains(&slug) {
            continue;
        }
        let manifest_row = manifest_by_slug
            .get(slug)
            .unwrap_or_else(|| panic!("manifest missing framework command row for {slug}"));
        assert_eq!(
            row[r_layer].as_str(),
            manifest_row.get(m_layer).and_then(Value::as_str),
            "{slug}: layer mismatch runtime vs manifest"
        );
        assert_eq!(
            row[r_kind].as_str(),
            manifest_row.get(m_kind).and_then(Value::as_str),
            "{slug}: kind mismatch runtime vs manifest"
        );
        assert_eq!(
            row[r_summary].as_str(),
            manifest_row.get(m_desc).and_then(Value::as_str),
            "{slug}: description/summary mismatch runtime vs manifest"
        );
        let runtime_hosts: HashSet<String> = row[r_hosts]
            .as_array()
            .expect("runtime host_platforms")
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let manifest_hosts: HashSet<String> = manifest_row
            .get(m_hosts)
            .and_then(Value::as_array)
            .expect("manifest host_platforms")
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        assert_eq!(
            runtime_hosts, manifest_hosts,
            "{slug}: host_platforms mismatch runtime vs manifest"
        );
        assert_eq!(
            row[r_skill_path].as_str(),
            manifest_row.get(m_skill_path).and_then(Value::as_str),
            "{slug}: skill_path mismatch runtime vs manifest"
        );
        let runtime_hints: Vec<String> = row[r_trigger_hints]
            .as_array()
            .expect("runtime trigger_hints")
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let manifest_hints: Vec<String> = manifest_row
            .get(m_trigger_hints)
            .and_then(Value::as_array)
            .expect("manifest trigger_hints")
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        assert_eq!(
            runtime_hints, manifest_hints,
            "{slug}: trigger_hints mismatch runtime vs manifest"
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
        default.contains("/discussx"),
        "default_lifecycle_paragraph must reference /discussx"
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
        if meta.get("deprecated_alias_of").and_then(Value::as_str).is_some() {
            continue;
        }
        let paragraph = by_host
            .get(host_id)
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                panic!("lifecycle_by_host missing installable host {host_id}")
            });
        assert!(
            paragraph.contains("/discussx") || paragraph.contains("Default lifecycle"),
            "{host_id}: lifecycle paragraph must reference My lifecycle (/discussx or Default lifecycle)"
        );
    }
}

#[test]
fn plugin_catalog_routing_metadata_and_health_manifest_form_closed_loop() {
    let plugin_catalog = read_json(&project_root().join("skills/SKILL_PLUGIN_CATALOG.json"));
    let routing_metadata = read_json(&project_root().join("skills/SKILL_ROUTING_METADATA.json"));
    let explain = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json"));
    let health = read_json(&project_root().join("skills/SKILL_HEALTH_MANIFEST.json"));

    assert_eq!(plugin_catalog["schema_version"], "skill-plugin-catalog-v1");
    assert_eq!(plugin_catalog["source_of_truth"], false);
    assert_eq!(plugin_catalog["derived_from"], "skills/SKILL_MANIFEST.json");
    assert_eq!(
        routing_metadata["schema_version"],
        "skill-routing-metadata-v1"
    );
    assert_eq!(routing_metadata["source_of_truth"], false);
    assert_eq!(
        explain["schema_version"],
        "skill-routing-runtime-explain-v1"
    );
    assert_eq!(explain["source_of_truth"], false);
    assert_eq!(health["schema_version"], "skill-health-manifest-v1");
    assert_eq!(health["source_of_truth"], false);
    assert!(health["skills"].as_object().is_some());

    let catalog_skills = plugin_catalog["skills"]
        .as_object()
        .expect("plugin catalog skills");
    let metadata_skills = routing_metadata["skills"]
        .as_object()
        .expect("routing metadata skills");
    assert!(!catalog_skills.is_empty());
    for (slug, record) in catalog_skills {
        assert!(
            metadata_skills.contains_key(slug),
            "routing metadata missing slug {slug}"
        );
        assert_eq!(record["kind"], "skill");
        assert!(record["skill_path"].as_str().is_some());
        assert!(record["host_support"]["platforms"].as_array().is_some());
    }

    let skill = "skill-framework-developer";
    assert!(catalog_skills.contains_key(skill));
    assert!(metadata_skills.contains_key(skill));
    if explain["selected"][skill].is_object() {
        assert_eq!(
            explain["selected"][skill]["plugin_kind"],
            catalog_skills[skill]["kind"]
        );
    }
}

#[test]
fn plugin_catalog_routing_metadata_companion_schemas_contract() {
    let plugin_catalog = read_json(&project_root().join("skills/SKILL_PLUGIN_CATALOG.json"));
    let routing_metadata = read_json(&project_root().join("skills/SKILL_ROUTING_METADATA.json"));
    let explain = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json"));
    let health = read_json(&project_root().join("skills/SKILL_HEALTH_MANIFEST.json"));

    assert_eq!(plugin_catalog["schema_version"], "skill-plugin-catalog-v1");
    assert!(
        plugin_catalog["skills"].is_object(),
        "companion plugin catalog must list skills"
    );
    assert_eq!(
        routing_metadata["schema_version"],
        "skill-routing-metadata-v1"
    );
    assert_eq!(
        explain["schema_version"],
        "skill-routing-runtime-explain-v1"
    );
    assert_eq!(health["schema_version"], "skill-health-manifest-v1");
    assert!(
        routing_metadata["skills"].is_object(),
        "routing metadata companion must list skills"
    );
    assert_eq!(
        explain.get("source_of_truth").and_then(|v| v.as_bool()),
        Some(false),
        "RUNTIME_EXPLAIN is a refresh stub, not router hot-path truth"
    );
}

#[test]
fn gsd_slash_commands_removed_from_runtime_and_hooks() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let registry_text = read_text(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    assert!(
        registry.get("framework_commands").and_then(|v| v.get("gsd")).is_none(),
        "framework_commands.gsd must stay removed"
    );
    assert!(
        !registry_text.contains("/gsd-"),
        "RUNTIME_REGISTRY must not reference /gsd- commands"
    );
    let hook_common = read_text(&root.join("core/router-rs/src/hook_common.rs"));
    assert!(
        !hook_common.contains("/gsd-"),
        "hook_common must not recognize /gsd- entrypoints"
    );
}

#[test]
fn runtime_provider_registry_declares_component_plugin_lanes() {
    let registry =
        read_json(&project_root().join("configs/framework/RUNTIME_PROVIDER_REGISTRY.json"));
    let runtime = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    assert_eq!(registry["schema_version"], "runtime-provider-registry-v1");
    assert_eq!(registry["plugin_abi_version"], "skill-plugin-abi-v1");
    for lane in [
        "execution_providers",
        "storage_providers",
        "trace_replay_providers",
        "observability_providers",
        "sandbox_profile_providers",
        "host_projection_providers",
        "governance_eval_loop",
    ] {
        assert!(
            registry.get(lane).is_some(),
            "missing provider registry lane: {lane}"
        );
    }
    assert_eq!(
        registry["execution_providers"]["local_rust"]["status"],
        "implemented"
    );
    assert_eq!(
        registry["storage_providers"]["sqlite"]["status"],
        "implemented"
    );
    assert_eq!(
        registry["trace_replay_providers"]["human_intervention"]["status"],
        "declared"
    );
    assert_eq!(
        registry["host_projection_providers"]["codex-cli"]["status"],
        "implemented"
    );
    assert_eq!(
        registry["host_projection_providers"]["mcp"]["status"],
        "declared"
    );
    let supported_hosts = runtime["host_targets"]["supported"]
        .as_array()
        .expect("runtime supported hosts")
        .iter()
        .map(|host| host.as_str().expect("host id").to_string())
        .collect::<BTreeSet<_>>();
    let projected_hosts = runtime["host_projections"]
        .as_object()
        .expect("runtime host_projections")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let provider_hosts = registry["host_projection_providers"]
        .as_object()
        .expect("provider host_projection_providers")
        .keys()
        .filter(|host| supported_hosts.contains(*host))
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        projected_hosts, supported_hosts,
        "RUNTIME_REGISTRY host_targets.supported and host_projections must match"
    );
    assert_eq!(
        provider_hosts, supported_hosts,
        "RUNTIME_PROVIDER_REGISTRY must cover every supported host projection"
    );
    assert_eq!(
        registry["governance_eval_loop"]["metrics"][0],
        "route_expected_owner_accuracy"
    );
    assert!(
        !registry.to_string().contains("/Users/joe"),
        "provider registry must stay portable"
    );
}

#[test]
fn routing_signal_markers_json_unique_nonempty_lists() {
    let v = read_json(&project_root().join("configs/framework/ROUTING_SIGNAL_MARKERS.json"));
    assert_eq!(
        v.get("schema_version").and_then(Value::as_str),
        Some("routing-signal-markers-v1")
    );
    fn assert_no_dupes(arr: &Value, ctx: &str) {
        let a = arr
            .as_array()
            .unwrap_or_else(|| panic!("{ctx} must be array"));
        let mut seen = HashSet::new();
        for item in a {
            let s = item
                .as_str()
                .unwrap_or_else(|| panic!("{ctx} must be string list"));
            assert!(!s.is_empty(), "{ctx} empty string");
            assert!(
                seen.insert(s.to_string()),
                "{ctx} duplicate substring `{s}`"
            );
        }
    }
    let m = v.get("meta_routing_task").expect("meta_routing_task");
    assert_no_dupes(
        m.get("anchor_any_of_substrings")
            .expect("anchor_any_of_substrings"),
        "meta_routing_task.anchor_any_of_substrings",
    );
    assert_no_dupes(
        m.get("marker_any_of_substrings")
            .expect("marker_any_of_substrings"),
        "meta_routing_task.marker_any_of_substrings",
    );
    assert_no_dupes(
        &v["completion_execution_markers"],
        "completion_execution_markers",
    );
    assert_no_dupes(
        &v["supervisor_execution_markers"],
        "supervisor_execution_markers",
    );
}

#[test]
fn hook_observation_rules_json_schema_version() {
    let v =
        read_json(&project_root().join("configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json"));
    assert_eq!(
        v.get("schema_version").and_then(Value::as_str),
        Some("router-rs-hook-observation-rules-v1")
    );
}

/// Loads `NL_SIGNAL_REGISTRY` names from the built `router-rs` binary (no regex scan of Rust source).
fn nl_route_registry_signal_names() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let repo = project_root();
        let manifest = repo.join("core/router-rs/Cargo.toml");
        let output = Command::new("cargo")
            .current_dir(&repo)
            .args([
                "run",
                "-q",
                "--manifest-path",
                manifest.to_str().expect("manifest path utf-8"),
                "--",
                "framework",
                "nl-route-signal-registry-contract",
            ])
            .output()
            .unwrap_or_else(|e| {
                panic!("cargo run router-rs framework nl-route-signal-registry-contract: {e}");
            });
        assert!(
            output.status.success(),
            "nl-route-signal-registry-contract failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let raw = String::from_utf8_lossy(&output.stdout);
        let arr: Vec<String> = serde_json::from_str(raw.trim())
            .expect("nl-route-signal-registry-contract stdout must be a JSON string array");
        assert!(!arr.is_empty(), "NL_SIGNAL_REGISTRY dump must be non-empty");
        arr.into_iter().collect()
    })
}

fn nl_policy_signal_allowed(name: &str) -> bool {
    nl_route_registry_signal_names().contains(name)
}

fn nl_policy_collect_signals_from_when(when: &Value, out: &mut HashSet<String>, ctx: &str) {
    match when {
        Value::Bool(_) => {}
        Value::Object(map) => {
            if map.is_empty() {
                panic!("{ctx}: when must not be an empty object");
            }
            for k in map.keys() {
                assert!(
                    matches!(
                        k.as_str(),
                        "all" | "any" | "not" | "signal" | "query_contains" | "first_turn"
                    ),
                    "{ctx}: when has unknown key `{k}`"
                );
            }
            if let Some(arr) = map.get("all").and_then(Value::as_array) {
                assert_eq!(map.len(), 1, "{ctx}: when.all must be the sole object key");
                for (i, sub) in arr.iter().enumerate() {
                    nl_policy_collect_signals_from_when(sub, out, &format!("{ctx}.all[{i}]"));
                }
                return;
            }
            if let Some(arr) = map.get("any").and_then(Value::as_array) {
                assert_eq!(map.len(), 1, "{ctx}: when.any must be the sole object key");
                for (i, sub) in arr.iter().enumerate() {
                    nl_policy_collect_signals_from_when(sub, out, &format!("{ctx}.any[{i}]"));
                }
                return;
            }
            if map.contains_key("not") {
                assert_eq!(map.len(), 1, "{ctx}: when.not must be the sole object key");
                let inner = map.get("not").expect("not present");
                nl_policy_collect_signals_from_when(inner, out, &format!("{ctx}.not"));
                return;
            }
            assert_eq!(
                map.len(),
                1,
                "{ctx}: when leaf must have exactly one key among signal/query_contains/first_turn"
            );
            if let Some(s) = map.get("signal").and_then(Value::as_str) {
                assert!(
                    nl_policy_signal_allowed(s),
                    "{ctx}: signal `{s}` not in nl_route_adjustments NL_SIGNAL_REGISTRY"
                );
                out.insert(s.to_string());
                return;
            }
            assert!(
                map.get("query_contains").and_then(Value::as_str).is_some()
                    || map.get("first_turn").and_then(Value::as_bool).is_some(),
                "{ctx}: when leaf must be query_contains or first_turn"
            );
        }
        other => panic!("{ctx}: when must be bool or object, got {other:?}"),
    }
}

fn nl_policy_validate_rule(rule: &Value, ctx: &str) {
    let obj = rule
        .as_object()
        .unwrap_or_else(|| panic!("{ctx}: rule must be object"));
    for k in obj.keys() {
        assert!(
            matches!(k.as_str(), "record" | "when" | "action"),
            "{ctx}: unknown rule key `{k}`"
        );
    }
    let action = obj
        .get("action")
        .unwrap_or_else(|| panic!("{ctx}: missing action"));
    let aobj = action
        .as_object()
        .unwrap_or_else(|| panic!("{ctx}: action must be object"));
    for k in aobj.keys() {
        assert!(
            matches!(k.as_str(), "type" | "reason" | "delta"),
            "{ctx}: unknown action key `{k}`"
        );
    }
    let ty = aobj
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{ctx}: action.type required"));
    match ty {
        "suppress" | "boost" => {}
        other => panic!("{ctx}: unknown action.type `{other}`"),
    }
    if let Some(rec) = obj.get("record") {
        if !rec.is_null() {
            let robj = rec
                .as_object()
                .unwrap_or_else(|| panic!("{ctx}: record must be object or null"));
            for k in robj.keys() {
                assert!(
                    matches!(k.as_str(), "slug" | "slugs" | "gate_lower"),
                    "{ctx}: unknown record key `{k}`"
                );
            }
        }
    }
    let mut signals = HashSet::new();
    match obj.get("when") {
        None => {}
        Some(w) => nl_policy_collect_signals_from_when(w, &mut signals, &format!("{ctx}.when")),
    }
    let _ = signals;
}

fn nl_policy_validate_rule_list(rules: &[Value], label: &str) {
    for (i, rule) in rules.iter().enumerate() {
        nl_policy_validate_rule(rule, &format!("{label}[{i}]"));
    }
}

#[test]
fn nl_route_adjustments_json_schema_version() {
    let v = read_json(&project_root().join("configs/framework/NL_ROUTE_ADJUSTMENTS.json"));
    let root = v
        .as_object()
        .expect("NL_ROUTE_ADJUSTMENTS root must be object");
    for k in root.keys() {
        assert!(
            matches!(
                k.as_str(),
                "schema_version"
                    | "docs"
                    | "pre_framework_alias_rules"
                    | "post_framework_alias_rules"
            ),
            "NL_ROUTE_ADJUSTMENTS: unknown root key `{k}`"
        );
    }
    assert_eq!(
        v.get("schema_version").and_then(Value::as_str),
        Some("nl-route-adjustments-v1")
    );
    let pre = v["pre_framework_alias_rules"]
        .as_array()
        .expect("pre_framework_alias_rules must be array");
    let post = v["post_framework_alias_rules"]
        .as_array()
        .expect("post_framework_alias_rules must be array");
    nl_policy_validate_rule_list(pre, "pre_framework_alias_rules");
    nl_policy_validate_rule_list(post, "post_framework_alias_rules");

    let mut used_signals = HashSet::new();
    for (label, arr) in [("pre", pre.as_slice()), ("post", post.as_slice())] {
        for (ri, rule) in arr.iter().enumerate() {
            if let Some(w) = rule.get("when") {
                nl_policy_collect_signals_from_when(
                    w,
                    &mut used_signals,
                    &format!("{label}_rules[{ri}].when"),
                );
            }
        }
    }
    let allow = nl_route_registry_signal_names();
    for s in &used_signals {
        assert!(
            allow.contains(s.as_str()),
            "used signal `{s}` must appear in nl_route_adjustments NL_SIGNAL_REGISTRY"
        );
    }
    for reg in allow.iter() {
        assert!(
            used_signals.contains(reg),
            "NL_SIGNAL_REGISTRY entry `{reg}` is never referenced in NL_ROUTE_ADJUSTMENTS.json"
        );
    }
}

#[test]
fn document_only_provider_lanes_do_not_become_installable_hosts() {
    let registry =
        read_json(&project_root().join("configs/framework/RUNTIME_PROVIDER_REGISTRY.json"));
    let runtime = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let host_metadata = runtime["host_targets"]["metadata"]
        .as_object()
        .expect("runtime host metadata");
    let host_projection_providers = registry["host_projection_providers"]
        .as_object()
        .expect("provider host projections");

    for (host_id, provider) in host_projection_providers {
        let status = provider["status"].as_str().unwrap_or_default();
        let runtime_installable = host_metadata
            .get(host_id)
            .and_then(|meta| meta.get("installable"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if status != "implemented" {
            assert!(
                !runtime_installable,
                "document-only provider `{host_id}` must not be installable in RUNTIME_REGISTRY"
            );
        }
    }

    assert_eq!(
        host_metadata["codex-app"]["installable"], false,
        "codex-app remains runtime-supported but non-installable"
    );
}

#[test]
fn manifest_and_runtime_skill_paths_are_loadable() {
    for relative in [
        "skills/SKILL_MANIFEST.json",
        "skills/SKILL_ROUTING_RUNTIME.json",
    ] {
        let payload = read_json(&project_root().join(relative));
        let keys = payload["keys"].as_array().expect("keys");
        let slug_idx = key_index(keys, "slug");
        let skill_path_idx = key_index(keys, "skill_path");
        for row in payload["skills"].as_array().expect("skills") {
            let row = row.as_array().expect("skill row");
            let slug = row[slug_idx].as_str().expect("slug");
            let skill_path = row[skill_path_idx].as_str().expect("skill_path");
            assert!(
                !skill_path.starts_with('/') && !skill_path.contains(".."),
                "{relative} has unsafe skill_path for {slug}: {skill_path}"
            );
            assert!(
                project_root().join(skill_path).is_file(),
                "{relative} missing skill_path for {slug}: {skill_path}"
            );
        }
    }
}

// --- SKILL_MANIFEST hygiene ---

#[test]
fn skill_manifest_excludes_retired_autopilot_slug() {
    let manifest = read_json(&project_root().join("skills/SKILL_MANIFEST.json"));
    let keys = manifest["keys"].as_array().expect("manifest keys");
    let slug_idx = key_index(keys, "slug");
    let slugs = manifest["skills"]
        .as_array()
        .expect("manifest skills")
        .iter()
        .map(|row| row[slug_idx].as_str().expect("manifest slug"))
        .collect::<Vec<_>>();
    assert!(
        !slugs.contains(&"autopilot"),
        "retired autopilot must not appear in SKILL_MANIFEST.json (stub remains on disk only)"
    );
}

#[test]
fn routing_eval_cases_reference_existing_manifest_skills() {
    let manifest = read_json(&project_root().join("skills/SKILL_MANIFEST.json"));
    let manifest_keys = manifest["keys"].as_array().expect("manifest keys");
    let manifest_slug_idx = key_index(manifest_keys, "slug");
    let manifest_slugs = manifest["skills"]
        .as_array()
        .expect("manifest skills")
        .iter()
        .map(|row| row[manifest_slug_idx].as_str().expect("manifest slug"))
        .collect::<std::collections::HashSet<_>>();
    let eval_cases = read_json(&project_root().join("tests/routing_eval_cases.json"));
    for case in eval_cases["cases"].as_array().expect("eval cases") {
        let id = case["id"].as_str().unwrap_or("<missing id>");
        for key in ["focus_skill", "expected_owner", "expected_overlay"] {
            if let Some(slug) = case.get(key).and_then(|value| value.as_str()) {
                assert!(
                    manifest_or_runtime_lane_contains(&manifest_slugs, slug),
                    "case {id} {key} references missing slug {slug}"
                );
            }
        }
        for slug in case
            .get("forbidden_owners")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
        {
            assert!(
                manifest_or_runtime_lane_contains(&manifest_slugs, slug),
                "case {id} forbidden_owners references missing slug {slug}"
            );
        }
    }
}

#[test]
fn framework_aliases_reference_manifest_skills() {
    let manifest = read_json(&project_root().join("skills/SKILL_MANIFEST.json"));
    let manifest_keys = manifest["keys"].as_array().expect("manifest keys");
    let manifest_slug_idx = key_index(manifest_keys, "slug");
    let manifest_slugs = manifest["skills"]
        .as_array()
        .expect("manifest skills")
        .iter()
        .map(|row| row[manifest_slug_idx].as_str().expect("manifest slug"))
        .collect::<std::collections::HashSet<_>>();

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
                manifest_or_runtime_lane_contains(&manifest_slugs, owner),
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
                manifest_or_runtime_lane_contains(&manifest_slugs, slug),
                "framework alias {alias} execution_owners references missing slug {slug}"
            );
        }
    }
}

/// `research_contract` is narrative for hosts/docs; router-rs Execute embeds deep prompt text in
/// `runtime_ops.inc` instead of parsing this JSON at runtime.
#[test]
fn my_goal_persistence_contract_documents_execution_zone() {
    let registry = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let gp = registry
        .get("framework_commands")
        .and_then(|fc| fc.get("implementx"))
        .and_then(|g| g.get("goal_persistence"))
        .expect("framework_commands.implementx.goal_persistence");
    let eps = gp
        .get("execution_entrypoints")
        .and_then(|v| v.as_array())
        .expect("execution_entrypoints array");
    assert!(eps.iter().any(|v| v.as_str() == Some("/implementx")));
    assert!(eps.iter().any(|v| v.as_str() == Some("/verifyx")));
    let leader = gp
        .get("continuation_hook_leader")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        leader.contains("framework_goal_drive") && !leader.contains("GOAL_CONTINUE"),
        "continuation_hook_leader should be stdio-only: {leader}"
    );
    assert!(registry
        .get("framework_commands")
        .and_then(|fc| fc.get("autopilot"))
        .is_none());
}

/// Legacy GSD framework_command removed; My implementx is the published execution surface.
#[test]
fn my_framework_commands_exclude_legacy_gsd() {
    let registry = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    assert!(
        registry["framework_commands"].get("gsd").is_none(),
        "framework_commands.gsd must be removed"
    );
    let my_impl = &registry["framework_commands"]["implementx"];
    assert!(
        my_impl.get("surface_publish").and_then(|v| v.as_bool()).unwrap_or(true),
        "implementx defaults to surface_publish true"
    );
}

#[test]
fn discussx_skill_forbids_pre_exec_drive_until_done_true() {
    let text = read_text(&project_root().join("skills/discussx/SKILL.md"));
    assert!(
        !text.contains("\"drive_until_done\":true"),
        "discussx must not embed drive_until_done:true in stdio example"
    );
    assert!(
        !text.contains("\"drive_until_done\": true"),
        "discussx must not embed drive_until_done: true in stdio example"
    );
    assert!(
        text.contains("drive_until_done: false") || text.contains("drive_until_done\": false"),
        "discussx must show drive_until_done:false"
    );
}

/// 防止 framework 命令的 `skill_path` 再次指回仅存在于生成投影下的路径（裸 clone 会断链）。
#[test]
fn framework_command_skill_paths_do_not_use_codex_skill_surface_aliases() {
    let root = project_root();
    let forbidden = ["artifacts/codex-skill-surface/skills/autopilot/"];
    for rel in [
        "configs/framework/RUNTIME_REGISTRY.json",
        "skills/SKILL_ROUTING_RUNTIME.json",
        "skills/SKILL_PLUGIN_CATALOG.json",
        "skills/SKILL_HEALTH_MANIFEST.json",
    ] {
        let text = read_text(&root.join(rel));
        for needle in forbidden {
            assert!(
                !text.contains(needle),
                "{rel} must not reference legacy surface path {needle:?}"
            );
        }
    }
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let my_impl = &registry["framework_commands"]["implementx"];
    assert_eq!(
        my_impl["skill_path"].as_str().expect("implementx skill_path"),
        "skills/implementx/SKILL.md"
    );
    assert!(
        registry["framework_commands"].get("gsd").is_none(),
        "framework_commands.gsd must be removed"
    );
    assert!(
        registry["framework_commands"].get("autopilot").is_none(),
        "autopilot framework_command must be removed"
    );
    assert_eq!(
        registry["framework_commands"]["team"]["canonical_owner"], "agent-swarm-orchestration",
        "team must remain a framework alias backed by agent-swarm-orchestration"
    );

    let runtime = read_json(&root.join("skills/SKILL_ROUTING_RUNTIME.json"));
    let runtime_slugs = runtime["skills"]
        .as_array()
        .expect("runtime skills")
        .iter()
        .filter_map(|row| row.get(0).and_then(|value| value.as_str()))
        .collect::<Vec<_>>();
    assert!(
        !runtime_slugs.contains(&"team"),
        "team alias must not be a hot runtime skill"
    );

    let plugin_catalog = read_json(&root.join("skills/SKILL_PLUGIN_CATALOG.json"));
    assert!(
        plugin_catalog["skills"].get("team").is_none(),
        "team alias must not be a plugin skill record"
    );
    let routing_metadata = read_json(&root.join("skills/SKILL_ROUTING_METADATA.json"));
    assert!(
        routing_metadata["skills"].get("team").is_none(),
        "team alias must not be a routing metadata owner"
    );
    let health = read_json(&root.join("skills/SKILL_HEALTH_MANIFEST.json"));
    assert!(
        health["skills"].get("runtime:team").is_none(),
        "team alias must not be a runtime health skill"
    );
    assert!(
        !root
            .join("artifacts/codex-skill-surface/skills/team/SKILL.md")
            .exists(),
        "team alias must not be a generated Codex skill surface"
    );
}

fn key_index(keys: &[serde_json::Value], name: &str) -> usize {
    keys.iter()
        .position(|key| key.as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing key {name}"))
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
fn autoresearch_runtime_controller_stays_without_legacy_skill_entrypoint() {
    assert!(project_root()
        .join("core/autoresearch-rs/src/main.rs")
        .exists());
    assert!(!project_root().join("skills/autoresearch").exists());
}

#[test]
fn installed_project_hooks_are_router_rs_managed() {
    assert!(project_root().join(".codex/hooks.json").exists());
    assert!(!project_root().join(".codex/hooks").exists());
    let config = read_text(&project_root().join(".codex/config.toml"));
    assert!(config.contains("hooks = true"));
    assert!(!config.contains("codex_hooks"));
    let hooks = read_json(&project_root().join(".codex/hooks.json"));
    let hook_events = hooks["hooks"].as_object().unwrap();
    for event in [
        "SessionStart",
        "PreToolUse",
        "UserPromptSubmit",
        "PostToolUse",
        "Stop",
    ] {
        assert!(
            hook_events.contains_key(event),
            "missing Codex hook event: {event}"
        );
    }
    let hook_text = hooks.to_string();
    assert!(hook_text.contains("router-rs"));
    assert!(hook_text.contains("codex-router-rs-hook.sh"));
    assert!(!hook_text.contains("scripts/codex_hook_entrypoint.sh"));
    assert!(!hook_text.contains("sessionEnd"));
    let manifest = read_json(&project_root().join(".codex/host_entrypoints_sync_manifest.json"));
    assert!(manifest.to_string().contains(".codex/hooks.json"));
}

#[test]
fn cursor_hooks_json_matches_workspace_template_seven_event_set() {
    let contract = router_rs_json(&["schema-drift", "contract"]);
    let required: Vec<String> = contract["cursor_hooks_required"]
        .as_array()
        .expect("cursor_hooks_required")
        .iter()
        .map(|v| v.as_str().expect("event name").to_string())
        .collect();
    let forbidden: Vec<String> = contract["cursor_hooks_forbidden"]
        .as_array()
        .expect("cursor_hooks_forbidden")
        .iter()
        .map(|v| v.as_str().expect("event name").to_string())
        .collect();

    let hooks = read_json(&project_root().join(".cursor/hooks.json"));
    let template = read_json(
        &project_root().join("configs/framework/cursor-hooks.workspace-template.json"),
    );
    for doc in [(&hooks, "hooks.json"), (&template, "workspace-template")] {
        let events = doc.0["hooks"].as_object().expect("hooks object");
        for ev in &required {
            let key = ev.clone();
            assert!(
                events.contains_key(&key),
                "{} missing event {}",
                doc.1,
                ev
            );
            let cmd = events[&key][0]["command"].as_str().unwrap_or("");
            assert!(
                cmd.contains("cursor-router-rs-hook.sh"),
                "{} {} must use cursor-router-rs-hook.sh",
                doc.1,
                ev
            );
        }
        for ev in &forbidden {
            let key = ev.clone();
            assert!(
                !events.contains_key(&key),
                "{} must not register removed event {}",
                doc.1,
                ev
            );
        }
    }
    let h = hooks["hooks"].as_object().unwrap();
    let t = template["hooks"].as_object().unwrap();
    assert_eq!(h.keys().collect::<Vec<_>>(), t.keys().collect::<Vec<_>>());
    for ev in &required {
        let key = ev.clone();
        assert_eq!(
            h[&key][0]["timeout"], t[&key][0]["timeout"],
            "timeout mismatch on {ev}"
        );
        assert_eq!(
            h[&key][0]["command"], t[&key][0]["command"],
            "command mismatch on {ev}"
        );
    }
    assert_eq!(h.get("postToolUse").unwrap()[0]["timeout"], 20);
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
        "core/router-rs/src/host_integration.rs",
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

fn browser_mcp_rust_sources_concat() -> String {
    let root = project_root().join("core/router-rs/src/browser_mcp");
    let mut paths = collect_files_with_extension(&root, "rs");
    assert!(
        !paths.is_empty(),
        "expected Rust sources under {}",
        root.display()
    );
    paths.sort();
    paths
        .into_iter()
        .map(|p| read_text(&p))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn browser_mcp_exposes_repo_skill_router_tools() {
    let source = browser_mcp_rust_sources_concat();
    for marker in [
        "skill_route",
        "skill_search",
        "skill_read",
        "skill_route_status",
        "skills/SKILL_ROUTING_RUNTIME.json",
        "Read selected_skill_path from the canonical skills/ source before doing task work.",
    ] {
        assert!(source.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn install_skills_uses_rust_only_entrypoints() {
    assert!(!project_root().join("scripts/install_skills.sh").exists());
    let source = read_text(&project_root().join("core/router-rs/src/host_integration.rs"));
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
    let sync_source =
        read_text(&project_root().join("core/router-rs/src/host_entrypoint_sync.rs"));
    assert!(sync_source.contains("sync_host_entrypoints"));
    assert!(sync_source.contains("HostEntrypointPayloadProvider"));
    for forbidden in [
        "crate::codex_hooks",
        "build_codex_",
        "ensure_codex_skill_surface",
    ] {
        assert!(
            !sync_source.contains(forbidden),
            "host_entrypoint_sync must stay provider-based and host-neutral: {forbidden}"
        );
    }
    let codex_source = read_text(
        &project_root().join("core/router-rs/src/hosts/codex_hooks/mod.rs"),
    );
    assert!(codex_source.contains("codex_host_entrypoint_provider"));
    assert!(codex_source.contains("HostEntrypointPayloadProvider"));
}

#[test]
fn prompt_policy_is_rust_owned() {
    let root = project_root();
    let mod_rs = read_text(&root.join("core/router-rs/src/framework_runtime/mod.rs"));
    let compression =
        read_text(&root.join("core/router-rs/src/framework_runtime/prompt_compression.rs"));
    assert!(mod_rs.contains("build_framework_prompt_compression_envelope"));
    assert!(compression.contains("prompt_policy_owner"));
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
    let proxy_root = project_root().join("openai_proxy");
    if !proxy_root.join("config.yaml").is_file() {
        // openai_proxy removed in bfd7d87; keep test for forks that still ship the directory.
        return;
    }
    let config = read_text(&proxy_root.join("config.yaml"));
    let start_script = read_text(&proxy_root.join("start.sh"));
    assert!(config.contains("__OPENAI_PROXY_API_KEY__"));
    assert!(!config.contains("qscxzaq75321470"));
    assert!(!config.contains("sk-aggregator-"));
    assert!(start_script.contains("OPENAI_PROXY_API_KEY"));
}

#[test]
fn slides_native_pptx_lane_has_no_node_package_runtime() {
    let root = project_root().join("skills/slides");
    assert!(!root.join("package.json").exists());
    assert!(!root.join("package-lock.json").exists());
    assert!(!root.join("assets/package.template.json").exists());
    assert!(!root.join("assets/ppt.commands.json").exists());
    assert!(collect_files_with_extension(&root, "js").is_empty());
    assert!(collect_files_with_extension(&root, "ts").is_empty());
}

#[test]
fn slides_native_pptx_docs_are_not_runtime_contract() {
    assert!(
        collect_files_with_extension(&project_root().join("skills/slides/scripts"), "py")
            .is_empty()
    );
    let skill = read_text(&project_root().join("skills/slides/SKILL.md"));
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
    // This is an expensive integration test that depends on host PDF render tooling.
    // Keep the default contract suite portable by requiring an explicit opt-in.
    let enabled = std::env::var("SKILL_RUN_PPT_RENDER_TESTS")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled {
        return;
    }
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
fn slides_native_pptx_documents_design_and_aigc_gates() {
    let skill = read_text(&project_root().join("skills/slides/SKILL.md"));
    let workflow =
        read_text(&project_root().join("skills/slides/references/native-pptx/workflow.md"));
    let design_system =
        read_text(&project_root().join("skills/slides/references/native-pptx/design-system.md"));
    let checklist =
        read_text(&project_root().join("skills/slides/references/native-pptx/checklist.md"));
    let native_docs = format!("{skill}\n{workflow}");

    for token in [
        "$design-md",
        "$visual-review",
        "built-in Rust copy naturalization",
        "$copywriting",
        "$paper-writing",
        "Native PPTX References",
        "Text And Design Polishing Chain",
        "Rust inspection boost",
        "`deck.plan.json` stays the source of truth",
    ] {
        assert!(
            native_docs.contains(token),
            "missing native PPTX token: {token}"
        );
    }
    assert!(native_docs.contains(
        "outline -> text-owner polish -> DESIGN.md or visual contract -> deck.plan.json -> deck.pptx -> rendered\n\
PNG -> visual-review evidence -> design-md verdict -> ppt\n\
qa/build-qa sign-off"
    ));
    for marker in [
        "Copy Naturalization First",
        "Text Skill Loop",
        "$copywriting",
        "$paper-writing",
        "DESIGN.md / visual contract",
        "$visual-review",
        "match / minor drift / material drift",
        "hard fail",
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
        "fresh premium visual direction",
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
        "built-in Rust copy naturalization",
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
    let docs = markdown_text_under(&[project_root().join("skills/slides/references/native-pptx")]);
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
    let layout =
        read_text(&project_root().join("skills/slides/references/native-pptx/layout-patterns.md"));
    let method = read_text(&project_root().join("skills/slides/references/native-pptx/method.md"));
    let rust_cli =
        read_text(&project_root().join("skills/slides/references/native-pptx/rust-cli.md"));
    let visualization = read_text(
        &project_root().join("skills/slides/references/native-pptx/visualization_patterns.md"),
    );
    let install =
        read_text(&project_root().join("skills/slides/references/native-pptx/install.md"));

    assert!(layout.contains("Auto-Selection Rules"));
    assert!(layout.contains("choose the pattern that creates the clearest reading path"));
    assert!(method.contains("Rust Source-First Habit"));
    assert!(method.contains("change `deck.plan.json`, then rebuild"));
    assert!(rust_cli.contains("Rust `ppt office ...` owns inspection"));
    assert!(rust_cli.contains("not a package wrapper or\na second runtime"));
    assert!(rust_cli
        .contains("built-in Rust copy naturalization plus `$copywriting` / `$paper-writing"));
    assert!(visualization.contains("Prefer editable primitives"));
    assert!(install.contains("There is no skill-local package install step"));
    assert!(install.contains("text and design intentional"));
}

#[test]
fn slides_gate_is_executable_and_evidence_closed() {
    let skill = read_text(&project_root().join("skills/slides/SKILL.md"));
    for marker in [
        "Do not stop to ask for goal, audience, visual bar, or format when a safe default exists",
        "Re-run routing or consult the fallback manifest for that exact owner",
        "Rust `ppt` CLI",
        "cargo run --manifest-path rust_tools/pptx_tool_rs/Cargo.toml --bin ppt -- <command>",
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
        "built-in Rust copy naturalization",
        "$copywriting",
        "$paper-writing",
        "design-md drift verdict",
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

fn allowed_python_control_plane_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text == ".cursor/hook-tests/test_install_codex_cli_hooks.py"
        || text.starts_with(".cursor/hook-tests/tmp_")
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

#[test]
fn closeout_record_schema_is_published() {
    let path = project_root().join("configs/framework/CLOSEOUT_RECORD_SCHEMA.json");
    assert!(
        path.exists(),
        "expected closeout record schema at {}",
        path.display()
    );
    let schema = read_json(&path);
    assert_eq!(schema["schema_version"], "closeout-record-v1");
    let required = schema["required_fields"]
        .as_array()
        .expect("required_fields array");
    for expected in [
        "schema_version",
        "task_id",
        "verification_status",
        "summary",
    ] {
        assert!(
            required.iter().any(|v| v == expected),
            "closeout schema missing required field: {expected}"
        );
    }
    let rules = schema["enforcement_rules"]
        .as_array()
        .expect("enforcement_rules array");
    let schema_rules = rules
        .iter()
        .map(|rule| rule["id"].as_str().expect("rule id").to_string())
        .collect::<BTreeSet<_>>();
    let contract = router_rs_json(&["closeout", "contract"]);
    let contract_rules = contract["rules"]
        .as_array()
        .expect("contract rules")
        .iter()
        .map(|rule| rule.as_str().expect("contract rule id").to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        schema_rules, contract_rules,
        "CLOSEOUT_RECORD_SCHEMA.enforcement_rules must stay aligned with router-rs closeout contract"
    );
}

#[test]
fn closeout_evaluate_blocks_unverified_completion_via_cli() {
    let payload = serde_json::json!({
        "schema_version": "closeout-record-v1",
        "task_id": "policy-contract-1",
        "summary": "已完成 deck rebuild",
        "verification_status": "not_run",
    });
    let response = router_rs_json(&["closeout", "evaluate", "--input-json", &payload.to_string()]);
    assert_eq!(response["closeout_allowed"], false);
    let violations = response["violations"].as_array().expect("violations array");
    assert!(violations
        .iter()
        .any(|v| v["rule"] == "claimed_done_without_evidence"));
}

#[test]
fn closeout_evaluate_allows_clean_record_via_cli() {
    let payload = serde_json::json!({
        "schema_version": "closeout-record-v1",
        "task_id": "policy-contract-2",
        "summary": "Refactored builder; not yet executed",
        "verification_status": "partial",
        "changed_files": ["ppt/build_deck.py"],
        "risks": ["did not run python build_deck.py because PIL missing"]
    });
    let response = router_rs_json(&["closeout", "evaluate", "--input-json", &payload.to_string()]);
    assert_eq!(response["closeout_allowed"], true, "got {response:#?}");
    assert_eq!(response["claimed_completion"], false);
}

#[test]
fn closeout_evaluate_uses_task_evidence_context_via_cli() {
    let tmp = tempdir().unwrap();
    let repo = tmp.path();
    let task_id = "policy-context-closeout";
    let record_dir = repo.join("artifacts/closeout");
    fs::create_dir_all(&record_dir).unwrap();
    let record_path = record_dir.join(format!("{task_id}.json"));
    let payload = serde_json::json!({
        "schema_version": "closeout-record-v1",
        "task_id": task_id,
        "summary": "tests passed and task completed",
        "verification_status": "passed",
        "artifacts_checked": [{"path": "target/debug/app", "exists": true}]
    });
    fs::write(&record_path, serde_json::to_string(&payload).unwrap()).unwrap();
    let response = router_rs_json(&[
        "closeout",
        "evaluate",
        "--repo-root",
        repo.to_str().unwrap(),
        "--task-id",
        task_id,
        "--record-path",
        record_path.to_str().unwrap(),
    ]);
    assert_eq!(response["closeout_allowed"], false, "got {response:#?}");
    assert!(response["violations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["rule"] == "claimed_passed_without_evidence_index_rows"));
}

#[test]
fn closeout_contract_command_lists_rules() {
    let response = router_rs_json(&["closeout", "contract"]);
    assert_eq!(
        response["record_schema_version"], "closeout-record-v1",
        "got {response:#?}"
    );
    let rules = response["rules"].as_array().expect("rules array");
    assert!(rules
        .iter()
        .any(|v| v == "verification_passed_with_missing_artifact"));
}

#[test]
fn eval_route_cli_reports_metrics() {
    let cases_path = project_root().join("tests/routing_eval_cases.json");
    let cases_json = read_json(&cases_path);
    let expected_total = cases_json["cases"]
        .as_array()
        .expect("routing eval cases array")
        .len();
    let response = router_rs_json(&["eval", "route", "--cases", &cases_path.to_string_lossy()]);
    assert_eq!(
        response["total_cases"].as_u64().expect("total_cases") as usize,
        expected_total
    );
    // Routing regression gate: route_accuracy must be >= 0.95 across all eval cases.
    let route_accuracy = response["route_accuracy"]
        .as_f64()
        .expect("route_accuracy field");
    assert!(
        route_accuracy >= 0.95,
        "Routing regression detected: route_accuracy {:.4} < 0.95 threshold \
         ({} passed, {} failed out of {} total). \
         Fix the failing cases in tests/routing_eval_cases.json before merging.",
        route_accuracy,
        response["passed"].as_u64().unwrap_or(0),
        response["failed"].as_u64().unwrap_or(0),
        expected_total,
    );
    assert!(response["passed"].as_u64().unwrap() > 0);
    // overtrigger must be zero (false positives are worse than false negatives)
    let wrong_owner_rate = response["wrong_owner_rate"]
        .as_f64()
        .unwrap_or(0.0);
    assert!(
        wrong_owner_rate < 0.05,
        "Wrong-owner rate {:.4} exceeds 5% tolerance (threshold 0.05).",
        wrong_owner_rate,
    );
    // Per-case owner_correct: every eval case with expected_owner must route
    // to the expected skill. This catches manifest-only slugs that were
    // previously missing from the runtime index.
    let failures = response["failures"]
        .as_array()
        .expect("failures array");
    let owner_failures: Vec<&serde_json::Value> = failures
        .iter()
        .filter(|f| f["field"].as_str() == Some("selected_skill"))
        .collect();
    assert!(
        owner_failures.is_empty(),
        "Per-case owner mismatch detected ({} case(s)): {}",
        owner_failures.len(),
        owner_failures
            .iter()
            .map(|f| format!(
                "\n  case={}: expected={} got={}",
                f["case_id"].as_str().unwrap_or("?"),
                f["expected"].as_str().unwrap_or("?"),
                f["got"].as_str().unwrap_or("?"),
            ))
            .collect::<String>(),
    );
}

#[test]
fn eval_route_contract_cli_lists_metrics() {
    let response = router_rs_json(&["eval", "route-contract"]);
    assert_eq!(
        response["schema_version"], "routing-eval-report-v1",
        "got {response:#?}"
    );
    let metrics = response["metrics"].as_array().expect("metrics array");
    assert!(metrics.iter().any(|v| v == "route_accuracy"));
    assert!(metrics.iter().any(|v| v == "wrong_owner_rate"));
}

#[test]
fn harness_failure_taxonomy_config_matches_cli_contract() {
    let config = read_json(&project_root().join("configs/framework/HARNESS_FAILURE_TAXONOMY.json"));
    assert_eq!(config["schema_version"], "harness-failure-taxonomy-v1");
    let config_classes = config["classes"]
        .as_array()
        .expect("classes array")
        .iter()
        .map(|v| {
            (
                v["id"].as_str().expect("class id").to_string(),
                v["description"]
                    .as_str()
                    .expect("class description")
                    .to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let response = router_rs_json(&["eval", "harness-contract"]);
    let contract_classes = response["failure_taxonomy"]
        .as_array()
        .expect("failure taxonomy array")
        .iter()
        .map(|v| {
            (
                v["id"].as_str().expect("taxonomy id").to_string(),
                v["description"]
                    .as_str()
                    .expect("taxonomy description")
                    .to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(config_classes, contract_classes);
    for expected in [
        "route_miss",
        "verification_missing",
        "subagent_misuse",
        "trace_gap",
        "step_recovery_gap",
    ] {
        assert!(
            contract_classes.contains_key(expected),
            "missing {expected}"
        );
    }
}

#[test]
fn harness_behavioral_eval_cases_cover_required_tracks() {
    let config =
        read_json(&project_root().join("configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json"));
    assert_eq!(config["schema_version"], "harness-behavioral-eval-cases-v1");
    let tracks = config["tracks"]
        .as_array()
        .expect("tracks array")
        .iter()
        .map(|v| v["id"].as_str().expect("track id").to_string())
        .collect::<BTreeSet<_>>();
    let cases = config["cases"].as_array().expect("cases array");
    let case_ids = cases
        .iter()
        .map(|v| v["id"].as_str().expect("case id").to_string())
        .collect::<BTreeSet<_>>();
    let taxonomy_ids =
        read_json(&project_root().join("configs/framework/HARNESS_FAILURE_TAXONOMY.json"))
            ["classes"]
            .as_array()
            .expect("taxonomy classes")
            .iter()
            .map(|v| v["id"].as_str().expect("failure class id").to_string())
            .collect::<BTreeSet<_>>();
    let response = router_rs_json(&["eval", "harness-contract"]);
    let contract_tracks = response["behavioral_eval_tracks"]
        .as_array()
        .expect("contract tracks")
        .iter()
        .map(|v| v.as_str().expect("track").to_string())
        .collect::<BTreeSet<_>>();
    assert!(contract_tracks.is_subset(&tracks));
    for expected in [
        "routing_accuracy",
        "token_efficiency",
        "long_task_continuity",
        "trajectory_health",
        "closeout_integrity",
        "skill_contract_quality",
        "subagent_lane_integrity",
        "review_gate_integrity",
        "contract_integrity",
    ] {
        assert!(tracks.contains(expected), "missing track {expected}");
    }
    for track in config["tracks"].as_array().expect("tracks array") {
        for case_id in track["case_ids"].as_array().expect("case_ids") {
            let case_id = case_id.as_str().expect("case id");
            assert!(
                case_ids.contains(case_id),
                "track {} references missing case {case_id}",
                track["id"].as_str().unwrap_or("<unknown>")
            );
        }
    }
    for case in cases {
        let failure_class = case["failure_class"].as_str().expect("failure_class");
        assert!(
            taxonomy_ids.contains(failure_class),
            "case {} uses unknown failure_class {failure_class}",
            case["id"].as_str().unwrap_or("<unknown>")
        );
        assert!(
            case["verify"]
                .as_str()
                .unwrap_or_default()
                .contains("cargo ")
                || case["verify"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("router-rs "),
            "case {} must name an executable verification command",
            case["id"].as_str().unwrap_or("<unknown>")
        );
    }
}

#[test]
fn cursor_subagent_hook_contract_consumer_subset() {
    let path = project_root().join("configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json");
    assert!(
        path.is_file(),
        "missing {}",
        path.display()
    );
    let v = read_json(&path);
    assert_eq!(
        v["schema_version"].as_str().unwrap_or_default(),
        "cursor-subagent-hook-contract-v1"
    );
    let events = v["events"].as_object().expect("events object");
    assert!(events.contains_key("subagentStart"));
    assert!(events.contains_key("subagentStop"));
    let modes = v["modes"].as_object().expect("modes object");
    assert!(modes.contains_key("strict"));
    assert!(modes.contains_key("review_lite"));
    assert_eq!(
        modes["review_lite"]["doc_alias"].as_str(),
        Some("review-lite")
    );
    let fields = v["fields"].as_object().expect("fields object");
    assert!(fields.contains_key("subagent_id"));
    let fork = fields
        .get("fork_context")
        .and_then(|f| f.as_object())
        .expect("fork_context object");
    let accepted = fork
        .get("accepted_false_values")
        .and_then(|a| a.as_object())
        .expect("fork_context.accepted_false_values");
    let strings = accepted["string"]
        .as_array()
        .expect("fork_context accepted string spellings");
    for spelling in ["false", "0", "no", "n"] {
        assert!(
            strings.iter().any(|v| v.as_str() == Some(spelling)),
            "fork_context contract must document string spelling {spelling:?}"
        );
    }
    assert_eq!(
        fork["independent_when"].as_str().unwrap_or_default(),
        "json_boolean_false_or_integer_0_or_string_false_0_no_n"
    );
}

#[test]
fn harness_skill_contract_lint_cli_reports_protocol_shape() {
    let payload = serde_json::json!({
        "skills_root": project_root().join("skills").to_string_lossy(),
        "slugs": ["skill-framework-developer", "plan-mode", "agent-swarm-orchestration", "research-workbench", "openai-docs"]
    });
    let response = router_rs_json(&[
        "eval",
        "skill-contract-lint",
        "--input-json",
        &payload.to_string(),
    ]);
    assert_eq!(
        response["schema_version"],
        "router-rs-harness-skill-contract-lint-v1"
    );
    assert_eq!(
        response["skills_scanned"]
            .as_array()
            .expect("skills scanned")
            .len(),
        5
    );
    assert!(response["findings"].is_array());
    assert!(response["execution_items"].is_array());
    assert!(response["verification_results"].is_array());
    assert!(
        response["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .all(|finding| finding["severity"] != "major"),
        "default high-impact lint must not report major findings: {response:#?}"
    );
    assert_eq!(
        response["verification_results"][0]["status"], "pass",
        "default high-impact lint must be a gate, not shape-only: {response:#?}"
    );
}

#[test]
fn framework_step_ledger_append_projects_summary_into_task_state() {
    let tmp = tempdir().unwrap();
    let repo = tmp.path();
    let payload = serde_json::json!({
        "operation": "append",
        "repo_root": repo.to_string_lossy(),
        "task_id": "step-ledger-policy",
        "step_id": "plan-1",
        "phase": "implementation",
        "status": "pass",
        "input_text": "implement harness plan",
        "retry_count": 0,
        "side_effects": [],
        "evidence_ref": {"kind":"manual","label":"unit-test"},
        "next_resume_hint": "continue at verify"
    });
    let response = router_rs_json(&[
        "framework",
        "step-ledger",
        "--input-json",
        &payload.to_string(),
    ]);
    assert_eq!(
        response["schema_version"],
        "router-rs-step-ledger-response-v1"
    );
    let summary_payload = serde_json::json!({
        "operation": "summary",
        "repo_root": repo.to_string_lossy(),
        "task_id": "step-ledger-policy"
    });
    let summary = router_rs_json(&[
        "framework",
        "step-ledger",
        "--input-json",
        &summary_payload.to_string(),
    ]);
    assert_eq!(summary["entry_count"], 1);
    assert_eq!(summary["latest"]["step_id"], "plan-1");
    let task_state = read_json(
        &repo
            .join("artifacts/current/step-ledger-policy")
            .join("TASK_STATE.json"),
    );
    assert_eq!(task_state["step_ledger"]["entry_count"], 1);
    assert_eq!(
        task_state["step_ledger"]["latest"]["next_resume_hint"],
        "continue at verify"
    );
}

#[test]
fn paper_prose_quality_hook_txt_exists_and_nl_signal_registered() {
    let root = project_root();
    let prose_txt = root.join("configs/framework/PAPER_PROSE_QUALITY_HOOK.txt");
    assert!(
        prose_txt.is_file(),
        "missing PAPER_PROSE_QUALITY_HOOK.txt at {}",
        prose_txt.display()
    );
    let body = read_text(&prose_txt);
    assert!(
        body.contains("PAPER_PROSE_QUALITY_HOOK") || body.contains("language_register"),
        "prose hook txt must contain actionable prose gate hints"
    );
    let nl = read_json(&root.join("configs/framework/NL_ROUTE_ADJUSTMENTS.json"));
    let post_rules = nl["post_framework_alias_rules"]
        .as_array()
        .expect("nl post_framework_alias_rules array");
    let has_prose_boost = post_rules.iter().any(|rule| {
        rule.get("when")
            .and_then(|w| w.get("signal"))
            .and_then(Value::as_str)
            == Some("has_paper_prose_edit_context")
            && rule
                .get("record")
                .and_then(|r| r.get("slug"))
                .and_then(Value::as_str)
                == Some("paper-workbench")
            && rule.get("action").and_then(|a| a.get("type")).and_then(Value::as_str) == Some("boost")
    });
    assert!(
        has_prose_boost,
        "NL_ROUTE_ADJUSTMENTS must boost paper-workbench on has_paper_prose_edit_context"
    );
    let has_writing_boost = post_rules.iter().any(|rule| {
        rule.get("when")
            .and_then(|w| w.get("signal"))
            .and_then(Value::as_str)
            == Some("has_paper_writing_context")
            && rule
                .get("record")
                .and_then(|r| r.get("slug"))
                .and_then(Value::as_str)
                == Some("paper-workbench")
            && rule.get("action").and_then(|a| a.get("type")).and_then(Value::as_str) == Some("boost")
    });
    assert!(
        has_writing_boost,
        "NL_ROUTE_ADJUSTMENTS must boost paper-workbench on has_paper_writing_context"
    );
    for rule in post_rules {
        let slug = rule
            .get("record")
            .and_then(|r| r.get("slug"))
            .and_then(Value::as_str);
        if let Some(s) = slug {
            assert!(
                s != "paper-reviewer" && s != "paper-reviser",
                "post_framework_alias_rules must not target dead hot-route slugs: {s}"
            );
        }
    }
    let signals_rs = read_text(&root.join("core/router-rs/src/route/nl_route_adjustments.rs"));
    assert!(
        signals_rs.contains("has_paper_prose_negation_context"),
        "nl_route_adjustments must register has_paper_prose_negation_context"
    );
    let paper_prose_hook_rs = read_text(&root.join("core/router-rs/src/paper_prose_hook.rs"));
    for env in [
        "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK",
        "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
        "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK",
        "ROUTER_RS_ANTIGRAVITY_CLI_PAPER_PROSE_HOOK",
    ] {
        assert!(
            paper_prose_hook_rs.contains(env),
            "paper_prose_hook.rs must declare {env}"
        );
    }
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
                Some(
                    ".git"
                        | "target"
                        | "node_modules"
                        | ".venv"
                        | "venv"
                        | "__pycache__"
                        | "codex-skill-surface"
                        | "generated-artifacts-drift-check"
                )
            ) {
                continue;
            }
            collect_files(&path, visitor);
        } else if path.is_file() {
            visitor(&path);
        }
    }
}
