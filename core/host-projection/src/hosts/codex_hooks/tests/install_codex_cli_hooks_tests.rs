use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

static INSTALL_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_path(label: &str) -> PathBuf {
    ensure_test_deps();
    let base = std::env::temp_dir().join(format!(
        "install-codex-cli-hooks-{}-{}-{}",
        label,
        std::process::id(),
        INSTALL_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&base).unwrap();
    base
}

fn run_install(codex_home: &Path, repo_root: &Path, mode: InstallMode) -> Value {
    install_codex_cli_hooks(codex_home, repo_root, mode).unwrap()
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
    assert!(
        payload["command_digest"]
            .as_str()
            .is_some_and(|v| v.len() == 64)
    );
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
    let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
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
    let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
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
    let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
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
    let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
    assert!(result.is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn install_hooks_invalid_root_returns_error() {
    let result = merge_hooks_json(Some(json!([])), &install_hook_commands(Path::new(".")));
    assert!(
        result
            .err()
            .unwrap_or_default()
            .contains("root type: expected object")
    );
}

#[test]
fn install_hooks_invalid_hooks_field_returns_error() {
    let result = merge_hooks_json(
        Some(json!({"hooks":"not-an-object"})),
        &install_hook_commands(Path::new(".")),
    );
    assert!(
        result
            .err()
            .unwrap_or_default()
            .contains("`hooks` must be an object")
    );
}

#[test]
fn install_hooks_invalid_event_array_returns_error() {
    let result = merge_hooks_json(
        Some(json!({"hooks":{"Stop":{"x":1}}})),
        &install_hook_commands(Path::new(".")),
    );
    assert!(
        result
            .err()
            .unwrap_or_default()
            .contains("hooks.Stop must be an array")
    );
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
    let err = read_codex_stdin_limited(&mut cursor).unwrap_err();
    assert!(err.contains("exceeds 4 MiB"));
}

#[test]
fn codex_hook_rejects_invalid_utf8_stdin() {
    let bytes = vec![0xff, 0xfe, 0xfd];
    let mut cursor = std::io::Cursor::new(bytes);
    let err = read_codex_stdin_limited(&mut cursor).unwrap_err();
    assert_eq!(err, "stdin_invalid_utf8");
}

#[test]
fn codex_hook_rejects_truncated_utf8_sequence_stdin() {
    let mut buf = vec![b'a'; 64];
    buf.push(0x80);
    let mut cursor = std::io::Cursor::new(buf);
    let err = read_codex_stdin_limited(&mut cursor).unwrap_err();
    assert_eq!(err, "stdin_invalid_utf8");
}
