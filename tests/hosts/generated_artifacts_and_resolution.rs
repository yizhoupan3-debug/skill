use crate::common::{
    host_integration_json, json_from_output, output_text, project_root,
    router_rs_command, router_rs_json, run, seed_framework_markers, write_json, write_text,
};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

/// Like `router_rs_json` but passes `HOME` only to the child process,
/// avoiding mutation of the global environment which causes flaky tests
/// when Rust runs tests in parallel.
fn router_rs_json_with_home(home: &Path, args: &[&str]) -> Value {
    let mut cmd = router_rs_command(args);
    cmd.env("HOME", home);
    json_from_output(&run(cmd))
}

#[test]
fn compatibility_alias_inventory_and_generated_artifacts_status_are_reported() {
    let framework_root = project_root();
    let aliases = router_rs_json(&["framework", "host-integration", "compatibility-aliases"]);
    assert_eq!(
        aliases["schema_version"],
        "framework-compatibility-alias-inventory-v1"
    );
    let alias_entries = aliases["aliases"].as_array().unwrap();
    let expected_aliases = [
        "codex host-integration ...",
        "framework host-integration install-skills",
        "--repo-root",
    ];
    for expected_alias in expected_aliases {
        let alias = alias_entries
            .iter()
            .find(|alias| alias["alias"] == expected_alias)
            .unwrap_or_else(|| {
                panic!("missing compatibility alias inventory entry: {expected_alias}")
            });
        for field in [
            "owner",
            "reason",
            "primary_command",
            "kept_policy",
            "removal_condition",
        ] {
            assert!(
                alias[field].as_str().is_some_and(|value| !value.is_empty()),
                "alias {expected_alias} missing non-empty {field}"
            );
        }
        assert_eq!(alias["independent_behavior"], false);
    }
    let repo_root_alias = alias_entries
        .iter()
        .find(|alias| alias["alias"] == "--repo-root")
        .unwrap();
    assert!(
        repo_root_alias["kept_policy"]
            .as_str()
            .unwrap()
            .contains("never resolves or fills project_root")
    );

    // Run generated-artifacts-status against a clean temp framework to avoid
    // validation issues with real project projection files (.claude/rules/framework.md).
    let tmp = tempdir().unwrap();
    let tmp_framework = tmp.path().join("framework");
    seed_framework_markers(&tmp_framework);
    // Write a GENERATED_ARTIFACTS.json manifest declaring configs/framework/FRAMEWORK_SURFACE_POLICY.json
    write_json(
        &tmp_framework.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [{
                "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                "generator": "true",
                "compare": "byte-for-byte"
            }]
        }),
    );
    write_text(
        &tmp_framework.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"),
        r#"{"status":"fresh"}"#,
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        tmp_framework.to_str().unwrap(),
        "--skip-generator-run",
    ]);
    assert_eq!(
        status["schema_version"],
        "framework-generated-artifacts-status-v1"
    );
    assert_eq!(
        status["manifest_status"]["mode"],
        "manifest-backed-generated-artifact-metadata-only"
    );
    assert_eq!(status["manifest_status"]["skip_generator_run"], true);
    assert_eq!(status["drift_gate"]["enabled"], true);
    assert_eq!(
        status["drift_gate"]["compare"],
        json!(["byte-for-byte", "normalized-text"])
    );
    assert!(
        status["manifest_status"]
            .get("missing_required_generated_artifacts")
            .is_none(),
        "manifest-only status must not expose missing_required_generated_artifacts"
    );
    assert!(
        status["manifest_status"]
            .get("required_generated_artifacts")
            .is_none(),
        "manifest-only status must not expose required_generated_artifacts"
    );
    let declared_paths = status["manifest_status"]["declared_generated_artifact_paths"]
        .as_array()
        .unwrap();
    assert!(!declared_paths.is_empty());
    for required in declared_paths {
        let required = required.as_str().unwrap();
        assert!(
            status["generated_artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|artifact| artifact["path"] == required
                    && artifact["drifted"].is_boolean()
                    && artifact["regenerated_exists"].is_boolean()),
            "missing generated artifact status for {required}"
        );
    }
}

#[test]
fn generated_artifacts_status_fails_when_declared_artifact_missing_on_disk() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    let artifact_root = tmp.path().join("artifacts");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [
                {
                    "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                    "generator": "sh scripts/generate-surface.sh",
                    "compare": "byte-for-byte"
                },
                {
                    "path": "skills/SKILL_ROUTING_RUNTIME.json",
                    "generator": "sh scripts/generate-surface.sh",
                    "compare": "byte-for-byte"
                }
            ]
        }),
    );
    write_text(
        &framework_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"),
        r#"{"status":"fresh"}
"#,
    );
    write_text(
        &framework_root.join("scripts/generate-surface.sh"),
        r##"mkdir -p configs/framework
	printf '%s\n' '{"status":"fresh"}' > configs/framework/FRAMEWORK_SURFACE_POLICY.json
"##,
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--skip-generator-run",
    ]);

    assert_eq!(status["ok"], false);
    let runtime = status["generated_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["path"] == "skills/SKILL_ROUTING_RUNTIME.json")
        .expect("manifest-declared runtime artifact must be reported");
    assert_eq!(runtime["exists"], false);
    assert_eq!(runtime["clean"], false);
    assert!(
        status["manifest_status"]
            .get("missing_required_generated_artifacts")
            .is_none(),
        "manifest-only status must not expose missing_required_generated_artifacts"
    );
    assert!(
        !artifact_root
            .join("generated-artifacts-drift-check")
            .exists(),
        "generated-artifacts-status should clean temporary drift-check copies"
    );
}

#[test]
fn generated_artifacts_status_fails_when_manifest_omits_checked_in_projection() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [{
                "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                "generator": "true",
                "compare": "byte-for-byte"
            }]
        }),
    );
    write_text(
        &framework_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"),
        r#"{"status":"fresh"}
"#,
    );
    write_text(
        &framework_root.join(".claude/rules/framework.md"),
        "---\ndescription: test\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: claude -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: framework-host-projection-v1 -->\n<!-- install_scope: project -->\n\nprojection\n",
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--skip-generator-run",
    ]);

    assert_eq!(status["ok"], false);
    let undeclared = status["manifest_status"]["undeclared_generated_artifacts"]
        .as_array()
        .unwrap();
    assert!(
        undeclared
            .iter()
            .any(|path| path == ".claude/rules/framework.md"),
        "expected undeclared projection, got {undeclared:?}"
    );
}

#[test]
fn generated_artifacts_status_rejects_missing_or_unsupported_manifest_schema() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    seed_framework_markers(&framework_root);

    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "generated_artifacts": []
        }),
    );
    let missing_schema = run(router_rs_command([
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]));
    assert!(!missing_schema.status.success());
    let (_, stderr) = output_text(&missing_schema);
    assert!(
        stderr.contains("invalid generated artifact manifest"),
        "unexpected stderr for missing schema: {stderr}"
    );

    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v0",
            "generated_artifacts": []
        }),
    );
    let unsupported_schema = run(router_rs_command([
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]));
    assert!(!unsupported_schema.status.success());
    let (_, stderr) = output_text(&unsupported_schema);
    assert!(
        stderr.contains("unsupported generated artifact manifest schema_version"),
        "unexpected stderr for unsupported schema: {stderr}"
    );
}

#[test]
fn generated_artifacts_status_reports_undeclared_markers_across_reverse_reference_surfaces() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    let artifact_root = tmp.path().join("artifacts");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [{
                "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                "generator": "sh scripts/generate-surface.sh",
                "compare": "byte-for-byte"
            }]
        }),
    );
    write_text(
        &framework_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"),
        r#"{"status":"fresh","marker":"generated-by-test","derived_reports":["skills/SKILL_TIERS.json"]}
"#,
    );
    write_text(
        &framework_root.join("scripts/generate-surface.sh"),
        r##"mkdir -p configs/framework
	printf '%s\n' '{"status":"fresh","marker":"generated-by-test","derived_reports":["skills/SKILL_TIERS.json"]}' > configs/framework/FRAMEWORK_SURFACE_POLICY.json
"##,
    );
    write_text(
        &framework_root.join("skills/SKILL_EXTRA.json"),
        r#"{"marker":"generated-by-test"}
"#,
    );
    write_text(
        &framework_root.join("skills/SKILL_TIERS.json"),
        r#"{"marker":"generated-by-test"}
"#,
    );
    write_text(
        &framework_root.join("docs/generated.md"),
        "generated-by-test\n",
    );
    write_text(
        &framework_root.join(".codex/generated.json"),
        r#"{"marker":"generated-by-test"}
"#,
    );
    write_text(&framework_root.join("AGENTS.md"), "generated-by-test\n");
    write_text(
        &framework_root.join("tests/source.rs"),
        r#"let fixture = "generated-by-test";"#,
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);

    assert_eq!(status["ok"], false);
    let undeclared = status["manifest_status"]["undeclared_generated_artifacts"]
        .as_array()
        .unwrap();
    for expected in [
        ".codex/generated.json",
        "AGENTS.md",
        "docs/generated.md",
        "skills/SKILL_EXTRA.json",
    ] {
        assert!(
            undeclared.contains(&json!(expected)),
            "missing undeclared generated artifact marker: {expected}; got {undeclared:?}"
        );
    }
    assert!(!undeclared.contains(&json!("tests/source.rs")));
    assert!(
        !undeclared.contains(&json!("skills/SKILL_TIERS.json")),
        "derived reports declared by FRAMEWORK_SURFACE_POLICY.json should not be flagged"
    );
}

#[test]
fn generated_artifacts_status_reports_manifest_backed_drift() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    let artifact_root = tmp.path().join("artifacts");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [{
                "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                "generator": "sh scripts/generate-surface.sh",
                "compare": "byte-for-byte"
            }]
        }),
    );
    write_text(
        &framework_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"),
        r#"{"status":"stale","marker":"generated-by-test","bad":"/Users/joe/.codex ${HOME}/Documents/skill"}
"#,
    );
    write_text(
        &framework_root.join("scripts/generate-surface.sh"),
        r##"mkdir -p configs/framework
	printf '%s\n' '{"status":"fresh","marker":"generated-by-test"}' > configs/framework/FRAMEWORK_SURFACE_POLICY.json
"##,
    );
    write_text(
        &artifact_root.join("undeclared/root/IGNORED.json"),
        r#"{"marker":"generated-by-test"}
"#,
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);

    assert_eq!(status["ok"], false);
    assert_eq!(
        status["manifest_status"]["mode"],
        "manifest-backed-generated-artifact-drift-gate"
    );
    assert_eq!(
        status["manifest_status"]["drifted_artifacts"],
        json!([{
            "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
            "generator": "sh scripts/generate-surface.sh",
            "compare": "byte-for-byte"
        }])
    );
    assert_eq!(
        status["generated_artifacts"][0]["forbidden_markers"],
        json!(["expanded-codex-home", "expanded-consuming-project-root"])
    );
    assert_eq!(
        status["manifest_status"]["undeclared_generated_artifacts"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn projection_root_resolution_fails_closed_for_missing_framework_root() {
    let tmp = tempdir().unwrap();
    let bad_framework = tmp.path().join("missing-framework");
    let project = tmp.path().join("consumer");
    std::fs::create_dir_all(&project).unwrap();
    let output = run(router_rs_command([
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        bad_framework.to_str().unwrap(),
        "--project-root",
        project.to_str().unwrap(),
    ]));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("stale or missing framework_root"));
    assert!(stderr.contains("Repair by passing --framework-root"));
}

#[test]
fn projection_root_resolution_honors_env_fallbacks_and_cli_home_overrides() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let env_codex_home = tmp.path().join("env/.codex");
    let flag_codex_home = tmp.path().join("flag/.codex");
    std::fs::create_dir_all(&project_root).unwrap();

    let mut env_status = router_rs_command(["framework", "host-integration", "status"]);
    env_status
        .env("SKILL_FRAMEWORK_ROOT", &framework_root)
        .env("SKILL_PROJECT_ROOT", &project_root)
        .env("SKILL_ARTIFACT_ROOT", &artifact_root)
        .env("CODEX_HOME", &env_codex_home);
    let env_payload = json_from_output(&run(env_status));
    assert_eq!(
        env_payload["resolved_roots"]["framework_root"],
        framework_root.to_str().unwrap()
    );
    assert_eq!(
        env_payload["resolved_roots"]["project_root"],
        project_root.to_str().unwrap()
    );
    assert_eq!(
        env_payload["resolved_roots"]["artifact_root"],
        artifact_root.to_str().unwrap()
    );
    assert_eq!(
        env_payload["resolved_roots"]["host_home_roots"]["codex"],
        env_codex_home.to_str().unwrap()
    );

    let mut flag_status = router_rs_command([
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--codex-home",
        flag_codex_home.to_str().unwrap(),
    ]);
    flag_status.env("CODEX_HOME", &env_codex_home);
    let flag_payload = json_from_output(&run(flag_status));
    assert_eq!(
        flag_payload["resolved_roots"]["host_home_roots"]["codex"],
        flag_codex_home.to_str().unwrap()
    );
}

#[test]
fn project_discovery_ignores_host_private_projection_directories() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let host_private_only = tmp.path().join("host-private-only");
    std::fs::create_dir_all(host_private_only.join(".codex/prompts")).unwrap();

    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--manifest-path",
        framework_root
            .join("core/router-rs/Cargo.toml")
            .to_str()
            .unwrap(),
        "--bin",
        "router-rs-cli",
        "--",
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]);
    command.current_dir(&host_private_only);
    let output = run(command);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing project_root"));
    assert!(stderr.contains("pass --project-root or set SKILL_PROJECT_ROOT"));
}

#[test]
fn project_discovery_rejects_ambiguous_framework_like_candidate() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let other_framework = tmp.path().join("other-framework");
    seed_framework_markers(&other_framework);
    std::fs::create_dir_all(other_framework.join(".git")).unwrap();

    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--manifest-path",
        framework_root
            .join("core/router-rs/Cargo.toml")
            .to_str()
            .unwrap(),
        "--bin",
        "router-rs-cli",
        "--",
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]);
    command.current_dir(&other_framework);
    let output = run(command);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ambiguous project_root discovery"));
    assert!(stderr.contains("Pass both --framework-root and --project-root explicitly"));
}

#[test]
fn compatibility_alias_outputs_are_normalized_equivalent() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&home).unwrap();

    let _framework_status = router_rs_json_with_home(
        &home,
        &[
            "framework",
            "host-integration",
            "status",
            "--framework-root",
            framework_root.to_str().unwrap(),
            "--project-root",
            project_root.to_str().unwrap(),
            "--home",
            home.to_str().unwrap(),
        ],
    );
    let framework_status_with_repo_root = router_rs_json_with_home(
        &home,
        &[
            "framework",
            "host-integration",
            "status",
            "--repo-root",
            framework_root.to_str().unwrap(),
            "--project-root",
            project_root.to_str().unwrap(),
            "--home",
            home.to_str().unwrap(),
        ],
    );
    assert_eq!(
        normalize_alias_equivalence(framework_status_with_repo_root),
        normalize_alias_equivalence(router_rs_json_with_home(
            &home,
            &[
                "framework",
                "host-integration",
                "status",
                "--framework-root",
                framework_root.to_str().unwrap(),
                "--project-root",
                project_root.to_str().unwrap(),
                "--home",
                home.to_str().unwrap(),
            ]
        ))
    );
}

fn normalize_alias_equivalence(mut payload: serde_json::Value) -> serde_json::Value {
    if let Some(object) = payload.as_object_mut() {
        object.remove("invocation");
        object.remove("resolved_roots");
        // Collect host keys dynamically from `host_targets` and `results` so that
        // newly-added hosts are covered without manual list maintenance.
        let host_keys: Vec<String> = {
            let mut keys = Vec::new();
            if let Some(ht) = object.get("host_targets").and_then(Value::as_object) {
                keys.extend(ht.keys().cloned());
            }
            if let Some(r) = object.get("results").and_then(Value::as_object) {
                for k in r.keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }
            keys
        };
        if let Some(host_targets) = object
            .get_mut("host_targets")
            .and_then(Value::as_object_mut)
        {
            for key in &host_keys {
                host_targets.remove(key.as_str());
            }
        }
        if let Some(results) = object.get_mut("results").and_then(Value::as_object_mut) {
            for key in &host_keys {
                results.remove(key.as_str());
            }
        }
    }
    payload
}

#[test]
fn validation_subcommands_cover_install_skills_contract() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    seed_framework_markers(&repo_root);
    let bootstrap_path = tmp.path().join("framework_default_bootstrap.json");
    host_integration_json(&[
        "ensure-default-bootstrap",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--output-dir",
        tmp.path().to_str().unwrap(),
    ]);
    let bootstrap_ok = host_integration_json(&[
        "validate-default-bootstrap",
        "--bootstrap-path",
        bootstrap_path.to_str().unwrap(),
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    let source_path = host_integration_json(&[
        "resolve-skills-source",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    assert!(bootstrap_ok["ok"].as_bool().is_some());
    assert_path_eq(
        source_path["path"].as_str().unwrap(),
        &repo_root
            .join("skills")
            .canonicalize()
            .unwrap()
            .display()
            .to_string(),
    );
}

fn assert_path_eq(left: &str, right: &str) {
    assert_eq!(
        normalize_macos_private_var(left),
        normalize_macos_private_var(right)
    );
}

fn normalize_macos_private_var(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/private/") {
        format!("/{rest}")
    } else {
        path.to_string()
    }
}
