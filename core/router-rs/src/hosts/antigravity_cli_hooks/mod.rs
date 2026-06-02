use crate::codex_hooks::{
    build_hook_binary_preamble, hooks_install_acquire_lock, hooks_install_serialize_pretty,
    hooks_install_sha256_hex, hooks_install_write_atomic, merge_lifecycle_install_hooks_json,
    read_hook_stdin_payload, run_codex_lifecycle_context_hook_for_state_dir,
    run_codex_pre_tool_use_hook, InstallMode, INSTALL_LIFECYCLE_EVENTS,
    ROUTER_RS_HOOK_PROJECTION_VERSION,
};
use crate::router_rs_observation::{attach_router_rs_observation, HookObservationHost};
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const ANTIGRAVITY_CLI_HOOKS_PATH: &str = ".antigravitycli/hooks.json";
pub const INSTALL_EVENTS: [&str; 7] = INSTALL_LIFECYCLE_EVENTS;
const ANTIGRAVITY_CLI_HOOK_AUTHORITY: &str = "rust-antigravity-cli-hooks";
const LIFECYCLE_STATE_DIR_LEAF: &str = ".antigravitycli";

pub fn resolve_antigravity_cli_home(arg: Option<&Path>) -> Result<PathBuf, String> {
    resolve_antigravity_cli_home_with_options(arg, true)
}

fn resolve_antigravity_cli_home_with_options(
    arg: Option<&Path>,
    create_if_missing: bool,
) -> Result<PathBuf, String> {
    if let Some(candidate) = arg {
        let absolute = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            env::current_dir()
                .map_err(|err| format!("Could not resolve current directory: {err}"))?
                .join(candidate)
        };
        if create_if_missing {
            fs::create_dir_all(&absolute).map_err(|err| {
                format!(
                    "Failed to create antigravity cli home {}: {err}",
                    absolute.display()
                )
            })?;
        } else if !absolute.exists() {
            return Err(format!(
                "Antigravity CLI home does not exist (check mode will not create it): {}",
                absolute.display()
            ));
        }
        return absolute.canonicalize().map_err(|err| {
            format!(
                "Failed to canonicalize antigravity cli home {}: {err}",
                absolute.display()
            )
        });
    }
    if let Ok(from_env) = env::var("ANTIGRAVITY_CLI_HOME") {
        if !from_env.trim().is_empty() {
            return resolve_antigravity_cli_home_with_options(
                Some(Path::new(from_env.trim())),
                create_if_missing,
            );
        }
    }
    let home = env::var("HOME")
        .map_err(|_| "ANTIGRAVITY_CLI_HOME unset and HOME unavailable".to_string())?;
    let default_home = Path::new(&home).join(".antigravitycli");
    resolve_antigravity_cli_home_with_options(Some(default_home.as_path()), create_if_missing)
}

pub fn install_antigravity_cli_hooks(
    antigravity_cli_home: &Path,
    repo_root: &Path,
    mode: InstallMode,
) -> Result<Value, String> {
    let apply = matches!(mode, InstallMode::Apply);
    let resolved_home =
        resolve_antigravity_cli_home_with_options(Some(antigravity_cli_home), apply)?;
    let resolved_repo_root = if repo_root.is_absolute() {
        repo_root.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|err| format!("Could not resolve current directory: {err}"))?
            .join(repo_root)
    };
    let resolved_repo_root = resolved_repo_root.canonicalize().map_err(|err| {
        format!(
            "Failed to canonicalize repo root {}: {err}",
            resolved_repo_root.display()
        )
    })?;
    if !resolved_repo_root.exists() {
        return Err(format!(
            "Repo root does not exist: {}",
            resolved_repo_root.display()
        ));
    }

    let hooks_path = resolved_home.join("hooks.json");
    let lifecycle_command = build_antigravity_lifecycle_hook_command();
    let hook_commands = INSTALL_EVENTS
        .iter()
        .map(|event| ((*event).to_string(), lifecycle_command.clone()))
        .collect::<BTreeMap<_, _>>();
    let command_digest =
        hooks_install_sha256_hex(&hooks_install_serialize_pretty(&json!(hook_commands))?);
    let _install_guard = if apply {
        Some(hooks_install_acquire_lock(&resolved_home)?)
    } else {
        None
    };

    let hooks_existed = hooks_path.exists();
    if apply
        && hooks_existed
        && fs::symlink_metadata(&hooks_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err(format!(
            "Refusing to update symlinked hooks.json: {}",
            hooks_path.display()
        ));
    }
    let hooks_text = fs::read_to_string(&hooks_path).ok();
    let hooks_value = if let Some(text) = hooks_text.as_deref() {
        Some(
            serde_json::from_str::<Value>(text)
                .map_err(|err| format!("Failed to parse {}: {err}", hooks_path.display()))?,
        )
    } else {
        None
    };
    let (merged_hooks, hooks_stat) =
        merge_lifecycle_install_hooks_json(hooks_value, &hook_commands, &INSTALL_EVENTS)?;
    let hooks_serialized = hooks_install_serialize_pretty(&merged_hooks)?;
    let hooks_changed = hooks_text.as_deref() != Some(hooks_serialized.as_str());
    let mut backup_path: Option<PathBuf> = None;

    if apply && hooks_changed {
        if let Some(parent) = hooks_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create hooks parent directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        if hooks_existed {
            let backup = PathBuf::from(format!(
                "{}.bak.{}",
                hooks_path.display(),
                Utc::now().format("%Y%m%d%H%M%S")
            ));
            fs::copy(&hooks_path, &backup).map_err(|err| {
                format!(
                    "Failed to backup hooks {} -> {}: {err}",
                    hooks_path.display(),
                    backup.display()
                )
            })?;
            backup_path = Some(backup);
        }
        let write_result = hooks_install_write_atomic(&hooks_path, &hooks_serialized);
        if let Err(err) = write_result {
            if let Some(backup) = backup_path.as_ref() {
                let _ = fs::copy(backup, &hooks_path);
            }
            return Err(err);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&hooks_path, fs::Permissions::from_mode(0o644));
        }
    }

    if apply {
        let manifest = json!({
            "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
            "command_digest": command_digest,
        });
        let manifest_text = hooks_install_serialize_pretty(&manifest)?;
        hooks_install_write_atomic(
            &resolved_home.join(".router-rs-install.manifest.json"),
            &manifest_text,
        )?;
    }

    Ok(json!({
        "schema_version": "router-rs-antigravity-cli-install-hooks-v1",
        "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
        "command_digest": command_digest,
        "authority": ANTIGRAVITY_CLI_HOOK_AUTHORITY,
        "antigravity_cli_home": resolved_home.to_string_lossy().into_owned(),
        "repo_root": resolved_repo_root.to_string_lossy().into_owned(),
        "project_hooks_path": ANTIGRAVITY_CLI_HOOKS_PATH,
        "applied": apply,
        "hooks_json": {
            "path": hooks_path.to_string_lossy().into_owned(),
            "status": install_mode_status(hooks_stat.status, mode),
            "events": INSTALL_EVENTS,
            "preserved_existing_entries": hooks_stat.preserved_existing_entries,
            "added_entries": hooks_stat.added_entries,
            "removed_legacy_entries": hooks_stat.removed_legacy_entries,
            "backup_path": backup_path.map(|v| v.to_string_lossy().into_owned()),
        },
        "hook_commands": hook_commands,
    }))
}

fn install_mode_status(status: &'static str, mode: InstallMode) -> &'static str {
    match mode {
        InstallMode::Apply => status,
        InstallMode::Check => match status {
            "created" => "would-create",
            "updated" => "would-update",
            "unchanged" => "would-leave-unchanged",
            _ => "would-update",
        },
    }
}

pub fn build_antigravity_lifecycle_hook_command() -> String {
    let mut command = build_hook_binary_preamble(
        "ANTIGRAVITY_CLI_PROJECT_ROOT",
        "ANTIGRAVITY_CLI_PROJECT_ROOT",
        "printf '%s\\n' '{\"decision\":\"block\",\"message\":\"router-rs binary unavailable for Antigravity CLI hook\",\"reason\":\"router-rs binary unavailable; fail-closed instead of silently bypassing critical hook enforcement\"}'; exit 1",
    );
    command.push_str(
        "\"$RS_BIN\" host antigravity-cli hook lifecycle-context --repo-root \"$ANTIGRAVITY_CLI_PROJECT_ROOT\"",
    );
    command
}

pub fn run_antigravity_cli_hook(command: &str, repo_root: &Path) -> Result<Option<Value>, String> {
    let _registry_guard = crate::runtime_registry::HookRegistryRepoGuard::new(repo_root);
    let canonical = canonical_antigravity_cli_hook_command(command)?;
    let payload = match read_hook_stdin_payload() {
        Ok(payload) => payload,
        Err(err) => {
            let message = format!("Antigravity CLI hook input JSON invalid: {err}");
            return Ok(attach_antigravity_hook_observation(Some(lifecycle_input_error(
                &message,
            ))));
        }
    };
    let out = match canonical {
        "lifecycle-context" => run_codex_lifecycle_context_hook_for_state_dir(
            repo_root,
            &payload,
            LIFECYCLE_STATE_DIR_LEAF,
        )?,
        "pre-tool-use" => run_codex_pre_tool_use_hook(repo_root, &payload)?,
        _ => return Err(format!("Unsupported Antigravity CLI hook command: {command}")),
    };
    Ok(attach_antigravity_hook_observation(out))
}

fn canonical_antigravity_cli_hook_command(command: &str) -> Result<&'static str, String> {
    match command.trim().to_ascii_lowercase().as_str() {
        "lifecycle-context" | "review-subagent-gate" => Ok("lifecycle-context"),
        "pre-tool-use" | "pretooluse" => Ok("pre-tool-use"),
        "sessionstart" | "userpromptsubmit" | "posttooluse" | "stop" => Ok("lifecycle-context"),
        other => Err(format!("Unsupported Antigravity CLI hook command: {other}")),
    }
}

fn lifecycle_input_error(message: &str) -> Value {
    json!({
        "decision": "block",
        "message": message,
        "reason": message,
        "hookSpecificOutput": {
            "hookEventName": "AntigravityCliLifecycleContext",
            "permissionDecision": "deny",
            "permissionDecisionReason": message,
        },
    })
}

fn attach_antigravity_hook_observation(mut value: Option<Value>) -> Option<Value> {
    if let Some(ref mut v) = value {
        attach_router_rs_observation(v, HookObservationHost::AntigravityCli);
    }
    value
}

pub fn expected_antigravity_cli_install_command_digest() -> String {
    let lifecycle_command = build_antigravity_lifecycle_hook_command();
    let hook_commands = INSTALL_EVENTS
        .iter()
        .map(|event| ((*event).to_string(), lifecycle_command.clone()))
        .collect::<BTreeMap<_, _>>();
    hooks_install_sha256_hex(
        &hooks_install_serialize_pretty(&json!(hook_commands)).expect("hook_commands json"),
    )
}

pub fn antigravity_cli_hooks_install_status(cli_home: &Path) -> Value {
    let hooks_path = cli_home.join("hooks.json");
    let manifest_path = cli_home.join(".router-rs-install.manifest.json");
    let hooks_exists = hooks_path.is_file();
    let manifest_exists = manifest_path.is_file();
    let digest_ok = if manifest_exists {
        fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|manifest| manifest.get("command_digest").and_then(Value::as_str).map(String::from))
            .is_some_and(|digest| digest == expected_antigravity_cli_install_command_digest())
    } else {
        false
    };
    json!({
        "path": hooks_path.to_string_lossy(),
        "exists": hooks_exists,
        "managed": hooks_exists && manifest_exists && digest_ok,
        "install_manifest": {
            "path": manifest_path.to_string_lossy(),
            "exists": manifest_exists,
            "digest_matches": digest_ok,
        },
    })
}

const ANTIGRAVITY_CLI_HOOK_COMMAND_MARKER: &str = "host antigravity-cli hook";

fn is_managed_antigravity_cli_hook_command(command: &str, lifecycle_command: &str) -> bool {
    command == lifecycle_command || command.contains(ANTIGRAVITY_CLI_HOOK_COMMAND_MARKER)
}

fn remove_router_rs_hook_entries_from_hooks_json(mut data: Value, lifecycle_command: &str) -> (Value, usize) {
    let mut removed = 0usize;
    let Some(hooks_root) = data
        .as_object_mut()
        .and_then(|root| root.get_mut("hooks"))
        .and_then(Value::as_object_mut)
    else {
        return (data, removed);
    };
    for event in INSTALL_EVENTS {
        let Some(entries) = hooks_root
            .get_mut(event)
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = entries.len();
        entries.retain(|entry| {
            !entry
                .as_object()
                .and_then(|obj| obj.get("hooks"))
                .and_then(Value::as_array)
                .is_some_and(|hooks| {
                    hooks.iter().any(|hook| {
                        hook.as_object().is_some_and(|hook_obj| {
                            hook_obj.get("type").and_then(Value::as_str) == Some("command")
                                && hook_obj
                                    .get("command")
                                    .and_then(Value::as_str)
                                    .is_some_and(|cmd| {
                                        is_managed_antigravity_cli_hook_command(
                                            cmd, lifecycle_command,
                                        )
                                    })
                        })
                    })
                })
        });
        removed += before.saturating_sub(entries.len());
    }
    (data, removed)
}

pub fn remove_antigravity_cli_router_hooks(
    antigravity_cli_home: &Path,
    dry_run: bool,
) -> Result<Value, String> {
    let hooks_path = antigravity_cli_home.join("hooks.json");
    let manifest_path = antigravity_cli_home.join(".router-rs-install.manifest.json");
    if !hooks_path.is_file() && !manifest_path.is_file() {
        return Ok(json!({
            "status": "not-installed",
            "changed": false,
            "dry_run": dry_run,
            "removed_entries": 0,
        }));
    }
    let lifecycle_command = build_antigravity_lifecycle_hook_command();
    let mut changed = false;
    let mut removed_entries = 0usize;
    if hooks_path.is_file() {
        let text = fs::read_to_string(&hooks_path)
            .map_err(|err| format!("Failed to read {}: {err}", hooks_path.display()))?;
        let existing: Value = if text.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&text)
                .map_err(|err| format!("Failed to parse {}: {err}", hooks_path.display()))?
        };
        let (merged, removed) =
            remove_router_rs_hook_entries_from_hooks_json(existing, &lifecycle_command);
        removed_entries = removed;
        if removed > 0 && !dry_run {
            if fs::symlink_metadata(&hooks_path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                return Err(format!(
                    "Refusing to update symlinked hooks.json: {}",
                    hooks_path.display()
                ));
            }
            let serialized = hooks_install_serialize_pretty(&merged)?;
            hooks_install_write_atomic(&hooks_path, &serialized)?;
            changed = true;
        }
    }
    if manifest_path.is_file() && !dry_run && removed_entries > 0 {
        fs::remove_file(&manifest_path).map_err(|err| {
            format!(
                "Failed to remove install manifest {}: {err}",
                manifest_path.display()
            )
        })?;
        changed = true;
    }
    Ok(json!({
        "status": if dry_run && removed_entries > 0 {
            "would-remove"
        } else if changed {
            "removed"
        } else if manifest_path.is_file() && removed_entries == 0 {
            "unchanged"
        } else {
            "not-installed-or-user-owned"
        },
        "changed": changed,
        "dry_run": dry_run,
        "removed_entries": removed_entries,
        "hooks_path": hooks_path.to_string_lossy(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static INSTALL_SEQ: AtomicU64 = AtomicU64::new(0);

    fn fresh_home(label: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "antigravity-cli-hooks-{}-{}-{}",
            label,
            std::process::id(),
            INSTALL_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn install_merge_is_idempotent() {
        let home = fresh_home("idempotent");
        let payload1 =
            install_antigravity_cli_hooks(&home, Path::new("."), InstallMode::Apply).unwrap();
        let text1 = fs::read_to_string(home.join("hooks.json")).unwrap();
        let payload2 =
            install_antigravity_cli_hooks(&home, Path::new("."), InstallMode::Apply).unwrap();
        let text2 = fs::read_to_string(home.join("hooks.json")).unwrap();
        assert_eq!(text1, text2);
        assert_eq!(payload1["hooks_json"]["status"].as_str(), Some("created"));
        assert_eq!(
            payload2["hooks_json"]["status"].as_str(),
            Some("unchanged")
        );
        assert_eq!(
            payload2["hooks_json"]["added_entries"].as_u64(),
            Some(0)
        );
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn lifecycle_command_uses_host_antigravity_cli() {
        let cmd = build_antigravity_lifecycle_hook_command();
        assert!(cmd.contains("host antigravity-cli hook lifecycle-context"));
    }

    #[test]
    fn lifecycle_context_routes_state_to_antigravitycli_dir() {
        let repo = fresh_home("lifecycle-state");
        fs::create_dir_all(repo.join(".antigravitycli")).unwrap();
        let payload = json!({
            "hook_event_name": "Stop",
            "session_id": "ag-cli-1",
            "cwd": repo.to_string_lossy(),
            "prompt": "全面 review 这个仓库"
        });
        let out = attach_antigravity_hook_observation(
            run_codex_lifecycle_context_hook_for_state_dir(
                &repo,
                &payload,
                LIFECYCLE_STATE_DIR_LEAF,
            )
            .expect("lifecycle hook"),
        );
        assert!(out.is_some());
        let obs = out
            .as_ref()
            .and_then(|v| v.get("router_rs_observation"))
            .expect("observation attached");
        assert_eq!(obs.get("host").and_then(Value::as_str), Some("antigravity-cli"));
    }

    #[test]
    fn install_remove_roundtrip_clears_router_hooks() {
        let home = fresh_home("remove-roundtrip");
        let repo = home.parent().unwrap().join("repo");
        fs::create_dir_all(&repo).unwrap();
        install_antigravity_cli_hooks(&home, &repo, InstallMode::Apply).unwrap();
        assert!(home.join("hooks.json").is_file());
        assert!(home.join(".router-rs-install.manifest.json").is_file());
        let status = antigravity_cli_hooks_install_status(&home);
        assert_eq!(status["managed"].as_bool(), Some(true));
        let removal = remove_antigravity_cli_router_hooks(&home, false).unwrap();
        assert!(removal["removed_entries"].as_u64().unwrap_or(0) > 0);
        let status_after = antigravity_cli_hooks_install_status(&home);
        assert_eq!(status_after["managed"].as_bool(), Some(false));
        assert!(!home.join(".router-rs-install.manifest.json").exists());
        let _ = fs::remove_dir_all(home.parent().unwrap());
    }

    #[test]
    fn install_check_does_not_create_home_directory() {
        let base = std::env::temp_dir().join(format!(
            "antigravity-cli-check-{}-{}",
            std::process::id(),
            INSTALL_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let missing_home = base.join("missing-cli-home");
        assert!(!missing_home.exists());
        let err = install_antigravity_cli_hooks(&missing_home, Path::new("."), InstallMode::Check)
            .expect_err("check must not mkdir");
        assert!(
            err.contains("does not exist") || err.contains("check mode"),
            "unexpected err: {err}"
        );
        assert!(!missing_home.exists());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn remove_does_not_drop_manifest_when_no_entries_removed() {
        let home = fresh_home("manifest-kept");
        let repo = home.parent().unwrap().join("repo-manifest");
        fs::create_dir_all(&repo).unwrap();
        install_antigravity_cli_hooks(&home, &repo, InstallMode::Apply).unwrap();
        let hooks_path = home.join("hooks.json");
        fs::write(&hooks_path, r#"{"hooks":{"Stop":[]}}"#).unwrap();
        let removal = remove_antigravity_cli_router_hooks(&home, false).unwrap();
        assert_eq!(removal["removed_entries"].as_u64(), Some(0));
        assert_eq!(removal["status"].as_str(), Some("unchanged"));
        assert!(home.join(".router-rs-install.manifest.json").is_file());
        let _ = fs::remove_dir_all(home.parent().unwrap());
    }

    #[test]
    #[cfg(unix)]
    fn remove_refuses_symlink_hooks_json() {
        use std::os::unix::fs::symlink;
        let home = fresh_home("symlink-remove");
        let repo = home.parent().unwrap().join("repo-symlink");
        fs::create_dir_all(&repo).unwrap();
        install_antigravity_cli_hooks(&home, &repo, InstallMode::Apply).unwrap();
        let hooks_path = home.join("hooks.json");
        let real = home.join("hooks.real.json");
        fs::rename(&hooks_path, &real).unwrap();
        symlink(&real, &hooks_path).unwrap();
        let err = remove_antigravity_cli_router_hooks(&home, false).expect_err("symlink");
        assert!(err.contains("symlink"), "err={err}");
        assert!(home.join(".router-rs-install.manifest.json").is_file());
        let _ = fs::remove_dir_all(home.parent().unwrap());
    }
}
