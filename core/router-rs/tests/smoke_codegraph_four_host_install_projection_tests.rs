//! CG deferred: four-host install projection smoke — each closed-set host must
//! materialize `mcp-codegraph` in its projected MCP config after `install_projection_tool`.

#[cfg(feature = "codegraph")]
mod four_host_install_projection {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;
    use serial_test::serial;

    use crate::framework_host_targets::{
        host_targets_supported_host_ids, skills_install_tool_for_host_id,
    };
    use crate::host_integration::{
        ResolvedProjectionRoots, install_projection_tool, projection_scope_for_tool,
    };
    use crate::runtime_registry::load_runtime_registry_json;

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
        let root = unique_test_root("cg-five-host-projection");
        let home = root.join("home");
        let project_root = root.join("project");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(home.join(".local/bin")).expect("local bin");
        write_stub(&home.join(".local/bin/router-rs"), "#!/bin/sh\nexit 0\n");

        let roots = ResolvedProjectionRoots {
            framework_root: framework_root.to_path_buf(),
            project_root: project_root.clone(),
            artifact_root: project_root.join("artifacts"),
            account_home_root: home.clone(),
            host_home_roots: framework_kernel::runtime_registry::ALL_HOST_IDS
                .iter()
                .map(|id| (id.to_string(), home.join(format!(".{id}"))))
                .collect(),
        };
        (root, roots)
    }

    fn read_json(path: &Path) -> Value {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("missing projected config {}: {err}", path.display()));
        serde_json::from_str(&text).expect("parse json")
    }

    fn assert_mcp_servers_codegraph(payload: &Value, host_id: &str) {
        let entry = payload
            .get("mcp_servers")
            .and_then(|v| v.get("mcp-codegraph"))
            .unwrap_or_else(|| panic!("{host_id}: mcp_servers.mcp-codegraph missing"));
        assert_eq!(entry.get("type").and_then(Value::as_str), Some("stdio"));
        let args = entry
            .get("args")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{host_id}: mcp-codegraph args"));
        assert!(
            args.iter().any(|v| v.as_str() == Some("--repo-root")),
            "{host_id}: mcp-codegraph must pass --repo-root"
        );
    }

    fn assert_mcp_servers_camel_codegraph(payload: &Value, host_id: &str) {
        let entry = payload
            .get("mcpServers")
            .and_then(|v| v.get("mcp-codegraph"))
            .unwrap_or_else(|| panic!("{host_id}: mcpServers.mcp-codegraph missing"));
        assert_eq!(entry.get("type").and_then(Value::as_str), Some("stdio"));
        let args = entry
            .get("args")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{host_id}: mcp-codegraph args"));
        assert!(
            args.iter().any(|v| v.as_str() == Some("--repo-root")),
            "{host_id}: mcp-codegraph must pass --repo-root"
        );
    }

    fn assert_host_projected_codegraph(roots: &ResolvedProjectionRoots, host_id: &str) {
        match host_id {
            "cursor" => {
                let cursor_home = roots.host_home_root("cursor")
                    .unwrap_or_else(|| panic!("cursor host home not found"));
                let path = cursor_home.join("mcp.json");
                assert!(path.is_file(), "cursor user mcp.json must exist");
                assert_mcp_servers_codegraph(&read_json(&path), host_id);
            }
            "claude" => {
                let path = roots.project_root.join(".mcp.json");
                assert!(path.is_file(), "claude project .mcp.json must exist");
                assert_mcp_servers_camel_codegraph(&read_json(&path), host_id);
            }
            "codex" => {
                let path = roots.project_root.join(".codex/config.toml");
                assert!(path.is_file(), "codex config.toml must exist");
                let text = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("read codex config.toml: {e}"));
                assert!(
                    text.contains("[mcp_servers.mcp-codegraph]"),
                    "codex config.toml must include mcp-codegraph section"
                );
                assert!(
                    text.contains("--repo-root"),
                    "codex mcp-codegraph section must pass --repo-root"
                );
            }
            "opencode" => {
                let path = roots.project_root.join(".opencode/opencode.json");
                assert!(path.is_file(), "opencode project opencode.json must exist");
                assert_mcp_servers_camel_codegraph(&read_json(&path), host_id);
            }
            other => panic!("unexpected host_id {other}"),
        }
    }

    #[serial]
    #[test]
    fn codegraph_four_host_install_projection_smoke() {
        let framework_root = framework_repo_root();
        let registry = load_runtime_registry_json(&framework_root).expect("load RUNTIME_REGISTRY");
        let host_ids = host_targets_supported_host_ids(&registry).expect("supported host ids");
        assert_eq!(
            host_ids.len(),
            4,
            "closed-set must remain four hosts (codex, claude, cursor, opencode)"
        );

        let (cleanup_root, roots) = test_roots(&framework_root);
        let prior_home = std::env::var_os("HOME");
        unsafe { core_state_utils::env_sync::set_env("HOME", &roots.account_home_root) };

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
            assert_host_projected_codegraph(&roots, host_id);
        }

        if let Some(h) = prior_home {
            unsafe { core_state_utils::env_sync::set_env("HOME", &h) };
        } else {
            unsafe { core_state_utils::env_sync::remove_env("HOME") };
        }
        let _ = fs::remove_dir_all(cleanup_root);
    }
}
