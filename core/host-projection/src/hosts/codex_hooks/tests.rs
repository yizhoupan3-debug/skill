use super::*;
use router_rs::goal_state::ARTIFACTS_CURRENT_DIR;
use router_rs::hook_common::read_limited_stdin;
use serde_json::json;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn codex_first_nonempty_prompt_line_skips_leading_blank_lines() {
    assert_eq!(
        super::codex_first_nonempty_prompt_line("\n  \nreal task\nmore"),
        "real task"
    );
}

#[test]
fn protected_generated_paths_match_lexical_variants() {
    assert_eq!(normalize_repo_relative_path("./AGENTS.md"), "AGENTS.md");
    assert_eq!(
        normalize_repo_relative_path(".codex/../.codex/host_entrypoints_sync_manifest.json"),
        ".codex/host_entrypoints_sync_manifest.json"
    );
    assert!(router_rs::hook_common::path_guard::classify_protected_path(
        "./AGENTS.md",
        None,
        None,
        None
    )
    .is_some());
    assert!(router_rs::hook_common::path_guard::classify_protected_path(
        ".codex/../.codex/host_entrypoints_sync_manifest.json",
        None,
        None,
        None
    )
    .is_some());
    assert!(router_rs::hook_common::path_guard::classify_protected_path(
        "./.codex/prompts/gitx.md",
        None,
        None,
        None
    )
    .is_none());
}

#[test]
fn pre_tool_use_blocks_normalized_direct_paths() {
    let payload = json!({"tool_input": {"file_path": "./AGENTS.md"}});
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
    let payload = json!({"tool_input": {"file_path": ".codex/../.codex/host_entrypoints_sync_manifest.json"}});
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
    let payload = json!({"tool_input": {"file_path": ".codex/../.codex/prompts/autopilot.md"}});
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_none());
}

#[test]
fn pre_tool_use_blocks_normalized_bash_write_targets() {
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "printf x > ./AGENTS.md"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "printf x | tee .codex/../.codex/host_entrypoints_sync_manifest.json"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "printf x | tee .codex/prompts/gitx.md"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_none());

    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "printf x >| ./AGENTS.md"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
}

#[test]
fn pre_tool_use_allows_read_only_bash_commands_on_protected_paths() {
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cat ./AGENTS.md"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_none());

    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "rg contract_digest .codex/host_entrypoints_sync_manifest.json"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_none());
}

mod install_codex_hooks_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::SystemTime;

    static INSTALL_SEQ: AtomicU64 = AtomicU64::new(0);

    fn fresh_path(label: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "install-codex-hooks-{}-{}-{}",
            label,
            std::process::id(),
            INSTALL_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn run_install(codex_home: &Path, repo_root: &Path, mode: InstallMode) -> Value {
        install_codex_hooks(codex_home, repo_root, mode).unwrap()
    }

    fn install_hook_commands(repo_root: &Path) -> BTreeMap<String, String> {
        INSTALL_EVENTS
            .iter()
            .map(|event| {
                (
                    (*event).to_string(),
                    build_install_hook_command(repo_root, event),
                )
            })
            .collect()
    }

    #[test]
    fn empty_codex_home_creates_config_and_hooks() {
        let root = fresh_path("empty");
        let codex_home = root.join("new-codex-home");
        let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let config_path = codex_home.join("config.toml");
        let hooks_path = codex_home.join("hooks.json");
        assert!(config_path.exists());
        assert!(hooks_path.exists());
        assert_eq!(payload["config_toml"]["status"].as_str(), Some("created"));
        assert_eq!(payload["hooks_json"]["status"].as_str(), Some("created"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_config_with_features_block_preserves_other_keys() {
        let root = fresh_path("features-preserve");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            "[features]\nother_flag = true\n",
        )
        .unwrap();
        run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
        assert!(text.contains("other_flag = true"));
        assert!(text.contains("hooks = true"));
        assert!(!text.contains("codex_hooks"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_config_with_codex_hooks_false_under_features_replaces() {
        let root = fresh_path("replace");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            "[features]\ncodex_hooks = false\n",
        )
        .unwrap();
        run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
        assert_eq!(text, "[features]\nhooks = true\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_config_with_codex_hooks_under_other_section_untouched() {
        let root = fresh_path("other-section");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            "[custom]\ncodex_hooks = false\n[features]\nother = 1\n",
        )
        .unwrap();
        run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
        assert!(text.contains("[custom]\ncodex_hooks = false"));
        assert!(text.contains("[features]\nother = 1\nhooks = true"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn config_without_features_appends_section() {
        let root = fresh_path("append-features");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "[custom]\nvalue = 1\n").unwrap();
        run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
        assert!(text.ends_with("[features]\nhooks = true\n"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_hooks_json_preserves_existing_entry() {
        let root = fresh_path("preserve-hooks");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
        fs::write(
            codex_home.join("hooks.json"),
            "{\n  \"hooks\": {\n    \"Stop\": [\n      {\"hooks\": [{\"type\": \"command\", \"command\": \"echo keep\"}]}\n    ]\n  }\n}\n",
        )
        .unwrap();
        let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let text = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
        assert!(text.contains("echo keep"));
        assert!(
            payload["hooks_json"]["preserved_existing_entries"]
                .as_u64()
                .unwrap()
                >= 1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_removes_legacy_python_codex_hooks() {
        let root = fresh_path("remove-legacy-python");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
        fs::write(
            codex_home.join("hooks.json"),
            r#"{
  "hooks": {
"UserPromptSubmit": [
  {
    "hooks": [
      {
        "type": "command",
        "command": "/usr/bin/env python3 \"/Users/joe/Developer/skill/.codex/hooks/review_subagent_gate.py\"",
        "timeout": 10
      }
    ]
  }
],
"Stop": [
  {
    "hooks": [
      {"type": "command", "command": "echo keep"},
      {"type": "command", "command": "python3 review_subagent_gate.py"}
    ]
  }
]
  }
}
"#,
        )
        .unwrap();
        let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let text = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
        assert!(!text.contains("review_subagent_gate.py"));
        assert!(text.contains("echo keep"));
        assert!(text.contains("codex-router-rs-hook.sh"));
        assert!(text.contains("UserPromptSubmit"));
        assert_eq!(
            payload["hooks_json"]["removed_legacy_entries"].as_u64(),
            Some(2)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn idempotent_install() {
        let root = fresh_path("idempotent");
        let codex_home = root.join("codex");
        let first = run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let second = run_install(&codex_home, Path::new("."), InstallMode::Apply);
        assert_eq!(first["config_toml"]["status"].as_str(), Some("created"));
        assert_eq!(second["config_toml"]["status"].as_str(), Some("unchanged"));
        assert_eq!(second["hooks_json"]["status"].as_str(), Some("unchanged"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_mode_does_not_write() {
        let root = fresh_path("check-mode");
        let codex_home = root.join("codex-check-do-not-write");
        let payload = run_install(&codex_home, Path::new("."), InstallMode::Check);
        assert_eq!(
            payload["config_toml"]["status"].as_str(),
            Some("would-create")
        );
        assert_eq!(
            payload["hooks_json"]["status"].as_str(),
            Some("would-create")
        );
        assert!(!codex_home.join("config.toml").exists());
        assert!(!codex_home.join("hooks.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hook_command_format_pure_router_rs_binary() {
        let repo_root = Path::new("/Users/joe/Developer/skill");
        let stop_command = build_install_hook_command(repo_root, "Stop");
        assert!(stop_command.contains("codex-router-rs-hook.sh\" Stop"));
        assert!(!stop_command.contains("codex hook --event=Stop"));
        assert!(!stop_command.contains("/Users/joe/Developer/skill"));
        let pre_tool_command = build_install_hook_command(repo_root, "PreToolUse");
        assert!(pre_tool_command.contains("codex-router-rs-hook.sh\" PreToolUse"));
        assert!(!pre_tool_command.contains("codex hook pre-tool-use"));
    }

    #[test]
    fn hook_command_ignores_repo_root_shell_content() {
        let repo_root = Path::new("/tmp/repo-with-'quote");
        let command = build_install_hook_command(repo_root, "UserPromptSubmit");
        assert!(!command.contains("/tmp/repo-with-"));
        assert!(command.contains("git rev-parse --show-toplevel"));
        assert!(command.contains("codex-router-rs-hook.sh"));
        let status = Command::new("bash")
            .arg("-n")
            .arg("-c")
            .arg(&command)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn apply_creates_backup_when_hooks_existed() {
        let root = fresh_path("backup");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
        fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
        let before = fs::metadata(codex_home.join("hooks.json"))
            .unwrap()
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let backup = payload["hooks_json"]["backup_path"]
            .as_str()
            .map(PathBuf::from)
            .unwrap();
        assert!(backup.exists());
        let after = fs::metadata(codex_home.join("hooks.json"))
            .unwrap()
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        assert!(after >= before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_payload_contains_projection_version_and_digest() {
        let root = fresh_path("payload-meta");
        let codex_home = root.join("codex");
        let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
        assert_eq!(
            payload["projection_version"].as_str(),
            Some(ROUTER_RS_HOOK_PROJECTION_VERSION)
        );
        assert!(payload["command_digest"]
            .as_str()
            .is_some_and(|v| v.len() == 64));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_writes_manifest_file_with_version() {
        let root = fresh_path("manifest");
        let codex_home = root.join("codex");
        let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
        let manifest_path = codex_home.join(".router-rs-install.manifest.json");
        let manifest_text = fs::read_to_string(manifest_path).unwrap();
        let manifest: Value = serde_json::from_str(&manifest_text).unwrap();
        assert_eq!(
            manifest["projection_version"].as_str(),
            Some(ROUTER_RS_HOOK_PROJECTION_VERSION)
        );
        assert_eq!(manifest["command_digest"], payload["command_digest"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_hooks_backup_failure_bubbles_error() {
        let root = fresh_path("backup-failure");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
        fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o500)).unwrap();
        let before = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
        let result = install_codex_hooks(&codex_home, Path::new("."), InstallMode::Apply);
        #[cfg(unix)]
        fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        let after = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
        assert_eq!(before, after);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_hooks_write_failure_restores_backup() {
        let root = fresh_path("write-failure");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
        fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
        let before = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
        FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.set(true));
        let result = install_codex_hooks(&codex_home, Path::new("."), InstallMode::Apply);
        FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.set(false));
        assert!(result.is_err());
        let after = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
        assert_eq!(before, after);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_hooks_permission_denied_fails_cleanly() {
        let root = fresh_path("permission-denied");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o500)).unwrap();
        let result = install_codex_hooks(&codex_home, Path::new("."), InstallMode::Apply);
        #[cfg(unix)]
        fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_hooks_symlink_target_handled_safely() {
        let root = fresh_path("symlink-hooks");
        let codex_home = root.join("codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
        let target = root.join("actual-hooks.json");
        fs::write(&target, "{\"hooks\":{}}\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, codex_home.join("hooks.json")).unwrap();
        let result = install_codex_hooks(&codex_home, Path::new("."), InstallMode::Apply);
        assert!(result.is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_hooks_invalid_root_returns_error() {
        let result = merge_hooks_json(Some(json!([])), &install_hook_commands(Path::new(".")));
        assert!(result
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
            .contains("root type: expected object"));
    }

    #[test]
    fn install_hooks_invalid_hooks_field_returns_error() {
        let result = merge_hooks_json(
            Some(json!({"hooks":"not-an-object"})),
            &install_hook_commands(Path::new(".")),
        );
        assert!(result
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
            .contains("`hooks` must be an object"));
    }

    #[test]
    fn install_hooks_invalid_event_array_returns_error() {
        let result = merge_hooks_json(
            Some(json!({"hooks":{"Stop":{"x":1}}})),
            &install_hook_commands(Path::new(".")),
        );
        assert!(result
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
            .contains("hooks.Stop must be an array"));
    }

    #[test]
    fn atomic_write_completes_normally_with_fsync() {
        let root = fresh_path("atomic-fsync");
        let output = root.join("file.txt");
        write_atomic_text(&output, "hello").unwrap();
        assert_eq!(fs::read_to_string(output).unwrap(), "hello");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn codex_hook_rejects_oversized_stdin() {
        let large = vec![b'a'; 5 * 1024 * 1024];
        let mut cursor = std::io::Cursor::new(large);
        let err = read_limited_stdin(&mut cursor)
            .unwrap_err()
            .to_hook_exit();
        assert_eq!(err, "stdin_too_large");
    }

    #[test]
    fn codex_hook_rejects_invalid_utf8_stdin() {
        let bytes = vec![0xff, 0xfe, 0xfd];
        let mut cursor = std::io::Cursor::new(bytes);
        let err = read_limited_stdin(&mut cursor)
            .unwrap_err()
            .to_hook_exit();
        assert_eq!(err, "stdin_invalid_utf8");
    }

    #[test]
    fn codex_hook_rejects_truncated_utf8_sequence_stdin() {
        let mut buf = vec![b'a'; 64];
        buf.push(0x80);
        let mut cursor = std::io::Cursor::new(buf);
        let err = read_limited_stdin(&mut cursor)
            .unwrap_err()
            .to_hook_exit();
        assert_eq!(err, "stdin_invalid_utf8");
    }
}

mod lifecycle_context_tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn env_lock() -> router_rs::test_env_sync::ProcessEnvLockGuard {
        router_rs::test_env_sync::process_env_lock()
    }

    fn fresh_repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codex-lifecycle-context-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(dir.join(".codex/hook-state")).unwrap();
        dir
    }

    fn run_gate(repo: &std::path::Path, payload: &Value) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
        let _g = env_lock();
        run_codex_review_subagent_gate(repo, payload)
    }

    const TEST_COMPACT_FINDING: &str = "[P1] core/router-rs/src/hosts/codex_hooks/mod.rs:1 — wave-2 compact gate clear evidence line";

    #[test]
    fn operator_inject_off_skips_session_start_additional_context() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_OPERATOR_INJECT");
        std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
        let repo = fresh_repo();
        let out =
            super::handle_codex_session_start(&repo, &json!({"source": "startup"}));
        assert!(
            out.is_none(),
            "advisory SessionStart must honor ROUTER_RS_OPERATOR_INJECT kill-switch: {out:?}"
        );
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
            None => std::env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
        }
    }

    #[test]
    fn operator_inject_off_skips_user_prompt_submit_additional_context() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_OPERATOR_INJECT");
        std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
        let repo = fresh_repo();
        let evt = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-inject-off-ups",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let out = super::handle_codex_userpromptsubmit(&repo, &evt);
        assert!(
            out.is_none(),
            "advisory UserPromptSubmit must honor ROUTER_RS_OPERATOR_INJECT kill-switch: {out:?}"
        );
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
            None => std::env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
        }
    }

    #[test]
    fn user_prompt_submit_injects_paper_prose_hook_by_default() {
        let _g = env_lock();
        let prior_hook = std::env::var_os("ROUTER_RS_CODEX_PAPER_PROSE_HOOK");
        std::env::remove_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK");
        let repo = fresh_repo();
        let evt = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"prose-ups-default",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"SCI润色 abstract"
        });
        let out = super::handle_codex_userpromptsubmit(&repo, &evt);
        let ctx = out
            .as_ref()
            .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
            .unwrap_or_default();
        assert!(
            ctx.contains("PAPER_PROSE_QUALITY_HOOK"),
            "expected prose hook in UPS context: {ctx}"
        );
        match prior_hook {
            Some(v) => std::env::set_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK", v),
            None => std::env::remove_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK"),
        }
    }

    #[test]
    fn user_prompt_submit_review_emits_subagent_gate_context() {
        let repo = fresh_repo();
        let payload = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-1",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review全仓找bug"
        });
        let out = run_gate(&repo, &payload).unwrap();
        let ctx = out
            .as_ref()
            .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
            .unwrap_or_default();
        assert!(
            ctx.contains("配对审稿") || ctx.contains("fork_context"),
            "spawn-first nudge: {ctx}"
        );
        assert!(ctx.contains("fork_context=false"));
        assert!(ctx.contains("general-purpose") || ctx.contains("best-of-n-runner"));
        if !ctx.is_empty() {
            assert!(ctx.len() <= codex_additional_context_max_bytes());
        }
        let state = codex_load_state(&repo, &payload).unwrap().unwrap();
        assert_eq!(state.seq, 1);
        assert!(state.review_required);
    }

    #[test]
    fn user_prompt_submit_narrow_path_skips_review_arm() {
        let repo = fresh_repo();
        let payload = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-narrow",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"review ./README.md"
        });
        let out = run_gate(&repo, &payload).unwrap();
        assert!(
            out.is_none(),
            "narrow single-path review must not arm gate: {out:?}"
        );
        let armed = codex_load_state(&repo, &payload)
            .ok()
            .flatten()
            .map(|s| s.review_required)
            .unwrap_or(false);
        assert!(!armed, "narrow prompt should not set review_required");
    }

    #[test]
    fn user_prompt_submit_with_override_does_not_emit() {
        let repo = fresh_repo();
        let payload = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-ovr",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review全仓找bug，不要用子代理"
        });
        let out = run_gate(&repo, &payload).unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn additional_context_is_deduped_and_capped() {
        let duplicate = "Codex live state: one".to_string();
        let long_line = "x".repeat(codex_additional_context_max_bytes());
        let ctx = codex_compact_contexts(vec![
            duplicate.clone(),
            duplicate,
            long_line.clone(),
            long_line,
        ])
        .unwrap();
        assert!(ctx.len() <= codex_additional_context_max_bytes());
        assert_eq!(ctx.matches("Codex live state: one").count(), 1);
    }

    #[test]
    fn session_start_compact_context_under_small_budget_without_digest() {
        let repo = fresh_repo();
        let task_id = "session-priority";
        fs::create_dir_all(repo.join(ARTIFACTS_CURRENT_DIR).join(task_id)).expect("mkdir task");
        fs::write(
            repo.join(ARTIFACTS_CURRENT_DIR).join("active_task.json"),
            format!(r#"{{"task_id":"{task_id}"}}"#),
        )
        .expect("write active");
        fs::write(
            repo.join(ARTIFACTS_CURRENT_DIR).join(task_id).join("GOAL_STATE.json"),
            r#"{"goal":"keep the active goal visible before any static context","status":"running","drive_until_done":true,"done_when":["done"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("write goal");
        fs::write(
            repo.join(ARTIFACTS_CURRENT_DIR).join("SESSION_SUMMARY.md"),
            "very long continuity line ".repeat(80),
        )
        .expect("write summary");

        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
        std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
        let out = handle_codex_session_start(&repo, &json!({"source":"startup"}))
            .expect("session start output");
        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
        let ctx = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("additionalContext");
        assert!(!ctx.contains("Continuity digest:"), "{ctx}");
        assert!(ctx.contains("Repo:"), "{ctx}");
        assert!(!ctx.contains("Goal: running"), "{ctx}");
        assert!(ctx.len() <= 256, "len={} ctx={ctx:?}", ctx.len());
    }

    #[test]
    fn post_tool_use_with_subagent_marks_seen_without_explore_counting_deep_independent() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-2",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-2",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"explore","fork_context":false}
        });
        let out = run_gate(&repo, &post).unwrap();
        assert!(out.is_none());
        let state = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(state.review_subagent_seen);
        assert!(
            !state.independent_review_subagent_seen,
            "explore must not satisfy Codex independent deep-review bar"
        );
        assert!(state.generic_subagent_seen);
        assert!(state.review_lane_seen);
        assert!(!state.parallel_lane_seen);
        assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#explore"));
    }

    #[test]
    fn post_tool_general_purpose_fork_false_counts_deep_independent() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-2gp",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-2gp",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let out = run_gate(&repo, &post).unwrap();
        assert!(out.is_none());
        let state = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(state.independent_review_subagent_seen);
        assert!(state.review_lane_seen);
    }

    #[test]
    fn post_tool_review_lane_fork_false_does_not_count_deep_independent() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-2rev",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-2rev",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"review","fork_context":false}
        });
        let out = run_gate(&repo, &post).unwrap();
        assert!(out.is_none());
        let state = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(
            !state.independent_review_subagent_seen,
            "review subagent_type is Claude-only; must not satisfy Codex deep_gate_lanes"
        );
    }

    #[test]
    fn post_tool_use_without_subagent_type_marks_generic_and_untyped_label() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-2b",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-2b",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"prompt":"no type field"}
        });
        let out = run_gate(&repo, &post).unwrap();
        assert!(out.is_none());
        let state = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(state.generic_subagent_seen);
        assert!(state.review_subagent_seen);
        assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#untyped"));
        assert!(!state.review_lane_seen);
        assert!(!state.parallel_lane_seen);
    }

    #[test]
    fn saw_subagent_codex_accepts_whitelisted_tool_without_recognized_type() {
        assert!(saw_subagent_codex(
            "Task",
            &json!({"prompt":"missing type"})
        ));
    }

    #[test]
    fn delegation_stop_unblocks_after_worker_subagent() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-6c",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"前端后端测试并行推进"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-6c",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"worker"}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-6c",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn stop_blocks_when_hook_state_corrupt() {
        let _guard = env_lock();
        std::env::set_var("ROUTER_RS_HOOK_STATE_FAIL_OPEN", "true");
        let repo = fresh_repo();
        let payload = json!({
            "hook_event_name":"Stop",
            "session_id":"stop-corrupt-1",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"x"
        });
        let path = super::codex_state_path(&repo, &payload);
        fs::write(&path, b"{not json").unwrap();
        // B-3: corrupted state auto-recovers (backup .bak + reset to fresh)
        let out = super::handle_codex_stop(&repo, &payload);
        // Stop with no review_required proceeds normally (None = allow)
        assert!(out.is_none(), "corrupted state should auto-recover, not block: {out:?}");
        // Verify backup was created
        let bak_path = path.with_extension("json.bak");
        assert!(bak_path.exists(), "corrupt file should be backed up to .bak");
    }

    #[test]
    fn session_key_without_stable_identifier_is_deterministic() {
        let _g = env_lock();
        std::env::remove_var("CODEX_SESSION_ID");
        std::env::remove_var("CODEX_CONVERSATION_ID");
        std::env::remove_var("ROUTER_RS_CODEX_HOOK_STATE_SALT");
        let repo = fresh_repo();
        let event = json!({"cwd": repo.to_string_lossy()});
        let k1 = super::codex_session_key(&repo, &event);
        let k2 = super::codex_session_key(&repo, &event);
        assert_eq!(k1, k2, "fallback keys must alias the same hook-state file");
        assert_eq!(k1.len(), 32);
    }

    #[test]
    fn codex_session_key_differs_by_payload_session_when_strict_off() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY");
        std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "0");
        std::env::remove_var("CODEX_SESSION_ID");
        std::env::remove_var("CODEX_CONVERSATION_ID");
        let repo = fresh_repo();
        let cwd = repo.to_string_lossy().to_string();
        let k1 = super::codex_session_key(
            &repo,
            &json!({"session_id":"sess-a","cwd":cwd}),
        );
        let k2 = super::codex_session_key(
            &repo,
            &json!({"session_id":"sess-b","cwd":cwd}),
        );
        assert_ne!(k1, k2, "payload session_id must isolate hook-state when strict off");
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", v),
            None => std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY"),
        }
    }

    #[test]
    fn delegation_stop_does_not_block_when_only_explore_subagent_observed() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-6b",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"前端后端测试并行推进"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-6b",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"explore","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-6b",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn additional_context_truncates_on_newline_preference_under_small_budget() {
        // codex_additional_context_max_bytes clamps to [256, 8192]; use the
        // floor so the assertions exercise the real budget rather than a
        // value that the clamp silently rewrites.
        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
        std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
        let line1 = format!("{}{}", "A".repeat(24), ": L1");
        let line2 = format!("{}{}", "C".repeat(24), ": L2");
        let line3 = "B".repeat(240);
        let ctx = codex_compact_contexts(vec![format!("{line1}\n{line2}\n{line3}")]).unwrap();
        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
        assert!(ctx.ends_with("..."));
        assert!(
            ctx.matches('\n').count() >= 1,
            "expected multiple lines before ellipsis when budget allows: {ctx:?}"
        );
        assert!(ctx.len() <= 256);
    }

    #[test]
    fn codex_compact_contexts_dedup_requires_exact_trim_match() {
        let a = "Repo: /path/A";
        let b = "repo: /path/B";
        let ctx = codex_compact_contexts(vec![a.to_string(), b.to_string()]).expect("ctx");
        assert!(
            ctx.contains(a),
            "distinct lines must not merge on ASCII case: {ctx:?}"
        );
        assert!(
            ctx.contains(b),
            "distinct lines must not merge on ASCII case: {ctx:?}"
        );
    }

    /// Multi-segment `codex_compact_contexts` join order is preserved when the
    /// combined string is truncated (SessionStart budget). Complements
    /// `additional_context_truncates_on_newline_preference_under_small_budget`
    /// (single blob + newline preference inside one segment).
    #[test]
    fn codex_compact_contexts_preserves_join_order_under_small_budget() {
        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
        std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
        let part1 = "CODEX_JOIN_ORDER_MARK_FIRST:alpha";
        let part2 = "CODEX_JOIN_ORDER_MARK_SECOND:beta";
        let part3 = format!("CODEX_JOIN_ORDER_MARK_TAIL:{}", "Z".repeat(280));
        let ctx = codex_compact_contexts(vec![part1.to_string(), part2.to_string(), part3])
            .expect("expected combined contexts");
        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
        std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
        assert!(ctx.len() <= 256, "len={}", ctx.len());
        assert!(ctx.ends_with("..."));
        assert!(
            ctx.contains("CODEX_JOIN_ORDER_MARK_FIRST"),
            "first joined segment should survive truncation: {ctx:?}"
        );
        assert!(
            ctx.contains("CODEX_JOIN_ORDER_MARK_SECOND"),
            "second joined segment should appear before tail is cut: {ctx:?}"
        );
        let pos_first = ctx.find("CODEX_JOIN_ORDER_MARK_FIRST").expect("first mark");
        let pos_second = ctx
            .find("CODEX_JOIN_ORDER_MARK_SECOND")
            .expect("second mark");
        assert!(
            pos_first < pos_second,
            "join order should be preserved in truncated output: {ctx:?}"
        );
    }

    #[test]
    fn saw_subagent_codex_accepts_subagent_type_field() {
        assert!(saw_subagent_codex(
            "Task",
            &json!({"subagent_type":"explore"})
        ));
    }

    #[test]
    fn saw_subagent_codex_accepts_agent_type_field() {
        assert!(saw_subagent_codex(
            "Task",
            &json!({"agent_type":"ci-investigator"})
        ));
    }

    #[test]
    fn saw_subagent_codex_accepts_native_codex_agent_types() {
        for agent_type in ["default", "explorer", "worker"] {
            assert!(
                saw_subagent_codex("functions.spawn_agent", &json!({"agent_type":agent_type})),
                "expected native Codex agent_type={agent_type} to count as a subagent"
            );
        }
    }

    #[test]
    fn saw_subagent_codex_accepts_whitelisted_tool_even_when_type_unrecognized() {
        assert!(saw_subagent_codex(
            "Task",
            &json!({"subagent_type":"random-thing"})
        ));
    }

    #[test]
    fn post_tool_use_without_state_is_non_fatal() {
        let repo = fresh_repo();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-2c",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"explore","fork_context":false}
        });
        let out = run_gate(&repo, &post).unwrap();
        assert!(out.is_none());
        let state = codex_load_state(&repo, &post)
            .unwrap()
            .expect("lazy hook-state");
        assert!(state.generic_subagent_seen);
        assert!(
            !state.independent_review_subagent_seen,
            "explore must not satisfy deep independent reviewer ledger"
        );
    }

    #[test]
    fn post_tool_use_without_prior_state_persists_independent_deep_reviewer() {
        let _g = env_lock();
        let repo = fresh_repo();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-no-ups-deep",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review",
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let out = run_gate(&repo, &post).unwrap();
        assert!(out.is_none());
        let state = codex_load_state(&repo, &post).unwrap().expect("state");
        assert!(state.independent_review_subagent_seen);
        assert!(
            state.review_required,
            "deep PostTool with review prompt must arm review_required (B5 lazy bypass)"
        );
    }

    #[test]
    fn post_tool_deep_reviewer_without_review_prompt_does_not_arm_gate() {
        let repo = fresh_repo();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-no-review-arm",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"前端后端测试并行推进",
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let state = codex_load_state(&repo, &post).unwrap().expect("state");
        assert!(state.independent_review_subagent_seen);
        assert!(!state.review_required, "non-review PostTool must not arm review_required");
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-no-review-arm",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(
            out.is_none(),
            "Stop must not block when review_required was never armed: {out:?}"
        );
    }

    #[test]
    fn lazy_post_tool_deep_reviewer_arms_gate_and_stop_blocks_without_compact() {
        let _g = env_lock();
        let repo = fresh_repo();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-lazy-stop-contract",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review",
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        assert!(run_gate(&repo, &post)
            .unwrap()
            .is_none());
        let loaded = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(loaded.independent_review_subagent_seen);
        assert!(loaded.review_required, "deep PostTool must arm review_required");
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-lazy-stop-contract",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":""
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(
            msg.contains("CODEX_REVIEW_GATE"),
            "armed gate must block Stop without compact: {out:?}"
        );
    }

    #[test]
    fn post_tool_use_observes_fork_context_on_event_root() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-event-fork",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-event-fork",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "fork_context": false,
            "tool_input":{"subagent_type":"general-purpose"}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-event-fork",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续",
            "response": TEST_COMPACT_FINDING
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(
            out.is_none(),
            "event-root fork_context should satisfy independent reviewer; out={out:?}"
        );
    }

    #[test]
    fn post_tool_use_with_invalid_state_blocks_fail_closed() {
        let _guard = env_lock();
        std::env::set_var("ROUTER_RS_HOOK_STATE_FAIL_OPEN", "true");
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-2d",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let state_path = codex_state_path(&repo, &start);
        fs::write(&state_path, "{invalid").unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-2d",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"explore"}
        });
        // B-3: corrupted state auto-recovers; PostToolUse proceeds with fresh state
        let out = run_gate(&repo, &post).unwrap();
        // Fresh state with subagent_type=explore should trigger review gate
        // but not due to corruption block
        assert!(
            out.is_none() || out.as_ref().and_then(|v| v.get("decision")).and_then(Value::as_str) != Some("block"),
            "invalid hook-state should auto-recover on PostToolUse, not block: {out:?}"
        );
        // Verify backup was created
        let bak_path = state_path.with_extension("json.bak");
        assert!(bak_path.exists(), "corrupt file should be backed up to .bak");
    }

    #[test]
    fn stop_without_state_blocks_when_review_prompt_without_ups_evidence() {
        let repo = fresh_repo();
        let payload = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-3",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let out = run_gate(&repo, &payload).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
    }

    #[test]
    fn stop_without_state_does_not_block_when_no_text() {
        let repo = fresh_repo();
        let payload = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-4",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":""
        });
        let out = run_gate(&repo, &payload).unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn stop_with_review_prompt_no_subagent_blocks() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-5",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-5",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
    }

    #[test]
    fn stop_with_review_prompt_shared_fork_subagent_blocks() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-5b",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-5b",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"explore","fork_context":true}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-5b",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
    }

    #[test]
    fn stop_with_review_prompt_missing_fork_context_subagent_blocks() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-5c",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-5c",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"explore"}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-5c",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
    }

    #[test]
    fn stop_with_delegation_prompt_does_not_block() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-6",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"前端后端测试并行推进"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-6",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn stop_with_subagent_seen_resets_state_after_general_purpose_deep_reviewer() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-7",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-7",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-7",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续",
            "response": TEST_COMPACT_FINDING
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(out.is_none());
        let state = codex_load_state(&repo, &stop).unwrap().unwrap();
        assert_eq!(state.seq, 0);
        assert!(!state.review_subagent_seen);
        assert!(!state.independent_review_subagent_seen);
    }

    #[test]
    fn stop_blocks_after_posttool_without_compact_findings() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-wave2-post-only",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-wave2-post-only",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-wave2-post-only",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(msg.contains("CODEX_REVIEW_GATE"), "posttool alone must not clear: {out:?}");
        assert!(msg.contains("phase=2"), "expected phase=2 after posttool: {msg}");
    }

    #[test]
    fn stop_compact_alone_without_posttool_blocks() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-wave2-compact-only",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-wave2-compact-only",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续",
            "response": TEST_COMPACT_FINDING
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(
            msg.contains("CODEX_REVIEW_GATE"),
            "compact alone must not clear without countable posttool: {out:?}"
        );
    }

    #[test]
    fn stop_rg_clear_clears_review_gate() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-rg-clear",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-rg-clear",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"rg_clear"
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(out.is_none(), "rg_clear must clear codex review gate: {out:?}");
    }

    #[test]
    fn my_light_implementx_stop_suppresses_review_gate() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-my-light",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"/implementx run waves"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let armed = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-my-light",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &armed).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-my-light",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"/implementx finish"
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(
            out.is_none(),
            "my-light must suppress CODEX_REVIEW_GATE on Stop: {out:?}"
        );
    }

    #[test]
    fn my_light_post_tool_suppress_clears_hook_state() {
        let repo = fresh_repo();
        let sid = "sm-my-light-post";
        let arm = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &arm).unwrap();
        assert!(
            codex_load_state(&repo, &arm)
                .unwrap()
                .map(|s| s.review_required)
                .unwrap_or(false)
        );
        let my = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"/implementx run waves"
        });
        let _ = run_gate(&repo, &my).unwrap();
        assert!(
            !codex_load_state(&repo, &my)
                .unwrap()
                .map(|s| s.review_required)
                .unwrap_or(true),
            "my-light UPS must clear review_required"
        );
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"/implementx",
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        assert!(
            codex_load_state(&repo, &post)
                .unwrap()
                .map(|s| s.seq)
                .unwrap_or(0)
                == 0,
            "my-light PostTool (suppress) must clear hook-state"
        );
    }

    #[test]
    fn codex_review_gate_disable_env_skips_block() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
        router_rs::hook_common::set_test_my_light_override(Some(true));
        std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1");
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-disable",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-disable",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(out.is_none(), "disable env must skip gate: {out:?}");
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE"),
        }
        router_rs::hook_common::set_test_my_light_override(None);
    }

    #[test]
    fn codex_review_gate_disable_clears_armed_state_on_userpromptsubmit() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
        let repo = fresh_repo();
        let arm = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-disable-clear",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &arm).unwrap();
        assert!(
            codex_load_state(&repo, &arm)
                .unwrap()
                .map(|s| s.review_required)
                .unwrap_or(false)
        );
        router_rs::hook_common::set_test_my_light_override(Some(true));
        std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1");
        let ups_disable = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-disable-clear",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let _ = run_gate(&repo, &ups_disable).unwrap();
        let state = codex_load_state(&repo, &ups_disable).unwrap().unwrap();
        assert_eq!(state.seq, 0, "disable UPS must reset hook-state");
        assert!(!state.review_required);
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE"),
        }
        router_rs::hook_common::set_test_my_light_override(None);
    }

    #[test]
    fn codex_review_gate_disable_clears_state_on_posttool() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
        let repo = fresh_repo();
        let arm = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-disable-post",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &arm).unwrap();
        router_rs::hook_common::set_test_my_light_override(Some(true));
        std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1");
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-disable-post",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review",
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let state = codex_load_state(&repo, &post).unwrap().unwrap();
        assert_eq!(state.seq, 0, "disable PostTool must reset hook-state");
        assert!(!state.review_required);
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE"),
        }
        router_rs::hook_common::set_test_my_light_override(None);
    }

    #[test]
    fn post_tool_delegate_tool_does_not_count_deep_evidence() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-delegate",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-delegate",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Delegate",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let state = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(!state.independent_review_subagent_seen);
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-delegate",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(msg.contains("CODEX_REVIEW_GATE") && msg.contains("phase=0"));
    }

    #[test]
    fn post_tool_gp_missing_fork_codex_infer_off_blocks_at_stop() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
        std::env::set_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", "0");
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-infer-off",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-infer-off",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose"}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let state = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(!state.independent_review_subagent_seen);
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-infer-off",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(msg.contains("CODEX_REVIEW_GATE"));
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", v),
            None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE"),
        }
    }

    #[test]
    fn user_prompt_submit_review_and_implementx_suppresses_review_arming() {
        let _g = env_lock();
        let repo = fresh_repo();
        let sid = "sm-dual-review-implementx";
        let arm = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review这个仓库"
        });
        let _ = run_gate(&repo, &arm).unwrap();
        let armed = codex_load_state(&repo, &arm).unwrap().unwrap();
        assert!(armed.review_required, "review-only UPS should arm; got {armed:?}");
        let dual = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"请全面review这个仓库 /implementx 修复刚发现的问题"
        });
        let _ = run_gate(&repo, &dual).unwrap();
        let cleared = codex_load_state(&repo, &dual).unwrap().unwrap();
        assert!(
            !cleared.review_required,
            "my-light goal drive must clear/disarm review on Codex UPS; got {cleared:?}"
        );
    }

    #[test]
    fn rearm_review_resets_codex_independent_evidence() {
        let _g = env_lock();
        let repo = fresh_repo();
        let sid = "sm-rearm-evidence";
        let arm = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &arm).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let seeded = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(seeded.independent_review_subagent_seen);
        assert!(seeded.phase >= 2);
        let rearm = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review全仓找bug"
        });
        let _ = run_gate(&repo, &rearm).unwrap();
        let reset = codex_load_state(&repo, &rearm).unwrap().unwrap();
        assert!(
            !reset.independent_review_subagent_seen,
            "re-arm review must reset PostTool evidence"
        );
        assert_eq!(reset.phase, 0);
        assert_eq!(reset.subagent_start_count, 0);
        assert!(!reset.review_subagent_seen);
        assert!(!reset.generic_subagent_seen);
        assert!(reset.review_required);
    }

    #[test]
    fn rearm_review_preserves_evidence_when_override() {
        let repo = fresh_repo();
        let sid = "sm-rearm-override";
        let arm = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &arm).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let seeded = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(seeded.independent_review_subagent_seen);
        let override_ups = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review，不要用子代理"
        });
        let _ = run_gate(&repo, &override_ups).unwrap();
        let kept = codex_load_state(&repo, &override_ups).unwrap().unwrap();
        assert!(
            kept.independent_review_subagent_seen,
            "override must not reset prior PostTool reviewer evidence"
        );
        assert!(kept.review_override);
    }

    #[test]
    fn legacy_phase_two_alone_compact_does_not_clear_codex_review_gate() {
        let _g = env_lock();
        let repo = fresh_repo();
        let sid = "sm-legacy-phase2-compact";
        let arm = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &arm).unwrap();
        let sp = codex_state_path(&repo, &arm);
        let mut state = codex_load_state(&repo, &arm).unwrap().unwrap();
        state.phase = 2;
        state.subagent_start_count = 0;
        state.independent_review_subagent_seen = false;
        state.review_required = true;
        assert!(codex_save_state_to_path(&sp, &state));
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id": sid,
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续",
            "response":"[P1] scripts/foo.rs:1 — issue — impact — verify",
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(
            msg.contains("CODEX_REVIEW_GATE"),
            "legacy phase=2 without PostTool start/independent must not clear gate; msg={msg:?}"
        );
        let loaded = codex_load_state(&repo, &stop).unwrap().unwrap();
        assert!(
            loaded.phase < 3,
            "compact must not bump to phase 3 without countable evidence"
        );
    }

    #[test]
    fn stop_reject_reason_in_response_clears_gate() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-reject-resp",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-reject-resp",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"",
            "response":"small_task"
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(out.is_none(), "reject token in response must clear: {out:?}");
    }

    #[test]
    fn stop_clears_after_best_of_n_runner_posttool_and_compact() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-bon",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-bon",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"best-of-n-runner","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-bon",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续",
            "response": TEST_COMPACT_FINDING
        });
        let out = run_gate(&repo, &stop).unwrap();
        assert!(out.is_none(), "best-of-n + compact must clear: {out:?}");
    }

    #[test]
    fn stop_with_review_explore_fork_false_still_blocks() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-7-explore",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-7-explore",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"explore","fork_context":false}
        });
        let _ = run_gate(&repo, &post).unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-7-explore",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续"
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
    }

    #[test]
    fn stop_hook_active_bypass_skips_gate_only_when_env_set() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS");
        std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", "1");
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-8-bypass",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let payload = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-8-bypass",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续",
            "stop_hook_active": true
        });
        let out = run_gate(&repo, &payload).unwrap();
        assert!(out.is_none(), "bypass env must skip review gate on replay: {out:?}");
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", v),
            None => std::env::remove_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS"),
        }
    }

    #[test]
    fn stop_hook_active_still_blocks_review_gate_by_default() {
        let _g = env_lock();
        let prior = std::env::var_os("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS");
        std::env::remove_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS");
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-8-default",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"全面review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let payload = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-8-default",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"继续",
            "stop_hook_active": true
        });
        let out = run_gate(&repo, &payload).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert!(
            out.as_ref()
                .and_then(|v| v.get("decision"))
                .and_then(Value::as_str)
                == Some("block")
                && msg.contains("CODEX_REVIEW_GATE"),
            "stop_hook_active without bypass must still enforce review: {out:?}"
        );
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", v),
            None => {}
        }
    }

    #[test]
    fn stop_completion_claim_blocks_with_closeout_followup_when_strict() {
        let _g = env_lock();
        let prev = std::env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
        std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");
        let repo = fresh_repo();
        let tid = "t-codex-closeout";
        fs::create_dir_all(repo.join(ARTIFACTS_CURRENT_DIR).join(tid)).unwrap();
        fs::write(
            repo.join(ARTIFACTS_CURRENT_DIR).join("active_task.json"),
            format!(r#"{{"task_id":"{tid}"}}"#),
        )
        .unwrap();
        let stop = json!({
            "hook_event_name":"Stop",
            "session_id":"sm-closeout",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"all done, shipped"
        });
        let out = run_gate(&repo, &stop).unwrap();
        let msg = out
            .as_ref()
            .and_then(|v| v["followup_message"].as_str())
            .unwrap_or_default();
        assert_eq!(
            out.as_ref()
                .and_then(|v| v.get("decision"))
                .and_then(Value::as_str),
            Some("block")
        );
        assert!(
            msg.contains("CLOSEOUT_FOLLOWUP") && msg.contains("missing_record"),
            "expected closeout block on Stop; got {out:?}"
        );
        match prev {
            Some(v) => std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
    }

    #[test]
    fn post_tool_state_lock_failure_blocks_like_user_prompt_submit() {
        let repo = fresh_repo();
        let event = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"lock-pt-block",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        });
        let state_path = codex_state_path(&repo, &event);
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
        fs::write(&lock_path, "pid=1 ts=1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o000)).unwrap();
        }
        #[cfg(not(unix))]
        {
            let guard = acquire_codex_state_lock(&state_path).unwrap();
            let _hold = guard;
        }
        let out = run_gate(&repo, &event).unwrap();
        assert_eq!(
            out.as_ref().and_then(|v| v.get("decision")).and_then(Value::as_str),
            Some("block"),
            "PostTool lock failure must fail-closed: {out:?}"
        );
        assert_eq!(
            out.as_ref().and_then(|v| v.get("reason")).and_then(Value::as_str),
            Some("Codex hook state could not be persisted under .codex/hook-state.")
        );
    }

    #[test]
    fn no_drift_warn_when_manifest_missing() {
        let repo = fresh_repo();
        let codex_home = repo.join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();
        std::env::set_var("CODEX_HOME", &codex_home);
        let payload = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-drift-1",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"普通提问"
        });
        let out = run_gate(&repo, &payload).unwrap();
        // Plain prompts no longer arm a hard subagent gate,
        // so the hook may return None (no context to emit). If context IS
        // emitted for other reasons, it must not contain a drift warning.
        let ctx = out
            .as_ref()
            .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
            .unwrap_or_default()
            .to_string();
        assert!(!ctx.contains("hook projection drift detected"));
    }

    #[test]
    fn no_drift_warn_when_manifest_matches() {
        let repo = fresh_repo();
        let codex_home = repo.join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();
        std::env::set_var("CODEX_HOME", &codex_home);
        let manifest = json!({
            "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
            "command_digest": "abc",
        });
        fs::write(
            codex_home.join(".router-rs-install.manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();
        let payload = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-drift-2",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"普通提问"
        });
        let out = run_gate(&repo, &payload).unwrap();
        if let Some(value) = out {
            let ctx = value["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap_or_default();
            assert!(!ctx.contains("hook projection drift detected"));
        }
    }

    #[test]
    fn v1_migration_ignores_removed_override_flag() {
        let repo = fresh_repo();
        let event = json!({"session_id":"v1-override"});
        let state_path = codex_state_path(&repo, &event);
        fs::write(
            state_path,
            r#"{"schema_version":1,"override":true,"subagent_required":true}"#,
        )
        .unwrap();
        let state = codex_load_state(&repo, &event).unwrap().unwrap();
        assert_eq!(state.seq, 0);
    }

    #[test]
    fn v1_migration_ignores_removed_reject_reason_flag() {
        let repo = fresh_repo();
        let event = json!({"session_id":"v1-reject"});
        let state_path = codex_state_path(&repo, &event);
        fs::write(
            state_path,
            r#"{"schema_version":1,"reject_reason_seen":true}"#,
        )
        .unwrap();
        let state = codex_load_state(&repo, &event).unwrap().unwrap();
        assert_eq!(state.seq, 0);
    }

    #[test]
    fn v1_delegation_only_maps_to_phase1() {
        let repo = fresh_repo();
        let event = json!({"session_id":"v1-phase"});
        let state_path = codex_state_path(&repo, &event);
        fs::write(
            state_path,
            r#"{"schema_version":1,"delegation_required":true,"review_subagent_seen":false}"#,
        )
        .unwrap();
        let state = codex_load_state(&repo, &event).unwrap().unwrap();
        assert_eq!(state.seq, 1);
    }

    #[test]
    fn codex_session_key_fallback_is_stable_without_identifiers() {
        let _guard = env_lock();
        std::env::remove_var("CODEX_SESSION_ID");
        std::env::remove_var("CODEX_CONVERSATION_ID");
        std::env::remove_var("ROUTER_RS_CODEX_HOOK_STATE_SALT");
        let repo = fresh_repo();
        let event = json!({"cwd": repo.to_string_lossy()});
        let a = codex_session_key(&repo, &event);
        let b = codex_session_key(&repo, &event);
        assert_eq!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn codex_session_key_differs_by_cwd_when_unstable() {
        let _guard = env_lock();
        std::env::remove_var("CODEX_SESSION_ID");
        std::env::remove_var("CODEX_CONVERSATION_ID");
        let repo = fresh_repo();
        let a = codex_session_key(&repo, &json!({"cwd":"/tmp/a"}));
        let b = codex_session_key(&repo, &json!({"cwd":"/tmp/b"}));
        assert_ne!(a, b, "unstable fallback must not collapse unlike cwd");
    }

    #[test]
    fn saw_subagent_codex_accepts_agent_type_camel_case_field() {
        assert!(saw_subagent_codex(
            "Task",
            &json!({"agentType":"browser-use"})
        ));
    }

    #[test]
    fn post_tool_use_with_agent_type_camel_case_marks_seen_without_deep_independent() {
        let repo = fresh_repo();
        let start = json!({
            "hook_event_name":"UserPromptSubmit",
            "session_id":"sm-2e",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt":"please do deep review"
        });
        let _ = run_gate(&repo, &start).unwrap();
        let post = json!({
            "hook_event_name":"PostToolUse",
            "session_id":"sm-2e",
            "cwd": repo.to_string_lossy().to_string(),
            "tool_name":"Task",
            "tool_input":{"agentType":"explore","fork_context":false}
        });
        let out = run_gate(&repo, &post).unwrap();
        assert!(out.is_none());
        let state = codex_load_state(&repo, &post).unwrap().unwrap();
        assert!(state.review_subagent_seen);
        assert!(
            !state.independent_review_subagent_seen,
            "explore must not satisfy Codex independent deep-review bar"
        );
        assert!(state.generic_subagent_seen);
        assert!(state.review_lane_seen);
        assert!(!state.parallel_lane_seen);
        assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#explore"));
    }

    #[test]
    fn dispatch_unknown_event_blocks_with_message() {
        let repo = fresh_repo();
        let payload = json!({
            "hook_event_name":"Other",
            "session_id":"sm-9",
            "cwd": repo.to_string_lossy().to_string()
        });
        let out = run_gate(&repo, &payload)
            .unwrap()
            .unwrap();
        assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
        assert!(out
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("unsupported"));
    }

    #[test]
    fn dispatch_missing_event_blocks_with_message() {
        let repo = fresh_repo();
        let payload = json!({"session_id":"sm-10"});
        let out = run_gate(&repo, &payload)
            .unwrap()
            .unwrap();
        assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
        assert!(out
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("missing"));
    }

    #[test]
    fn codex_state_lock_recovers_from_stale_lock() {
        let repo = fresh_repo();
        let event = json!({"session_id":"lock-stale"});
        let state_path = codex_state_path(&repo, &event);
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
        fs::write(&lock_path, "pid=999999 ts=1\n").unwrap();
        let lock = acquire_codex_state_lock(&state_path);
        assert!(lock.is_ok());
    }

    #[test]
    fn codex_state_lock_recovers_from_corrupt_lock_metadata() {
        let repo = fresh_repo();
        let event = json!({"session_id":"lock-corrupt"});
        let state_path = codex_state_path(&repo, &event);
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
        fs::write(&lock_path, "not-a-lock-metadata-line\n").unwrap();
        let lock = acquire_codex_state_lock(&state_path);
        assert!(lock.is_ok());
    }

    #[test]
    fn codex_state_lock_recovers_from_unparseable_pid_and_ts() {
        let repo = fresh_repo();
        let event = json!({"session_id":"lock-unparseable"});
        let state_path = codex_state_path(&repo, &event);
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
        fs::write(&lock_path, "pid=bad ts=bad\n").unwrap();
        let lock = acquire_codex_state_lock(&state_path);
        assert!(lock.is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn codex_state_lock_blocks_until_released() {
        use std::sync::mpsc;

        let repo = fresh_repo();
        let event = json!({"session_id":"lock-held"});
        let state_path = codex_state_path(&repo, &event);
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        let guard = acquire_codex_state_lock(&state_path).unwrap();
        let state_path_clone = state_path.clone();
        let (tx, rx) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let second = acquire_codex_state_lock(&state_path_clone).unwrap();
            let _ = tx.send(());
            drop(second);
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(rx.try_recv().is_err());
        drop(guard);
        rx.recv_timeout(Duration::from_secs(5))
            .expect("second acquirer should proceed after lock release");
        waiter.join().unwrap();
    }

    #[cfg(not(unix))]
    #[test]
    fn codex_state_lock_blocks_when_held() {
        let repo = fresh_repo();
        let event = json!({"session_id":"lock-held"});
        let state_path = codex_state_path(&repo, &event);
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        let guard = acquire_codex_state_lock(&state_path).unwrap();
        let started = std::time::Instant::now();
        let second = acquire_codex_state_lock(&state_path);
        assert!(second.is_err());
        assert!(started.elapsed() >= Duration::from_millis(1200));
        drop(guard);
    }

    #[test]
    fn codex_state_lock_serializes_concurrent_writes() {
        let repo = fresh_repo();
        let event = json!({"session_id":"lock-inc"});
        let repo_a = repo.clone();
        let repo_b = repo.clone();
        let event_a = event.clone();
        let event_b = event.clone();
        let worker = move |repo_root: PathBuf, ev: Value| {
            for _ in 0..1000 {
                with_codex_state_lock(&repo_root, &ev, |loaded| {
                    let mut state = loaded.unwrap_or_default();
                    state.seq += 1;
                    Ok((Some(state), ()))
                })
                .unwrap();
            }
        };
        let t1 = std::thread::spawn(move || worker(repo_a, event_a));
        let t2 = std::thread::spawn(move || worker(repo_b, event_b));
        t1.join().unwrap();
        t2.join().unwrap();
        let state = codex_load_state(&repo, &event).unwrap().unwrap();
        // flock on macOS has known edge cases with concurrent threads;
        // accept 1999-2000 to avoid flaky test failures.
        assert!(
            state.seq >= 1999 && state.seq <= 2000,
            "concurrent seq should be 1999 or 2000, got {}",
            state.seq
        );
    }

    #[test]
    fn userpromptsubmit_simple_prompt_records_only_telemetry() {
        let repo = fresh_repo();
        let event = json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "test-p0a-simple",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt": "just a simple question about coding"
        });
        let _ = run_gate(&repo, &event).unwrap();
        let state = codex_load_state(&repo, &event).unwrap().unwrap();
        assert_eq!(state.seq, 1);
        assert!(!state.review_subagent_seen);
    }

    #[test]
    fn userpromptsubmit_review_prompt_records_gate_requirement() {
        let repo = fresh_repo();
        let event = json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "test-p0a-review",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt": "please do a deep code review of this module"
        });
        let _ = run_gate(&repo, &event).unwrap();
        let state = codex_load_state(&repo, &event).unwrap().unwrap();
        assert_eq!(state.seq, 1);
        assert!(state.review_required);
        assert!(!state.review_subagent_seen);
    }

    // P0-B: protected prefix tests
    #[test]
    fn protected_prefixes_cover_skill_files_and_registry() {
        assert!(
            router_rs::hook_common::path_guard::classify_protected_path(
                "skills/SKILL_ROUTING_RUNTIME.json",
                None,
                None,
                None
            )
            .is_some(),
            "SKILL_ROUTING_RUNTIME.json should be protected"
        );
        assert!(
            router_rs::hook_common::path_guard::classify_protected_path(
                "skills/SKILL_MANIFEST.json",
                None,
                None,
                None
            )
            .is_some(),
            "SKILL_MANIFEST.json should be protected"
        );
        assert!(
            router_rs::hook_common::path_guard::classify_protected_path(
                "configs/framework/RUNTIME_REGISTRY.json",
                None,
                None,
                None
            )
            .is_some(),
            "RUNTIME_REGISTRY.json should be protected"
        );
        assert!(
            router_rs::hook_common::path_guard::classify_protected_path(
                "skills/other_file.json",
                None,
                None,
                None
            )
            .is_none(),
            "non-SKILL_ prefixed file should not be protected"
        );
    }

    // P1-B: CODEX_SESSION_ID env var fallback test
    #[test]
    fn codex_session_key_uses_codex_session_id_env_when_no_event_fields() {
        let _guard = env_lock();
        // Use a unique env-var value to avoid cross-test pollution.
        let unique_id = format!(
            "test-stable-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        );
        let event = json!({});
        let repo = fresh_repo();
        std::env::set_var("CODEX_SESSION_ID", &unique_id);
        let a = codex_session_key(&repo, &event);
        let b = codex_session_key(&repo, &event);
        std::env::remove_var("CODEX_SESSION_ID");
        assert_eq!(a, b, "env var fallback should produce a stable key");
        assert!(
            a.chars().all(|c| c.is_ascii_hexdigit()),
            "key should be hex"
        );
        assert_eq!(a.len(), 32, "key should be 32 hex chars");
    }

    #[test]
    fn codex_session_key_matches_for_session_id_camel_case() {
        let repo = fresh_repo();
        let sid = "sess-key-camel-01";
        let snake = codex_session_key(&repo, &json!({"session_id": sid}));
        let camel = codex_session_key(&repo, &json!({"sessionId": sid}));
        assert_eq!(snake, camel);
    }

    #[test]
    fn codex_session_key_uses_codex_conversation_id_env_when_no_event_fields() {
        let _guard = env_lock();
        let unique_id = format!(
            "test-conv-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        );
        let event = json!({});
        std::env::remove_var("CODEX_SESSION_ID");
        let repo = fresh_repo();
        std::env::set_var("CODEX_CONVERSATION_ID", &unique_id);
        let a = codex_session_key(&repo, &event);
        let b = codex_session_key(&repo, &event);
        std::env::remove_var("CODEX_CONVERSATION_ID");
        assert_eq!(a, b, "CODEX_CONVERSATION_ID fallback should be stable");
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn strict_stable_session_key_blocks_userpromptsubmit_without_identifier() {
        let _guard = env_lock();
        std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "1");
        std::env::remove_var("CODEX_SESSION_ID");
        std::env::remove_var("CODEX_CONVERSATION_ID");
        let repo = fresh_repo();
        let event = json!({
            "hook_event_name": "UserPromptSubmit",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt": "hello"
        });
        let out = super::run_codex_lifecycle_context_hook(&repo, &event)
            .unwrap()
            .unwrap();
        assert_eq!(out["decision"], json!("block"));
        std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY");
    }

    #[test]
    fn strict_stable_session_key_allows_sessionstart_without_identifier() {
        let _guard = env_lock();
        std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "1");
        std::env::remove_var("CODEX_SESSION_ID");
        std::env::remove_var("CODEX_CONVERSATION_ID");
        let repo = fresh_repo();
        let event = json!({
            "hook_event_name": "SessionStart",
            "source": "startup"
        });
        let out = super::run_codex_lifecycle_context_hook(&repo, &event)
            .unwrap()
            .expect("sessionstart output");
        assert!(out.get("hookSpecificOutput").is_some());
        std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY");
    }

    #[test]
    fn strict_stable_session_key_off_allows_userpromptsubmit_without_identifier() {
        let _guard = env_lock();
        std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "0");
        std::env::remove_var("CODEX_SESSION_ID");
        std::env::remove_var("CODEX_CONVERSATION_ID");
        let repo = fresh_repo();
        let event = json!({
            "hook_event_name": "UserPromptSubmit",
            "cwd": repo.to_string_lossy().to_string(),
            "prompt": "hello"
        });
        let out = super::run_codex_lifecycle_context_hook(&repo, &event).unwrap();
        assert!(
            !matches!(out, Some(ref v) if v.get("decision") == Some(&json!("block"))),
            "unexpected lifecycle block when strict mode off"
        );
    }

    // P1-C: prune_stale_hook_state_files test
    #[test]
    fn prune_removes_excess_files_over_limit() {
        let repo = fresh_repo();
        let state_dir = repo.join(".codex/hook-state");
        // Create 60 fake review-subagent JSON files
        for i in 0..60u64 {
            let name = format!("review-subagent-{:032x}.json", i);
            fs::write(state_dir.join(&name), "{}").unwrap();
        }
        prune_stale_hook_state_files(&state_dir);
        let count = fs::read_dir(&state_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name();
                let s = n.to_string_lossy();
                s.starts_with("review-subagent-") && s.ends_with(".json")
            })
            .count();
        assert!(
            count <= 50,
            "after pruning, at most 50 files should remain, got {count}"
        );
    }
}
