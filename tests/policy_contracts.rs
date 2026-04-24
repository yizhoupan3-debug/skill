mod common;

use common::{cargo_manifest_command, json_from_output, project_root, read_json, read_text, run};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[test]
fn project_claude_agents_stay_retired() {
    assert!(
        !project_root().join(".claude/agents").exists(),
        "Project Claude subagents are retired; keep reusable behavior in skills/."
    );
}

#[test]
fn plugin_manifest_exposes_skills_and_mcp_bundle() {
    let plugin_root = project_root().join("plugins/skill-framework-native");
    let manifest = read_json(&plugin_root.join(".codex-plugin/plugin.json"));
    assert_eq!(manifest["name"], "skill-framework-native");
    assert_eq!(manifest["skills"], "./skills/");
    assert_eq!(manifest["mcpServers"], "./.mcp.json");
    assert_eq!(
        manifest["interface"]["displayName"],
        "Skill Framework Native"
    );
}

#[test]
fn plugin_mcp_bundle_points_back_to_repo_root() {
    let payload = read_json(&project_root().join("plugins/skill-framework-native/.mcp.json"));
    let framework = &payload["mcpServers"]["framework-mcp"];
    assert_eq!(
        framework["command"],
        "./scripts/router-rs/target/release/router-rs"
    );
    assert_eq!(
        framework["args"],
        serde_json::json!(["--framework-mcp-stdio", "--repo-root", "../.."])
    );
    assert_eq!(framework["cwd"], "../..");
    assert_eq!(payload["mcpServers"].as_object().unwrap().len(), 1);
}

#[test]
fn marketplace_registers_local_plugin_when_fixture_exists() {
    let marketplace_path = project_root().join(".agents/plugins/marketplace.json");
    if !marketplace_path.is_file() {
        return;
    }
    let marketplace = read_json(&marketplace_path);
    assert_eq!(
        marketplace["interface"]["displayName"],
        "Skill Local Marketplace"
    );
    let plugin = &marketplace["plugins"][0];
    assert_eq!(plugin["name"], "skill-framework-native");
    assert_eq!(plugin["source"]["path"], "./plugins/skill-framework-native");
    assert_eq!(plugin["policy"]["installation"], "AVAILABLE");
}

#[test]
fn gitx_skill_exposes_codex_shortcut_and_closeout_flow() {
    let content = read_text(&project_root().join("skills/gitx/SKILL.md"));
    for marker in [
        "name: gitx",
        "$gitx",
        "review、修复、整理、提交、合并 worktree、推送",
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
fn refresh_skill_stays_available_for_codex_global_entry() {
    let skill_path = project_root().join("skills/refresh/SKILL.md");
    let content = read_text(&skill_path);
    assert!(skill_path.is_file());
    for marker in [
        "name: refresh",
        "$refresh",
        r#"PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}""#,
        r#""$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR""#,
        r#""$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR""#,
        r#"cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR""#,
        "下一轮执行 prompt 已准备好，并且已经复制到剪贴板。",
        "--framework-refresh-verbose",
    ] {
        assert!(content.contains(marker), "missing marker: {marker}");
    }
    assert!(!content.contains("manual next-turn execution prompt"));
}

#[test]
fn latex_compile_acceleration_discovery_surface_is_precise() {
    let content = read_text(&project_root().join("skills/latex-compile-acceleration/SKILL.md"));
    for marker in [
        "name: latex-compile-acceleration",
        "At 每轮对话开始 / first-turn / conversation start",
        "LaTeX 编译太慢",
        "TikZ externalization",
        "preamble 预编译",
        "prefer this skill over ppt-beamer",
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
    for skill in ["skills/doc", "skills/xlsx"] {
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
        project_root().join("skills/xlsx"),
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
        project_root().join("skills/xlsx/agents/openai.yaml"),
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
    let retired_paths = [
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
    let existing: Vec<_> = retired_paths
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
fn installed_project_hooks_use_router_rs_only() {
    for surface in [".claude/settings.json", ".codex/hooks.json"] {
        let payload = read_json(&project_root().join(surface));
        let mut commands = Vec::new();
        for entries in payload["hooks"].as_object().unwrap().values() {
            for entry in entries.as_array().unwrap() {
                for hook in entry["hooks"].as_array().unwrap() {
                    commands.push(hook["command"].as_str().unwrap().to_string());
                }
            }
        }
        assert!(!commands.is_empty());
        assert!(commands.iter().all(|command| command.contains("router-rs")));
        assert!(commands.iter().all(|command| !command.contains("python3")));
        assert!(commands.iter().all(|command| !command.contains(".py")));
        assert!(commands
            .iter()
            .all(|command| !command.contains("host-integration-rs")));
    }
}

#[test]
fn repo_local_codex_framework_mcp_uses_rust_only_entrypoint() {
    let source = read_text(&project_root().join(".codex/config.toml"));
    assert!(!source.contains("python3"));
    assert!(!source.contains("scripts.framework_mcp"));
    assert!(source.contains(
        r#"command = "/Users/joe/Documents/skill/scripts/router-rs/target/release/router-rs""#
    ));
    assert!(!source.contains("scripts/router-rs/Cargo.toml"));
    assert!(!source.contains(r#"command = "cargo""#));
    assert!(source.contains("--framework-mcp-stdio"));
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
    let source = read_text(&project_root().join("scripts/router-rs/src/claude_hooks.rs"));
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
fn package_template_uses_rust_tool_runner() {
    let package = read_json(&project_root().join("skills/ppt-pptx/assets/package.template.json"));
    assert!(package.get("dependencies").is_none());
    let scripts = package["scripts"].as_object().unwrap();
    for (name, command) in scripts {
        assert!(
            command.as_str().unwrap().starts_with("ppt "),
            "{name}: {command}"
        );
    }
    assert_eq!(
        scripts["build"],
        "ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered"
    );
}

#[test]
fn ppt_skill_package_uses_rust_smoke_test() {
    let package = read_json(&project_root().join("skills/ppt-pptx/package.json"));
    assert!(package.get("dependencies").is_none());
    assert!(package["scripts"]["smoke:test"]
        .as_str()
        .unwrap()
        .starts_with(
            "cargo run --manifest-path ../../rust_tools/pptx_tool_rs/Cargo.toml --bin ppt --"
        ));
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
    assert!(!temp.path().join("deck.js").exists());
    assert!(!temp.path().join("package-lock.json").exists());

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
    ] {
        assert!(skill.contains(token), "missing skill token: {token}");
    }
    assert!(skill.contains(
        "DESIGN.md or visual contract -> deck.plan.json -> deck.pptx -> rendered\n\
PNG -> visual-review evidence -> design-output-auditor verdict -> ppt\n\
qa/build-qa sign-off"
    ));
    for marker in [
        "Copy Naturalization First",
        "DESIGN.md / visual contract",
        "$visual-review",
        "$design-output-auditor",
        "match / minor drift / material drift /\nhard fail",
    ] {
        assert!(workflow.contains(marker), "missing workflow marker: {marker}");
    }
    for field in [
        "Visual Theme & Atmosphere",
        "Color Palette & Roles",
        "Typography Rules",
        "Layout Principles",
        "Generation Guardrails",
        "Anti-Patterns",
        "deck.plan.json",
        "match",
        "minor drift",
        "material drift",
        "hard fail",
    ] {
        assert!(design_system.contains(field), "missing design field: {field}");
    }
    for marker in [
        "本页展示",
        "AI-slop",
        "Rendered slides reviewed through `$visual-review`",
        "Design audit verdict is `match` or only acceptable `minor drift`",
    ] {
        assert!(checklist.contains(marker), "missing checklist marker: {marker}");
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
    ] {
        assert!(!docs.contains(forbidden), "{forbidden}");
    }
    assert!(docs.contains("Rust CLI"));
    assert!(docs.contains("deck.plan.json"));
    assert!(docs.contains("deck.pptx"));
}

#[test]
fn ppt_rust_outline_generation_naturalizes_copy_and_design_chain() {
    let source = read_text(&project_root().join("rust_tools/pptx_tool_rs/src/main.rs"));
    for marker in [
        "fn naturalize_outline_value(",
        "fn naturalize_copy_text(",
        "let outline = naturalize_outline_value(outline);",
        "generic AI filler",
        "design-output-auditor drift check",
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
