// Test helper functions that may not all be called from every test.
#![allow(dead_code)]

use crate::common::{
    assert_success, cargo_manifest_command, json_from_output, project_root, read_json, read_text,
    run, run_ok,
};
use crate::policy::policy_helpers::{collect_files_with_extension, markdown_text_under};
use std::process::Command;
use tempfile::tempdir;

#[test]
fn router_rs_main_binary_compiles() {
    let mut command = Command::new("cargo");
    command
        .args([
            "check",
            "--manifest-path",
            "core/router-rs/Cargo.toml",
            "--bin",
            "router-rs-cli",
        ])
        .current_dir(project_root());
    assert_success(&run(command));
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
        " -- docx <docx>",
        " -- xlsx",
        "render-docx",
        "render-xlsx",
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
    let main_source = read_text(&project_root().join("rust_tools/ooxml_parser_rs/src/main.rs"));
    let lib_source = read_text(&project_root().join("rust_tools/ooxml_parser_rs/src/lib.rs"));
    for marker in [
        "Docx { input, json }",
        "ReadDocx {",
        "ReadXlsx {",
        "RenderXlsx(RenderXlsxArgs)",
        "RenderDocx(RenderDocxArgs)",
    ] {
        assert!(
            main_source.contains(marker),
            "missing marker in main.rs: {marker}"
        );
    }
    for marker in [
        "fn inspect_docx(",
        "fn read_docx(",
        "fn read_xlsx(",
        "fn render_xlsx",
        "fn render_docx",
    ] {
        assert!(
            lib_source.contains(marker),
            "missing marker in lib.rs: {marker}"
        );
    }
}

#[test]
fn ooxml_rust_cli_owns_batch_subcommands() {
    let manifest_path = project_root().join("rust_tools/ooxml_parser_rs/Cargo.toml");
    let manifest = read_text(&manifest_path);
    assert!(manifest.contains("name = \"ooxml\""));

    let batch = read_text(&project_root().join("rust_tools/ooxml_parser_rs/src/batch.rs"));
    for marker in [
        "fn run_batch(",
        "read_docx_content",
        "read_xlsx_content",
        "OOXML",
    ] {
        assert!(batch.contains(marker), "batch.rs missing marker: {marker}");
    }

    let engine = read_text(&project_root().join("rust_tools/batch-common/src/engine.rs"));
    for marker in ["catalog.json", "results.jsonl", "checkpoint.json"] {
        assert!(
            engine.contains(marker),
            "batch-common engine.rs missing marker: {marker}"
        );
    }

    let main_path = project_root().join("rust_tools/ooxml_parser_rs/src/main.rs");
    let source = read_text(&main_path);
    for marker in ["Batch {", "stdin_paths", "print_catalog_summary"] {
        assert!(source.contains(marker), "main.rs missing marker: {marker}");
    }
}

#[test]
fn ooxml_install_script_exists() {
    let script = project_root().join("scripts/install-ooxml-tool.sh");
    assert!(script.exists(), "missing install-ooxml-tool.sh");
    let text = read_text(&script);
    assert!(text.contains("ooxml_parser_rs"));
    assert!(text.contains("rust-release-bin.sh"));
    let helper = read_text(&project_root().join("scripts/rust-release-bin.sh"));
    assert!(helper.contains("target_directory"));
}

#[test]
fn ppt_install_script_exists() {
    let script = project_root().join("scripts/install-ppt-tool.sh");
    assert!(script.exists(), "missing install-ppt-tool.sh");
    let text = read_text(&script);
    assert!(text.contains("pptx_tool_rs"));
    assert!(text.contains("/ppt"));
}

#[test]
fn doc_skill_declares_ooxml_batch_artifacts() {
    let skill = read_text(&project_root().join("skills/doc/SKILL.md"));
    assert!(skill.contains("ooxml-batch/catalog.json"));
    assert!(skill.contains("install-ooxml-tool.sh"));
    assert!(skill.contains("禁止") && skill.contains("cargo run"));
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
fn github_source_gate_rust_cli_owns_both_commands() {
    let source = read_text(&project_root().join("rust_tools/gh_source_gate_rs/src/lib.rs"));
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
fn browser_mcp_live_config_never_points_to_node_runtime() {
    let surfaces = [
        "core/host-projection/src/host_integration/mod.rs",
        "tools/browser-mcp/src/lib.rs",
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
    let root = project_root().join("tools/browser-mcp/src");
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
fn browser_mcp_does_not_expose_skill_routing_tools() {
    let source = browser_mcp_rust_sources_concat();
    // After tool separation, browser-mcp should NOT contain skill routing tools
    for marker in [
        "skill_route",
        "skill_search",
        "skill_read",
        "skill_route_status",
    ] {
        assert!(
            !source.contains(marker),
            "browser-mcp should NOT contain: {marker}"
        );
    }
    // But should still contain web_fetch
    assert!(
        source.contains("web_fetch"),
        "browser-mcp should contain web_fetch"
    );
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
fn pdf_rust_cli_owns_batch_subcommands() {
    let manifest_path = project_root().join("rust_tools/pdf_tool_rs/Cargo.toml");
    let manifest = read_text(&manifest_path);
    assert!(manifest.contains("name = \"pdf\""));
    assert!(manifest.contains(r#"name = "pdf_tool_rs""#));

    let main_path = project_root().join("rust_tools/pdf_tool_rs/src/main.rs");
    if main_path.exists() {
        let source = read_text(&main_path);
        for marker in [
            "Read {",
            "Info {",
            "Batch {",
            "stdin_paths",
            "run_batch",
            "print_catalog_summary",
        ] {
            assert!(source.contains(marker), "main.rs missing marker: {marker}");
        }
    } else {
        let batch = read_text(&project_root().join("rust_tools/pdf_tool_rs/src/batch.rs"));
        for marker in [
            "fn run_batch(",
            "catalog.json",
            "results.jsonl",
            "checkpoint.json",
        ] {
            assert!(batch.contains(marker), "batch.rs missing marker: {marker}");
        }
    }
}

#[test]
fn pdf_batch_skip_scanned_documented() {
    let guide = read_text(&project_root().join("skills/pdf/references/detailed-guide.md"));
    for marker in [
        "--skip-scanned",
        "skip_scanned",
        "content_class",
        "PDF_BENCH=1",
        "batch_bench",
    ] {
        assert!(
            guide.contains(marker),
            "detailed-guide.md missing pdf batch marker: {marker}"
        );
    }

    let read_rs = read_text(&project_root().join("rust_tools/pdf_tool_rs/src/read.rs"));
    for marker in [
        "shallow_scan_classify",
        "SHALLOW_SAMPLE_PAGES",
        "classify_content",
    ] {
        assert!(
            read_rs.contains(marker),
            "read.rs missing skip-scanned implementation marker: {marker}"
        );
    }

    let batch_rs = read_text(&project_root().join("rust_tools/pdf_tool_rs/src/batch.rs"));
    assert!(
        batch_rs.contains("skip_scanned"),
        "batch.rs must wire --skip-scanned"
    );

    let bench = read_text(&project_root().join("rust_tools/pdf_tool_rs/benches/batch_bench.rs"));
    assert!(
        bench.contains("PDF_BENCH"),
        "batch_bench.rs must gate on PDF_BENCH"
    );
}

#[test]
fn pdf_skill_frontmatter_declares_rust_runtime() {
    let skill = read_text(&project_root().join("skills/pdf/SKILL.md"));
    assert!(skill.contains("allowed_tools:"));
    assert!(skill.contains("- rust"));
    assert!(skill.contains("runtime_requirements:"));
    assert!(skill.contains("- pdf"));
    assert!(skill.contains("${SKILL_FRAMEWORK_ROOT}"));
    assert!(skill.contains("pdf-batch/catalog.json"));
    assert!(skill.contains("禁止") && skill.contains("cargo run"));
}

#[test]
fn ppt_rust_manifest_exposes_direct_cli() {
    let manifest = read_text(&project_root().join("rust_tools/pptx_tool_rs/Cargo.toml"));
    assert!(manifest.contains("name = \"ppt\""));
    assert!(manifest.contains("path = \"src/bin/ppt.rs\""));
    assert!(
        project_root()
            .join("rust_tools/pptx_tool_rs/src/bin/ppt.rs")
            .exists()
    );
}

#[test]
fn ppt_rust_cli_owns_workspace_and_outline_commands() {
    let main_source = read_text(&project_root().join("rust_tools/pptx_tool_rs/src/main.rs"));
    let lib_source = read_text(&project_root().join("rust_tools/pptx_tool_rs/src/lib.rs"));
    let commands_source =
        read_text(&project_root().join("rust_tools/pptx_tool_rs/src/commands.rs"));
    let qa_source = read_text(&project_root().join("rust_tools/pptx_tool_rs/src/qa.rs"));
    let office_source = read_text(&project_root().join("rust_tools/pptx_tool_rs/src/office.rs"));
    assert!(main_source.contains("Init(InitArgs)"));
    assert!(main_source.contains("Outline(OutlineArgs)"));
    assert!(main_source.contains("BuildQa(BuildQaArgs)"));
    assert!(lib_source.contains("fn init_workspace("));
    assert!(lib_source.contains("default_value = \"deck.plan.json\""));
    // workdir.join("deck.pptx") 和 QualityMode::Strict 已移至 commands.rs
    assert!(commands_source.contains("workdir.join(\"deck.pptx\")"));
    assert!(commands_source.contains("QualityMode::Strict"));
    assert!(lib_source.contains("fn strict_quality_gate("));
    assert!(lib_source.contains("fn write_pptx_package("));
    // build_pptx_slide_specs 已移至 slide_specs.rs 子模块
    let slide_specs_source =
        read_text(&project_root().join("rust_tools/pptx_tool_rs/src/slide_specs.rs"));
    assert!(slide_specs_source.contains("fn build_pptx_slide_specs("));
    assert!(office_source.contains("fn rust_office_outline_value("));
    assert!(office_source.contains("fn rust_office_issues_value("));
    assert!(office_source.contains("fn rust_office_validate_value("));
    assert!(office_source.contains("rust-pptx-inspector"));
    assert!(qa_source.contains("fn font_check_ok("));
    assert!(qa_source.contains("fn inspector_ok("));
    assert!(qa_source.contains("ok: bool"));
    assert!(!lib_source.contains("officecli"));
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
    assert_success(&run(init));

    let outline = temp.path().join("outline.json");
    assert!(temp.path().join("sources.md").is_file());
    let mut build = cargo_manifest_command(&manifest, &[]);
    build
        .args(["--bin", "ppt", "--", "outline"])
        .arg(&outline)
        .args(["--bootstrap", "--build", "--json"]);
    assert_success(&run(build));

    assert!(temp.path().join("deck.plan.json").is_file());
    assert!(temp.path().join("deck.pptx").is_file());
    assert!(temp.path().join("ppt.commands.json").is_file());
    assert!(!temp.path().join("deck.js").exists());
    assert!(!temp.path().join("package-lock.json").exists());

    let commands_manifest = read_json(&temp.path().join("ppt.commands.json"));
    assert_eq!(commands_manifest["runtime"].as_str(), Some("ppt"));
    let commands = commands_manifest["commands"].as_object().unwrap();
    assert!(
        commands
            .values()
            .all(|command| command.as_str().unwrap().starts_with("ppt "))
    );
    assert!(commands.contains_key("check_inspector"));
    assert!(commands.contains_key("watch_rust"));
    assert!(commands.contains_key("build_strict"));
    assert!(
        commands["check_rust"]
            .as_str()
            .unwrap()
            .contains("--fail-on-issues")
    );
    assert!(
        commands["build_strict"]
            .as_str()
            .unwrap()
            .contains("--quality strict")
    );

    let mut extract = cargo_manifest_command(&manifest, &[]);
    extract
        .args(["--bin", "ppt", "--", "extract-structure"])
        .arg(temp.path().join("deck.pptx"));
    let structure = json_from_output(&run(extract));
    assert_eq!(structure["slide_count"].as_u64(), Some(3));
    assert!(
        structure["slides"][0]["notes"]
            .as_str()
            .unwrap_or_default()
            .contains("Cover slide generated by the Rust ppt CLI.")
    );

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
    let query_text_output = run_ok(query_text);
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
    assert!(
        rust_cli
            .contains("built-in Rust copy naturalization plus `$copywriting` / `$paper-writing")
    );
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
        "slides_evidence.json",
        "Final response stays concise but includes the `.pptx` link and the verification evidence used",
        "workspace",
        "temp",
        "artifacts/scratch",
    ] {
        assert!(
            skill.contains(marker),
            "missing slides gate marker: {marker}"
        );
    }
    assert!(!skill.contains("@oai/artifact-tool"));
    assert!(!skill.contains("compact verification pass"));
    assert!(!skill.contains("Final response contains only"));
}

#[test]
fn ppt_rust_outline_generation_naturalizes_copy_and_design_chain() {
    // naturalize_outline_value 已移至 yaml_parse.rs，naturalize_copy_text 已移至 text_processing.rs
    let yaml_source = read_text(&project_root().join("rust_tools/pptx_tool_rs/src/yaml_parse.rs"));
    let text_source =
        read_text(&project_root().join("rust_tools/pptx_tool_rs/src/text_processing.rs"));
    for marker in [
        "fn naturalize_outline_value(",
        "let outline = naturalize_outline_value(outline);",
    ] {
        assert!(
            yaml_source.contains(marker),
            "missing marker in yaml_parse.rs: {marker}"
        );
    }
    for marker in ["fn naturalize_copy_text(", r#""本页展示""#, r#""赋能""#] {
        assert!(
            text_source.contains(marker),
            "missing marker in text_processing.rs: {marker}"
        );
    }
    // design-md drift verdict + copy naturalization markers 已移至 yaml_parse.rs
    for marker in [
        "design-md drift verdict",
        "generic AI filler",
        "built-in Rust copy naturalization",
        "$copywriting",
        "$paper-writing",
    ] {
        assert!(
            yaml_source.contains(marker),
            "missing marker in yaml_parse.rs: {marker}"
        );
    }
}
