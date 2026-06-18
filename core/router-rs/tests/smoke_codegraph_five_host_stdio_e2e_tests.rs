//! CG deferred: five-host install projection + real `mcp-codegraph` stdio subprocess.
//!
//! Opt-in: `ROUTER_RS_CODEGRAPH_STDIO_E2E=1` (spawns real binary; may build `mcp-codegraph` first).

#[cfg(feature = "codegraph")]
mod five_host_stdio_e2e {
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use serde_json::Value;
    use serial_test::serial;

    use crate::framework_host_targets::{
        host_targets_supported_host_ids, skills_install_tool_for_host_id,
    };
    use crate::host_integration::{
        ResolvedProjectionRoots, install_projection_tool, projection_scope_for_tool,
    };
    use crate::runtime_registry::load_runtime_registry_json;

    const CODEGRAPH_TOOLS: &[&str] = &[
        "codegraph_search",
        "codegraph_callers",
        "codegraph_callees",
        "codegraph_impact",
        "codegraph_node",
        "codegraph_status",
    ];

    fn stdio_e2e_enabled() -> bool {
        std::env::var("ROUTER_RS_CODEGRAPH_STDIO_E2E")
            .ok()
            .as_deref()
            == Some("1")
    }

    fn framework_repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn unique_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "router-rs-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn write_stub(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write stub");
    }

    fn test_roots(framework_root: &Path) -> (PathBuf, ResolvedProjectionRoots) {
        let root = unique_test_root("cg-five-host-stdio-e2e");
        let home = root.join("home");
        let project_root = root.join("project");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(home.join(".local/bin")).expect("local bin");
        write_stub(&home.join(".local/bin/router-rs"), "#!/bin/sh\nexit 0\n");
        write_stub(
            &project_root.join("anchor.rs"),
            "fn cg_stdio_e2e_anchor() {}\n",
        );

        let roots = ResolvedProjectionRoots {
            framework_root: framework_root.to_path_buf(),
            project_root: project_root.clone(),
            artifact_root: project_root.join("artifacts"),
            account_home_root: home.clone(),
            host_home_roots: [
                ("codex".into(), home.join(".codex")),
                ("cursor".into(), home.join(".cursor")),
                ("claude".into(), home.join(".claude")),
                ("opencode".into(), home.join(".opencode")),
            ]
            .into_iter()
            .collect(),
        };
        (root, roots)
    }

    fn read_json(path: &Path) -> Value {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("missing projected config {}: {err}", path.display()));
        serde_json::from_str(&text).expect("parse json")
    }

    /// Read command and args from codex TOML config for a given MCP server section.
    fn extract_codex_mcp_config(path: &Path, server_id: &str) -> (String, Vec<String>) {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("missing codex config {}: {err}", path.display()));
        let section_header = format!("[mcp_servers.{}]", server_id);
        let section_start = text.find(&section_header)
            .unwrap_or_else(|| panic!("section {section_header} not found in {}", path.display()));
        let section = &text[section_start..];
        let section_end = section.find("\n# managed_by:").unwrap_or(section.len());
        let section_body = &section[..section_end];

        let command = section_body.lines()
            .find_map(|l| l.strip_prefix("command = "))
            .map(|v| v.trim_matches('"').to_string())
            .unwrap_or_else(|| panic!("{server_id}: command not found in {}", path.display()));
        let args_line = section_body.lines()
            .find_map(|l| l.strip_prefix("args = "))
            .unwrap_or_default();
        let args: Vec<String> = serde_json::from_str(args_line)
            .unwrap_or_else(|_| Vec::new());
        (command, args)
    }

    fn projected_codegraph_config(
        roots: &ResolvedProjectionRoots,
        host_id: &str,
    ) -> (PathBuf, &'static str) {
        match host_id {
            "cursor" => {
                let cursor_home = roots.host_home_root("cursor")
                    .unwrap_or_else(|| panic!("cursor host home not found"));
                (cursor_home.join("mcp.json"), "mcp_servers")
            }
            "claude" => (roots.project_root.join(".mcp.json"), "mcpServers"),
            "codex" => (
                roots.project_root.join(".codex/config.toml"),
                "mcp_servers",
            ),
            "opencode" => (
                roots.project_root.join(".opencode/opencode.json"),
                "mcpServers",
            ),
            other => panic!("unexpected host_id {other}"),
        }
    }

    fn extract_stdio_launch(entry: &Value, host_id: &str) -> (String, Vec<String>) {
        let command = entry
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{host_id}: mcp-codegraph command missing"));
        let args = entry
            .get("args")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| panic!("{host_id}: mcp-codegraph args missing"));
        assert!(
            args.iter().any(|a| a == "--repo-root"),
            "{host_id}: projected mcp-codegraph must pass --repo-root"
        );
        (command.to_string(), args)
    }

    fn mcp_codegraph_candidate_paths(framework_root: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Ok(raw) = std::env::var("CARGO_BIN_EXE_mcp-codegraph") {
            paths.push(PathBuf::from(raw));
        }
        if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
            let base = PathBuf::from(td);
            paths.push(base.join("debug/mcp-codegraph"));
            paths.push(base.join("release/mcp-codegraph"));
        }
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for base in [
            manifest.join("../../target"),
            manifest.join("../target"),
            PathBuf::from("/tmp/skill-cargo-cg"),
            PathBuf::from("/tmp/skill-cargo-target"),
            framework_root.join("target"),
        ] {
            paths.push(base.join("debug/mcp-codegraph"));
            paths.push(base.join("release/mcp-codegraph"));
        }
        paths
    }

    fn ensure_mcp_codegraph_built(framework_root: &Path) -> PathBuf {
        for candidate in mcp_codegraph_candidate_paths(framework_root) {
            if candidate.is_file() {
                return candidate;
            }
        }
        let status = Command::new("cargo")
            .current_dir(framework_root)
            .args([
                "build",
                "--quiet",
                "-p",
                "codegraph-rs",
                "--bin",
                "mcp-codegraph",
            ])
            .status()
            .expect("cargo build mcp-codegraph");
        assert!(
            status.success(),
            "failed to build mcp-codegraph; run `cargo build -p codegraph-rs --bin mcp-codegraph`"
        );
        for candidate in mcp_codegraph_candidate_paths(framework_root) {
            if candidate.is_file() {
                return candidate;
            }
        }
        panic!("mcp-codegraph binary not found after cargo build");
    }

    fn run_stdio_tools_probe(command: &str, args: &[String], host_id: &str) -> String {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|err| panic!("{host_id}: spawn mcp-codegraph ({command}): {err}"));

        let stdin = child.stdin.as_mut().expect("stdin");
        let requests = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"cg-five-host-e2e","version":"1.0"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codegraph_status","arguments":{}}}"#,
        ];
        for line in requests {
            writeln!(stdin, "{line}").expect("write stdin");
        }
        drop(child.stdin.take());

        let output = child
            .wait_with_output()
            .unwrap_or_else(|err| panic!("{host_id}: wait mcp-codegraph: {err}"));
        assert!(
            output.status.success(),
            "{host_id}: mcp-codegraph exited {:?}; stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn assert_codegraph_tools_visible(stdout: &str, host_id: &str) {
        for tool in CODEGRAPH_TOOLS {
            assert!(
                stdout.contains(tool),
                "{host_id}: tools/list or tools/call missing {tool}; stdout={stdout}"
            );
        }
        assert!(
            stdout.contains("node_count") || stdout.contains("stats"),
            "{host_id}: codegraph_status response missing stats; stdout={stdout}"
        );
    }

    #[serial]
    #[test]
    fn codegraph_five_host_stdio_e2e_smoke() {
        if !stdio_e2e_enabled() {
            eprintln!(
                "skip codegraph_five_host_stdio_e2e_smoke: set ROUTER_RS_CODEGRAPH_STDIO_E2E=1"
            );
            return;
        }

        let framework_root = framework_repo_root();
        let _built_bin = ensure_mcp_codegraph_built(&framework_root);

        let registry = load_runtime_registry_json(&framework_root).expect("load RUNTIME_REGISTRY");
        let host_ids = host_targets_supported_host_ids(&registry).expect("supported host ids");
        assert_eq!(host_ids.len(), 5, "closed-set must remain five hosts");

        let (cleanup_root, roots) = test_roots(&framework_root);
        let prior_home = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", &roots.account_home_root) };

        for host_id in &host_ids {
            let tool = skills_install_tool_for_host_id(&registry, host_id).expect("install tool");
            let scope = projection_scope_for_tool(&tool, "project").expect("scope");
            let result = install_projection_tool(&roots, &tool, scope)
                .unwrap_or_else(|err| panic!("install {host_id} ({tool}): {err}"));
            assert_eq!(
                result.get("status").and_then(Value::as_str),
                Some("installed"),
                "{host_id} install status"
            );

            let (config_path, servers_key) = projected_codegraph_config(&roots, host_id);
            let (command, args) = if host_id == "codex" {
                extract_codex_mcp_config(&config_path, "mcp-codegraph")
            } else {
                let payload = read_json(&config_path);
                let entry = payload
                    .get(servers_key)
                    .and_then(|v| v.get("mcp-codegraph"))
                    .unwrap_or_else(|| {
                        panic!(
                            "{host_id}: {servers_key}.mcp-codegraph missing in {}",
                            config_path.display()
                        )
                    });
                extract_stdio_launch(entry, host_id)
            };
            let stdout = run_stdio_tools_probe(&command, &args, host_id);
            assert_codegraph_tools_visible(&stdout, host_id);
        }

        if let Some(h) = prior_home {
            unsafe { std::env::set_var("HOME", h) };
        } else {
            unsafe { std::env::remove_var("HOME") };
        }
        let _ = fs::remove_dir_all(cleanup_root);
    }
}
