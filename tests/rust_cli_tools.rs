#![allow(dead_code)]
mod common;

use common::{assert_success, json_from_output, project_root, read_text, run};
use serde_json::Value;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn update_audit_cli_contract_is_registered() {
    let args = read_text(&project_root().join("core/runtime-core/src/cli/args.rs"));
    let maint = read_text(&project_root().join("core/runtime-core/src/framework_maint.rs"));
    assert!(args.contains("UpdateAudit(UpdateAuditArgs)"));
    assert!(args.contains("repo_root: Option<PathBuf>"));
    assert!(args.contains("Dry-run `/update` repository knowledge/hygiene audit"));
    assert!(maint.contains("MaintSubcommand::UpdateAudit(args) => update_audit(args)"));
    assert!(maint.contains("\"schema_version\": \"framework-maint-update-audit-v1\""));
    for key in [
        "key_document_candidates",
        "git_tracking",
        "suspected_dead_code_markers",
        "suspected_stale_docs",
        "suspected_retired_files",
        "recommended_actions",
    ] {
        assert!(
            maint.contains(key),
            "missing update-audit output key: {key}"
        );
    }
    assert!(maint.contains("research data"));
}

#[test]
fn refresh_host_projections_keeps_claude_code_projection_explicit() {
    let args = read_text(&project_root().join("core/runtime-core/src/cli/args.rs"));
    let maint = read_text(&project_root().join("core/runtime-core/src/framework_maint.rs"));
    assert!(args.contains("non-Codex framework installs"));
    assert!(maint.contains("let installable_tools = installable_projection_tools(&fw)?"));
    assert!(maint.contains("verify_installable_projections(&fw, &installable_tools)?"));
    assert!(maint.contains("for tool in &projection_tools"));
    assert!(maint.contains("verify_claude_code_projection"));
    assert!(maint.contains(".claude/rules/framework.md"));
    assert!(maint.contains(".claude/.framework-projection.json"));
    assert!(maint.contains("host_projection: claude"));
}

#[test]
fn update_audit_runs_on_plain_git_repo_and_preserves_status_columns() {
    let tmp = tempdir().unwrap();
    let mut git_init = Command::new("git");
    git_init.arg("init").current_dir(tmp.path());
    assert_success(&run(git_init));
    std::fs::write(tmp.path().join("README.md"), "initial\n").unwrap();
    let mut git_add = Command::new("git");
    git_add.args(["add", "README.md"]).current_dir(tmp.path());
    assert_success(&run(git_add));
    let mut git_commit = Command::new("git");
    git_commit
        .args([
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=Test User",
            "commit",
            "-m",
            "seed",
        ])
        .current_dir(tmp.path());
    assert_success(&run(git_commit));
    std::fs::write(tmp.path().join("README.md"), "changed\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("notes")).unwrap();
    std::fs::write(
        tmp.path().join("notes/research-plan.md"),
        "TODO: update experiment plan\n",
    )
    .unwrap();

    let output = run({
        let mut c = Command::new("cargo");
        c.args(["run", "--quiet", "--manifest-path"])
            .arg(project_root().join("core/router-rs/Cargo.toml"))
            .args(["--bin", "router-rs-cli"])
            .current_dir(project_root())
            .arg("--")
            .args([
                "framework",
                "maint",
                "update-audit",
                "--repo-root",
                tmp.path().to_str().unwrap(),
            ]);
        c
    });
    let payload = json_from_output(&output);
    assert_eq!(payload["schema_version"], "framework-maint-update-audit-v1");
    assert_eq!(payload["mode"], "dry-run");
    assert_eq!(payload["mutates_files"], false);
    let status = payload["git_tracking"]["status_porcelain"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        status.iter().any(|line| line.starts_with(" M README.md")),
        "status output should preserve porcelain leading columns: {status:?}"
    );
    let key_docs = payload["key_document_candidates"].as_array().unwrap();
    assert!(key_docs.iter().any(|item| {
        item["path"] == "notes/research-plan.md" && item["tracking"] == "untracked"
    }));
    let stale_docs = payload["suspected_stale_docs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        stale_docs
            .iter()
            .any(|line| line.starts_with("notes/research-plan.md:1:TODO")),
        "untracked research doc TODO should be scanned: {stale_docs:?}"
    );
}
