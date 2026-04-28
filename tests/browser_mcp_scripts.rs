mod common;

use common::{
    make_executable, project_root, read_json, router_rs_command, run, write_json, write_text,
};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use tempfile::tempdir;

const SOURCE_FILES: &[&str] = &[
    "index.ts",
    "runtime.ts",
    "server.ts",
    "types.ts",
    "errors.ts",
];
const DIST_FILES: &[&str] = &[
    "index.js",
    "runtime.js",
    "server.js",
    "types.js",
    "errors.js",
];
const ROUTER_EXEC_LOG_ENV: &str = "FAKE_ROUTER_OUTPUT";

#[test]
fn resolver_prefers_newest_resume_manifest_event_transport_path() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    write_json(
        &search_root.join("older/TRACE_RESUME_MANIFEST.json"),
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/older.json",
            "updated_at": "2026-04-23T00:00:00+00:00"
        }),
    );
    let newer_manifest = search_root.join("newer/TRACE_RESUME_MANIFEST.json");
    write_json(
        &newer_manifest,
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/newer.json",
            "updated_at": "2026-04-23T00:05:00+00:00"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_path_eq(
        &stdout_trim(&completed),
        &newer_manifest.canonicalize().unwrap().display().to_string(),
    );
}

#[test]
fn resolver_falls_back_to_manifest_file_recency_when_updated_at_is_invalid() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    let older_manifest = search_root.join("older/TRACE_RESUME_MANIFEST.json");
    let newer_manifest = search_root.join("newer/TRACE_RESUME_MANIFEST.json");
    write_json(
        &older_manifest,
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/older.json",
            "updated_at": "not-a-timestamp"
        }),
    );
    write_json(
        &newer_manifest,
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/newer.json",
            "updated_at": "still-not-a-timestamp"
        }),
    );
    set_mtime(&older_manifest, 1_700_000_000);
    set_mtime(&newer_manifest, 1_700_000_100);
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_path_eq(
        &stdout_trim(&completed),
        &newer_manifest.canonicalize().unwrap().display().to_string(),
    );
}

#[test]
fn resolver_reads_sqlite_resume_manifest_payloads() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    seed_sqlite_payload(
        &search_root.join("sqlite-run/runtime_checkpoint_store.sqlite3"),
        "runtime-data/TRACE_RESUME_MANIFEST.json",
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/logical/sqlite/runtime_event_transports/session__job.json",
            "updated_at": "2026-04-23T00:10:00+00:00"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_eq!(
        stdout_trim(&completed),
        sqlite_payload_locator(&search_root, "runtime-data/TRACE_RESUME_MANIFEST.json")
    );
}

#[test]
fn resolver_falls_back_to_sqlite_payload_key_for_binding_candidates() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    seed_sqlite_payload(
        &search_root.join("sqlite-run/runtime_checkpoint_store.sqlite3"),
        "runtime-data/runtime_event_transports/sqlite-session.json",
        &json!({
            "schema_version": "runtime-event-transport-v1",
            "binding_backend_family": "sqlite"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_eq!(
        stdout_trim(&completed),
        sqlite_payload_locator(
            &search_root,
            "runtime-data/runtime_event_transports/sqlite-session.json"
        )
    );
}

#[test]
fn resolver_falls_back_to_binding_artifact_when_manifest_is_missing() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    let binding_path = search_root.join("run-a/data/runtime_event_transports/session__job.json");
    write_json(
        &binding_path,
        &json!({
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": binding_path,
            "binding_backend_family": "sqlite"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_eq!(stdout_trim(&completed), binding_path.display().to_string());
}

#[test]
fn resolver_prefers_manifest_candidates_over_binding_candidates_when_manifest_has_timestamp() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    let manifest_path = search_root.join("manifest/TRACE_RESUME_MANIFEST.json");
    let binding_path = search_root.join("binding/data/runtime_event_transports/session__job.json");
    write_json(
        &manifest_path,
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/from-manifest.json",
            "updated_at": "2026-04-23T00:00:01+00:00"
        }),
    );
    write_json(
        &binding_path,
        &json!({
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": "/tmp/runtime_event_transports/from-binding.json",
            "binding_backend_family": "filesystem"
        }),
    );
    set_mtime(&binding_path, 1_800_000_000);
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_path_eq(
        &stdout_trim(&completed),
        &manifest_path.canonicalize().unwrap().display().to_string(),
    );
}

#[test]
fn resolver_ignores_invalid_payloads_and_keeps_valid_binding_fallback() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    write_text(
        &search_root.join("broken/TRACE_RESUME_MANIFEST.json"),
        "{not-json\n",
    );
    write_json(
        &search_root.join("missing-path/TRACE_RESUME_MANIFEST.json"),
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "updated_at": "2026-04-23T00:20:00+00:00"
        }),
    );
    let binding_path = search_root.join("run-b/data/runtime_event_transports/session__job.json");
    write_json(
        &binding_path,
        &json!({
            "schema_version": "runtime-event-transport-v1",
            "binding_backend_family": "sqlite"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_eq!(stdout_trim(&completed), binding_path.display().to_string());
}

#[test]
fn resolver_reads_sqlite_binding_payload_without_explicit_binding_path() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    seed_sqlite_payload(
        &search_root.join("sqlite-run/runtime_checkpoint_store.sqlite3"),
        "runtime-data/runtime_event_transports/session__job.json",
        &json!({
            "schema_version": "runtime-event-transport-v1",
            "binding_backend_family": "sqlite"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_eq!(
        stdout_trim(&completed),
        sqlite_payload_locator(
            &search_root,
            "runtime-data/runtime_event_transports/session__job.json"
        )
    );
}

#[test]
fn resolver_returns_none_when_no_attach_candidates_exist() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    fs::create_dir_all(&search_root).unwrap();
    let completed = run_resolver_cli(&search_root);
    assert_eq!(completed.status.code(), Some(1));
    assert!(completed.stdout.is_empty());
}

#[test]
fn resolver_ignores_sqlite_query_failures_and_uses_filesystem_fallback() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    write_text(
        &search_root.join("sqlite-run/runtime_checkpoint_store.sqlite3"),
        "not-a-real-sqlite-db",
    );
    let binding_path = search_root.join("good/data/runtime_event_transports/session__job.json");
    write_json(
        &binding_path,
        &json!({
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": binding_path,
            "binding_backend_family": "sqlite"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_eq!(stdout_trim(&completed), binding_path.display().to_string());
}

#[test]
fn resolver_cli_prints_resolved_attach_path_on_success() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    let binding_path = search_root.join("run-a/data/runtime_event_transports/session__job.json");
    write_json(
        &binding_path,
        &json!({
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": binding_path,
            "binding_backend_family": "sqlite"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert!(completed.status.success());
    assert_eq!(stdout_trim(&completed), binding_path.display().to_string());
    let stderr = String::from_utf8_lossy(&completed.stderr);
    assert!(
        stderr.is_empty() || stderr.contains("Compiling") || stderr.contains("Finished"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn resolver_ignores_bare_filesystem_binding_without_replay_manifest() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    let binding_path = search_root.join("run-a/data/runtime_event_transports/session__job.json");
    write_json(
        &binding_path,
        &json!({
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": binding_path,
            "binding_backend_family": "filesystem"
        }),
    );
    let completed = run_resolver_cli(&search_root);
    assert_eq!(completed.status.code(), Some(1));
    assert!(completed.stdout.is_empty());
}

#[test]
fn resolver_cli_exits_nonzero_when_no_candidates_exist() {
    let tmp = tempdir().unwrap();
    let search_root = tmp.path().join("scratch");
    fs::create_dir_all(&search_root).unwrap();
    let completed = run_resolver_cli(&search_root);
    assert_eq!(completed.status.code(), Some(1));
    assert!(completed.stdout.is_empty());
}

#[test]
fn launcher_prefers_explicit_attach_descriptor_env_over_auto_discovery() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    write_json(
        &repo_root.join("framework_runtime/artifacts/scratch/older/TRACE_RESUME_MANIFEST.json"),
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/auto/discovered/runtime_event_transports/older.json",
            "updated_at": "2026-04-23T00:00:00+00:00"
        }),
    );
    let result = run_launcher(
        &repo_root,
        &[(
            "BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH",
            "/explicit/descriptor.json",
        )],
        &[],
    );
    assert_eq!(
        normalize_macos_private_var(result["cwd"].as_str().unwrap()),
        normalize_macos_private_var(&repo_root.display().to_string())
    );
    assert_eq!(
        result["argv"],
        json!([
            "browser",
            "mcp-stdio",
            "--repo-root",
            repo_root.display().to_string(),
            "--runtime-attach-descriptor-path",
            "/explicit/descriptor.json"
        ])
    );
}

#[test]
fn launcher_leaves_sqlite_attach_discovery_to_rust_runtime() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    seed_sqlite_payload(
        &repo_root.join(
            "framework_runtime/artifacts/scratch/sqlite-run/runtime_checkpoint_store.sqlite3",
        ),
        "runtime-data/TRACE_RESUME_MANIFEST.json",
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/logical/sqlite/runtime_event_transports/session__job.json",
            "updated_at": "2026-04-23T00:10:00+00:00"
        }),
    );
    let result = run_launcher(&repo_root, &[], &[]);
    assert_eq!(
        result["argv"],
        json!([
            "browser",
            "mcp-stdio",
            "--repo-root",
            repo_root.display().to_string()
        ])
    );
}

#[test]
fn browser_mcp_stdio_exposes_repository_skill_router_tools() {
    let repo_root = project_root();
    let request = [
        serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }))
        .unwrap(),
        serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "skill_route",
                "arguments": {"query": "路由系统触发稳定吗"}
            }
        }))
        .unwrap(),
        serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "skill_read",
                "arguments": {"skill": "skill-framework-developer", "maxChars": 2000}
            }
        }))
        .unwrap(),
    ]
    .join("\n");
    let mut command = router_rs_command([
        "browser",
        "mcp-stdio",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    command.stdin(Stdio::piped()).stdout(Stdio::piped());
    let mut child = command.spawn().unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(request.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    common::assert_success(&output);
    let payloads = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let tool_names = payloads[0]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool.get("name").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"skill_route"));
    assert!(tool_names.contains(&"skill_search"));
    assert!(tool_names.contains(&"skill_read"));
    assert_eq!(
        payloads[1]["result"]["structuredContent"]["decision"]["selected_skill"],
        "skill-framework-developer"
    );
    assert!(payloads[2]["result"]["structuredContent"]["content"]
        .as_str()
        .unwrap()
        .contains("# skill-framework-developer"));
}

#[test]
fn launcher_leaves_filesystem_attach_discovery_to_rust_runtime() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    let manifest_path =
        repo_root.join("framework_runtime/artifacts/scratch/run-a/TRACE_RESUME_MANIFEST.json");
    write_json(
        &manifest_path,
        &json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/auto/discovered/runtime_event_transports/session__job.json",
            "updated_at": "2026-04-23T00:10:00+00:00"
        }),
    );
    let result = run_launcher(&repo_root, &[], &[]);
    assert_eq!(
        result["argv"],
        json!([
            "browser",
            "mcp-stdio",
            "--repo-root",
            repo_root.display().to_string()
        ])
    );
}

#[test]
fn launcher_uses_canonical_attach_artifact_env() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    let result = run_launcher(
        &repo_root,
        &[(
            "BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH",
            "/explicit/attach-artifact.json",
        )],
        &[],
    );
    assert_eq!(
        result["argv"],
        json!([
            "browser",
            "mcp-stdio",
            "--repo-root",
            repo_root.display().to_string(),
            "--runtime-attach-artifact-path",
            "/explicit/attach-artifact.json"
        ])
    );
}

#[test]
fn launcher_prefers_descriptor_env_over_attach_artifact_env() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    let result = run_launcher(
        &repo_root,
        &[
            (
                "BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH",
                "/explicit/descriptor.json",
            ),
            (
                "BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH",
                "/explicit/attach-artifact.json",
            ),
        ],
        &[],
    );
    assert_eq!(
        result["argv"],
        json!([
            "browser",
            "mcp-stdio",
            "--repo-root",
            repo_root.display().to_string(),
            "--runtime-attach-descriptor-path",
            "/explicit/descriptor.json"
        ])
    );
}

#[test]
fn launcher_falls_back_to_plain_start_when_no_attach_input_exists() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    let result = run_launcher(&repo_root, &[], &["--headless", "false"]);
    assert_eq!(
        result["argv"],
        json!([
            "browser",
            "mcp-stdio",
            "--repo-root",
            repo_root.display().to_string(),
            "--headless",
            "false"
        ])
    );
}

#[test]
fn launcher_builds_router_rs_when_no_prebuilt_binary_exists() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    fs::remove_file(repo_root.join("scripts/router-rs/target/release/router-rs")).unwrap();
    let cargo_log = repo_root.join("fake-cargo-output.json");
    let fake_cargo = install_fake_cargo_builder(&repo_root, &cargo_log);
    let mut command =
        Command::new(repo_root.join("tools/browser-mcp/scripts/start_browser_mcp.sh"));
    command
        .current_dir(&repo_root)
        .env_remove("BROWSER_MCP_ROUTER_RS_BIN")
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_cargo.parent().unwrap().display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("CARGO_TARGET_DIR", repo_root.join("shared-target"));
    let result = run(command);
    assert!(
        result.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        read_json(&cargo_log)["argv"],
        json!(["browser", "mcp-stdio", "--repo-root", repo_root])
    );
}

#[test]
fn launcher_rebuilds_router_rs_when_sources_are_newer_than_binary() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    let router_path = repo_root.join("scripts/router-rs/target/release/router-rs");
    set_mtime(&router_path, 1_700_000_000);
    write_text(
        &repo_root.join("scripts/router-rs/src/main.rs"),
        "// fresh source\n",
    );
    let cargo_log = repo_root.join("fresh-cargo-output.json");
    let fake_cargo = install_fake_cargo_builder(&repo_root, &cargo_log);

    let mut command =
        Command::new(repo_root.join("tools/browser-mcp/scripts/start_browser_mcp.sh"));
    command
        .current_dir(&repo_root)
        .env_remove("BROWSER_MCP_ROUTER_RS_BIN")
        .env_remove(ROUTER_EXEC_LOG_ENV)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_cargo.parent().unwrap().display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("CARGO_TARGET_DIR", repo_root.join("shared-target"));

    common::assert_success(&run(command));
    assert_eq!(
        read_json(&cargo_log)["argv"],
        json!(["browser", "mcp-stdio", "--repo-root", repo_root])
    );
}

#[test]
fn launcher_never_falls_back_to_node_runtime() {
    let tmp = tempdir().unwrap();
    let repo_root = prepare_repo(tmp.path());
    fs::remove_file(repo_root.join("scripts/router-rs/target/release/router-rs")).unwrap();
    fs::write(
        repo_root.join("scripts/router-rs/run_router_rs.sh"),
        "#!/bin/sh\necho rust-launcher-only > \"$ROUTER_EXEC_LOG\"\nexit 7\n",
    )
    .unwrap();
    make_executable(&repo_root.join("scripts/router-rs/run_router_rs.sh"));

    let output_path = repo_root.join("launcher-output.txt");
    let mut command =
        Command::new(repo_root.join("tools/browser-mcp/scripts/start_browser_mcp.sh"));
    command
        .current_dir(&repo_root)
        .env_remove("BROWSER_MCP_ROUTER_RS_BIN")
        .env("ROUTER_EXEC_LOG", &output_path);

    let result = run(command);
    assert_eq!(result.status.code(), Some(7));
    assert_eq!(
        fs::read_to_string(output_path).unwrap().trim(),
        "rust-launcher-only"
    );
    let node_entrypoint = ["dist", "index.js"].join("/");
    assert!(!String::from_utf8_lossy(&result.stderr).contains(&node_entrypoint));
}

fn run_resolver_cli(search_root: &std::path::Path) -> Output {
    let repo_root = project_root();
    run(router_rs_command([
        "browser",
        "resolve-attach-artifact",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--search-root",
        search_root.to_str().unwrap(),
    ]))
}

fn sqlite_payload_locator(search_root: &std::path::Path, payload_key: &str) -> String {
    search_root
        .join("sqlite-run")
        .join(payload_key)
        .display()
        .to_string()
}

fn prepare_repo(tmp_path: &std::path::Path) -> std::path::PathBuf {
    let repo_root = tmp_path.join("repo");
    let script_root = repo_root.join("tools/browser-mcp/scripts");
    fs::create_dir_all(&script_root).unwrap();
    fs::copy(
        project_root().join("tools/browser-mcp/scripts/start_browser_mcp.sh"),
        script_root.join("start_browser_mcp.sh"),
    )
    .unwrap();
    make_executable(&script_root.join("start_browser_mcp.sh"));
    fs::create_dir_all(repo_root.join("scripts/router-rs")).unwrap();
    fs::copy(
        project_root().join("scripts/router-rs/run_router_rs.sh"),
        repo_root.join("scripts/router-rs/run_router_rs.sh"),
    )
    .unwrap();
    make_executable(&repo_root.join("scripts/router-rs/run_router_rs.sh"));
    install_fake_router(&repo_root);
    for name in SOURCE_FILES {
        write_text(
            &repo_root.join("tools/browser-mcp/src").join(name),
            &format!("// {name}\n"),
        );
    }
    for name in DIST_FILES {
        write_text(
            &repo_root.join("tools/browser-mcp/dist").join(name),
            &format!("// built {name}\n"),
        );
    }
    repo_root
}

fn run_launcher(
    repo_root: &std::path::Path,
    envs: &[(&str, &str)],
    extra_args: &[&str],
) -> serde_json::Value {
    let output_path = repo_root.join("fake-router-output.json");
    let mut command =
        Command::new(repo_root.join("tools/browser-mcp/scripts/start_browser_mcp.sh"));
    command
        .current_dir(repo_root)
        .env(
            "BROWSER_MCP_ROUTER_RS_BIN",
            repo_root
                .join("scripts/router-rs/target/release/router-rs")
                .display()
                .to_string(),
        )
        .env(ROUTER_EXEC_LOG_ENV, &output_path)
        .args(extra_args);
    for (key, value) in envs {
        command.env(key, value);
    }
    common::assert_success(&run(command));
    read_json(&output_path)
}

fn install_fake_router(repo_root: &Path) {
    let router_path = repo_root.join("scripts/router-rs/target/release/router-rs");
    write_text(
        &router_path,
        &format!(
            r#"#!/bin/sh
printf '{{"argv":[' > "${env_key}"
first=1
for arg in "$@"; do
  if [ "$first" = 0 ]; then printf ',' >> "${env_key}"; fi
  first=0
  escaped=$(printf '%s' "$arg" | sed 's/\\/\\\\/g; s/"/\\"/g')
  printf '"%s"' "$escaped" >> "${env_key}"
done
printf '],"cwd":"%s"}}\n' "$(pwd | sed 's/\\/\\\\/g; s/"/\\"/g')" >> "${env_key}"
"#,
            env_key = ROUTER_EXEC_LOG_ENV
        ),
    );
    make_executable(&router_path);
}

fn install_fake_cargo_builder(repo_root: &Path, cargo_log: &Path) -> std::path::PathBuf {
    let fake_cargo = repo_root.join("fake-bin/cargo");
    write_text(
        &fake_cargo,
        &format!(
            r#"#!/bin/sh
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/router-rs" <<'SH'
#!/bin/sh
printf '{{"argv":[' > "{cargo_log}"
first=1
for arg in "$@"; do
  if [ "$first" = 0 ]; then printf ',' >> "{cargo_log}"; fi
  first=0
  escaped=$(printf '%s' "$arg" | sed 's/\\/\\\\/g; s/"/\\"/g')
  printf '"%s"' "$escaped" >> "{cargo_log}"
done
printf ']}}\n' >> "{cargo_log}"
SH
chmod +x "$CARGO_TARGET_DIR/release/router-rs"
"#,
            cargo_log = cargo_log.display()
        ),
    );
    make_executable(&fake_cargo);
    fake_cargo
}

fn seed_sqlite_payload(db_path: &std::path::Path, payload_key: &str, payload: &serde_json::Value) {
    fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    let connection = Connection::open(db_path).unwrap();
    connection
        .execute(
            "CREATE TABLE runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?1, ?2)",
            (payload_key, payload.to_string()),
        )
        .unwrap();
}

fn set_mtime(path: &std::path::Path, epoch: i64) {
    let status = Command::new("touch")
        .args(["-t", &format_epoch_for_touch(epoch)])
        .arg(path)
        .status()
        .expect("failed to run touch");
    assert!(status.success());
}

fn format_epoch_for_touch(epoch: i64) -> String {
    let output = Command::new("date")
        .args(["-r", &epoch.to_string(), "+%Y%m%d%H%M.%S"])
        .output()
        .expect("failed to run date");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn stdout_trim(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
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
