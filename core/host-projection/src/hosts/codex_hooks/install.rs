use router_rs::host_entrypoint_sync::HostEntrypointPayloadProvider;
use router_rs::host_integration::ensure_codex_skill_surface;
use chrono::Utc;
use router_rs::framework_error::{FrameworkError, FrameworkResult, ResultExt};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::audit::sha256_hex;
use super::lifecycle::{
    codex_hook_command_timeout_secs, HooksMergeStat, InstallMode,
};
use super::policy_embed::build_codex_agent_policy;
use super::state::{lock_is_stale, CodexHookError};
use super::{
    CodexLifecycleHostKind, CODEX_AGENT_POLICY_PATH, CODEX_HOOK_AUTHORITY, CODEX_HOOKS_PATH,
    CODEX_HOOKS_README_PATH, HOST_ENTRYPOINT_JSON_RELATIVE_PATHS, HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
    INSTALL_EVENTS, INSTALL_STATUS_ANTIGRAVITY_POST_TOOL, INSTALL_STATUS_ANTIGRAVITY_PRE_TOOL,
    INSTALL_STATUS_ANTIGRAVITY_SESSION_START, INSTALL_STATUS_ANTIGRAVITY_STOP,
    INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_START, INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_STOP,
    INSTALL_STATUS_ANTIGRAVITY_USER_PROMPT, INSTALL_STATUS_POST_TOOL, INSTALL_STATUS_PRE_TOOL,
    INSTALL_STATUS_SESSION_START, INSTALL_STATUS_STOP, INSTALL_STATUS_SUBAGENT_START,
    INSTALL_STATUS_SUBAGENT_STOP, INSTALL_STATUS_USER_PROMPT, ROUTER_RS_HOOK_PROJECTION_VERSION,
    ATOMIC_WRITE_NONCE,
};
#[cfg(test)]
use super::FORCE_ATOMIC_WRITE_FAIL;

pub fn build_codex_hook_manifest() -> Value {
    let mut hooks = serde_json::Map::new();
    for event in INSTALL_EVENTS {
        let timeout = codex_hook_command_timeout_secs(CodexLifecycleHostKind::CODEX, event);
        let hook = json!({
            "type": "command",
            "command": build_project_hook_command(event),
            "timeout": timeout,
            "statusMessage": hook_event_status_message(CodexLifecycleHostKind::CODEX, event),
        });
        let mut entry = json!({
            "hooks": [hook],
        });
        if event == "SessionStart" {
            entry["matcher"] = json!("startup|resume|clear");
        }
        hooks.insert(event.to_string(), json!([entry]));
    }
    json!({
        "version": 1,
        "_comment": "Managed by router-rs. Regenerate with `cargo run --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root \"$PWD\"`.",
        "hooks": hooks,
    })
}

pub fn protected_generated_paths() -> Vec<&'static str> {
    router_rs::hook_common::path_guard::generated_host_entrypoint_paths()
}

/// Codex hook install projection. `codex_audit_commands.legacy_review_subagent_gate` is a
/// stable JSON key alias only: the CLI subcommand is `review-subagent-gate`, which maps to the
/// same `lifecycle-context` handler as `codex_lifecycle_context` (not a separate gate).
pub fn build_codex_hook_projection() -> Value {
    json!({
        "schema_version": "router-rs-codex-hook-projection-v1",
        "authority": CODEX_HOOK_AUTHORITY,
        "codex_agent_policy": build_codex_agent_policy(),
        "codex_hooks_readme": build_codex_hooks_readme(),
        "codex_hooks": build_codex_hook_manifest(),
        "codex_audit_commands": {
            "pre_tool_use": build_codex_hook_command("--event=PreToolUse"),
            "contract_guard": build_codex_hook_command("contract-guard"),
            "codex_lifecycle_context": build_codex_hook_command("lifecycle-context"),
            "legacy_review_subagent_gate": build_codex_hook_command("review-subagent-gate"),
        },
    })
}

pub fn build_codex_hooks_readme() -> String {
    "# Codex Hooks Projection\n\n\
Codex hooks are enabled for this repo and are managed by the Rust `router-rs` control plane.\n\n\
<!-- managed_by: router-rs codex sync -->\n\n\
**Policy snapshot:** the `codex_agent_policy` payload embeds repository `AGENTS.md` + `AGENTS_CODEX.md` at **router-rs compile time** (`include_str!`), not from disk on each hook run. `codex sync` / `framework sync-entrypoints` materialize **`AGENTS_CODEX.md`** and **`.codex/README.md`** (see `.codex/host_entrypoints_sync_manifest.json`); an existing `AGENTS_CODEX.md` on disk is preserved. When the delta file is missing, sync bootstraps **delta-only** content (not a merged kernel+delta blob). Rebuild before sync when hook payloads must carry policy edits (see `AGENTS_CODEX.md` → **Codex 构建快照与同步逻辑**).\n\n\
Project-local `.codex/hooks.json` uses the official Codex lifecycle surface: `SessionStart`, `PreToolUse`, `UserPromptSubmit`, `PostToolUse`, and `Stop`.\n\n\
Feature enablement uses `[features] hooks = true`; older public examples may still show `codex_hooks`, which this repository treats as a deprecated compatibility key and rewrites to `hooks`.\n\n\
`SessionStart` injects a lightweight workspace pointer (`Repo:` and optional `source`) when operator inject is enabled; it does **not** inject a continuity digest or hook-driven `GOAL_CONTINUE`. `UserPromptSubmit` injects only trigger-specific context. `PreToolUse` blocks direct edits to generated Codex surfaces. `PostToolUse` records subagent/tool telemetry and, when opted in (`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1`, default off), may append verification-like shell commands (for example `cargo test`) to `EVIDENCE_INDEX.json` when continuity is active. `Stop` enforces closeout and `CODEX_REVIEW_GATE` (wave-2 partial: broad/deep review requires PostTool countable deep-lane evidence **and** Stop compact findings, or bounded `rg_clear` / reject tokens; compact alone cannot clear); set **`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1`** to disable the hard gate (unset keeps enabled). `my-light` lifecycle (`/discussx|planx|implementx|verifyx` or `GOAL_STATE.lifecycle_profile`) suppresses the hard block. It does **not** write an automatic continuity checkpoint (`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT` is a no-op). Resume work via `/implementx`, `framework_goal_drive` stdio, and manual boards under `artifacts/current/<task_id>/`. Durable cleanup should use explicit session-artifact or snapshot commands rather than an extra end-of-session hook.\n\n\
Hook state is transient and lives under `.codex/hook-state/` in the current repository while the session is active. Stable keys require `session_id` / `conversation_id` / `thread_id` in hook payloads (snake_case **or** camelCase, e.g. `sessionId`) or `CODEX_SESSION_ID` / `CODEX_CONVERSATION_ID` in the environment; otherwise hook-state may not persist across invocations (router-rs logs a one-time stderr warning per process).\n\n\
**`ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY`** defaults **on** (`unset` = require stable keys). Set `0`/`false`/`off`/`no` for legacy payloads without `session_id` / env fallbacks (`SessionStart` is unaffected). Without a stable id and with strict mode off, hook-state uses a deterministic fallback keyed by **repo + cwd** (optional `ROUTER_RS_CODEX_HOOK_STATE_SALT`), not a single global file per machine.\n\n\
**`ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`** (default on): deep lane + omitted `fork_context` counts as independent reviewer evidence on PostTool. Set `0`/`false`/`off`/`no` to require explicit JSON `fork_context: false`.\n\n\
Generated hook commands resolve `router-rs` in order: **`ROUTER_RS_BIN`** when set to an executable path, then `core/router-rs/target/{release,debug}/router-rs`, then repo `target/{release,debug}/router-rs`, finally `command -v router-rs` (last resort — prefer pinning `ROUTER_RS_BIN` or building into the repo). If the binary is missing, **all** lifecycle hooks fail closed with a JSON `decision:block` line.\n\n\
Merged `additionalContext` for SessionStart/UserPromptSubmit is capped by UTF-8 **byte** length (not Unicode character count). Tune with `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` or legacy `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` (same semantics; clamped 256–8192; default 640 bytes).\n\n\
Successful Codex hook processes always print one JSON object line on stdout (including `{}` when there is no hook-specific output).\n\n\
Stop hook blocks when `.codex/hook-state` cannot be read or parsed (non-recoverable JSON/IO): fix permissions or delete corrupted state files before continuing.\n\n\
Use `cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint install-codex-user-hooks` when you want to install the same Codex hook projection into a user-level `~/.codex/hooks.json`. The installer keeps existing hooks and idempotently appends the managed command hook without replacing unrelated handlers.\n\n\
Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.\n\n\
Regenerate with:\n\n\
```sh\n\
cargo run --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root \"$PWD\"\n\
```\n\n\
Steady-state documentation map: `docs/README.md`.\n"
        .to_string()
}

pub fn codex_host_entrypoint_provider(
    repo_root: &Path,
) -> FrameworkResult<HostEntrypointPayloadProvider> {
    let mut files = BTreeMap::new();
    let policy_path = repo_root.join(CODEX_AGENT_POLICY_PATH);
    let policy = match fs::read(&policy_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            include_str!("../../../../../AGENTS_CODEX.md").as_bytes().to_vec()
        }
        Err(err) => {
            return Err(FrameworkError::other(format!(
                "failed to read {}: {err}",
                policy_path.to_string_lossy()
            )));
        }
    };
    files.insert(CODEX_AGENT_POLICY_PATH.to_string(), policy);
    files.insert(
        CODEX_HOOKS_PATH.to_string(),
        serialize_pretty_json_bytes(&build_codex_hook_manifest())?,
    );
    files.insert(
        CODEX_HOOKS_README_PATH.to_string(),
        build_codex_hooks_readme().into_bytes(),
    );
    Ok(HostEntrypointPayloadProvider {
        files,
        json_relative_paths: HOST_ENTRYPOINT_JSON_RELATIVE_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect(),
        manifest_relative_path: HOST_ENTRYPOINT_SYNC_MANIFEST_PATH.to_string(),
        agent_policy_entrypoint: CODEX_AGENT_POLICY_PATH.to_string(),
        after_apply: Some(|repo_root| codex_host_entrypoint_after_apply(repo_root).map_err(|e| e.to_string())),
    })
}

pub fn codex_host_entrypoint_after_apply(repo_root: &Path) -> FrameworkResult<Value> {
    ensure_codex_skill_surface(repo_root).map_framework_err()
}

pub fn serialize_pretty_json_bytes(payload: &Value) -> FrameworkResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(payload)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn build_hook_binary_preamble(
    project_var: &str,
    env_var: &str,
    missing_binary_fallback: &str,
) -> String {
    let mut command = String::new();
    command.push_str(&format!(
        "{project_var}=\"${{{env_var}:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}\"; "
    ));
    command.push_str(&format!(
        "RS_BIN=\"\"; \
if [ -n \"${{ROUTER_RS_BIN:-}}\" ] && [ -x \"${{ROUTER_RS_BIN}}\" ]; then \
_CMDV=\"$(command -v router-rs 2>/dev/null || true)\"; \
if [ \"$ROUTER_RS_BIN\" = \"$_CMDV\" ] || [[ \"$ROUTER_RS_BIN\" == \"${project_var}/\"* ]]; then RS_BIN=\"${{ROUTER_RS_BIN}}\"; \
else echo \"[router-rs] ROUTER_RS_BIN rejected (not in repo or PATH): $ROUTER_RS_BIN\" >&2; fi; \
elif [ -x \"${project_var}/core/router-rs/target/release/router-rs\" ]; then RS_BIN=\"${project_var}/core/router-rs/target/release/router-rs\"; \
elif [ -x \"${project_var}/core/router-rs/target/debug/router-rs\" ]; then RS_BIN=\"${project_var}/core/router-rs/target/debug/router-rs\"; \
elif [ -x \"${project_var}/target/release/router-rs\" ]; then RS_BIN=\"${project_var}/target/release/router-rs\"; \
elif [ -x \"${project_var}/target/debug/router-rs\" ]; then RS_BIN=\"${project_var}/target/debug/router-rs\"; \
else RS_BIN=\"$(command -v router-rs 2>/dev/null || true)\"; fi; "
    ));
    command.push_str("if [ ! -x \"$RS_BIN\" ]; then ");
    command.push_str(missing_binary_fallback);
    command.push_str("; fi; ");
    command
}

pub fn build_codex_hook_command(event: &str) -> String {
    let mut command =
        build_hook_binary_preamble("CODEX_PROJECT_ROOT", "CODEX_PROJECT_ROOT", "printf '%s\\n' '{\"decision\":\"block\",\"message\":\"router-rs binary unavailable for Codex hook\",\"reason\":\"router-rs binary unavailable; fail-closed instead of silently bypassing critical hook enforcement\"}'; exit 1");
    command.push_str(&format!(
        "\"$RS_BIN\" codex hook {event} --repo-root \"$CODEX_PROJECT_ROOT\""
    ));
    command
}

pub fn build_project_hook_command(event: &str) -> String {
    build_install_hook_command(Path::new("."), event)
}

pub struct HooksInstallLock {
    path: PathBuf,
}

impl Drop for HooksInstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn acquire_install_lock(codex_home: &Path) -> FrameworkResult<HooksInstallLock> {
    let lock_path = codex_home.join(".install.lock");
    for _ in 0..30 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let stamp = format!("pid={} ts={now_ms}\n", std::process::id());
                use std::io::Write as _;
                file.write_all(stamp.as_bytes())
                    .map_err(CodexHookError::StateLockWrite)?;
                file.sync_all()
                    .map_err(CodexHookError::StateLockSync)?;
                return Ok(HooksInstallLock { path: lock_path });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(FrameworkError::other(format!("install_lock_acquire_failed: {err}"))),
        }
    }
    Err(FrameworkError::other("install_lock_timeout"))
}

pub fn projection_version_older(manifest_version: &str, current: &str) -> bool {
    fn parse(value: &str) -> Option<(u64, u64, u64)> {
        let cleaned = value.trim().trim_start_matches('v');
        let mut parts = cleaned.split('.');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ))
    }
    match (parse(manifest_version), parse(current)) {
        (Some(found), Some(expected)) => found < expected,
        _ => true,
    }
}

static DRIFT_CACHE: LazyLock<std::sync::Mutex<(std::time::Instant, Option<String>)>> =
    LazyLock::new(|| std::sync::Mutex::new((std::time::Instant::now() - std::time::Duration::from_secs(600), None)));

pub fn codex_projection_drift_warning(repo_root: &Path) -> Option<String> {
    const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);
    {
        let guard = DRIFT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if guard.0.elapsed() < CACHE_TTL {
            return guard.1.clone();
        }
    }
    let warning = "[router-rs] hook projection drift detected; consider re-running `router-rs framework maint install-codex-user-hooks`.".to_string();
    let local_codex_home = repo_root.join("codex-home");
    let manifest_path = if local_codex_home.is_dir() {
        local_codex_home.join(".router-rs-install.manifest.json")
    } else {
        let codex_home = env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))?;
        if !codex_home.is_dir() {
            return None;
        }
        codex_home.join(".router-rs-install.manifest.json")
    };
    let text = match fs::read_to_string(manifest_path) {
        Ok(v) => v,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(_) => return Some(warning),
    };
    let manifest: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Some(warning),
    };
    let projection = manifest
        .get("projection_version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let result = if projection_version_older(projection, ROUTER_RS_HOOK_PROJECTION_VERSION) {
        Some(warning)
    } else {
        None
    };
    if let Ok(mut guard) = DRIFT_CACHE.lock() {
        *guard = (std::time::Instant::now(), result.clone());
    }
    result
}

pub fn resolve_codex_home(arg: Option<&Path>) -> FrameworkResult<PathBuf> {
    let candidate = if let Some(path) = arg {
        path.to_path_buf()
    } else if let Some(path) = env::var_os("CODEX_HOME") {
        PathBuf::from(path)
    } else if let Some(home) = env::var_os("HOME") {
        PathBuf::from(home).join(".codex")
    } else {
        return Err(
            FrameworkError::other("Could not resolve codex home: missing --codex-home, CODEX_HOME, and HOME"),
        );
    };
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        env::current_dir()
            .map_err(|err| FrameworkError::other(format!("Could not resolve current directory: {err}")))?
            .join(candidate)
    };
    fs::create_dir_all(&absolute)
        .map_err(|err| FrameworkError::other(format!("Failed to create codex home {}: {err}", absolute.display())))?;
    absolute.canonicalize().map_err(|err| {
        FrameworkError::other(format!(
            "Failed to canonicalize codex home {}: {err}",
            absolute.display()
        ))
    })
}

pub fn install_codex_hooks(
    codex_home: &Path,
    repo_root: &Path,
    mode: InstallMode,
) -> router_rs::framework_error::FrameworkResult<Value> {
    let apply = matches!(mode, InstallMode::Apply);
    let resolved_codex_home = resolve_codex_home(Some(codex_home))?;
    let resolved_repo_root = if repo_root.is_absolute() {
        repo_root.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|err| FrameworkError::other(format!("Could not resolve current directory: {err}")))?
            .join(repo_root)
    };
    let resolved_repo_root = resolved_repo_root.canonicalize().map_err(|err| {
        format!(
            "Failed to canonicalize repo root {}: {err}",
            resolved_repo_root.display()
        )
    })?;
    if !resolved_repo_root.exists() {
        return Err(FrameworkError::other(format!(
            "Repo root does not exist: {}",
            resolved_repo_root.display()
        )));
    }

    let config_path = resolved_codex_home.join("config.toml");
    let hooks_path = resolved_codex_home.join("hooks.json");
    let hook_commands = INSTALL_EVENTS
        .iter()
        .map(|event| {
            (
                (*event).to_string(),
                build_install_hook_command(&resolved_repo_root, event),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let command_digest = sha256_hex(&serialize_ascii_json_pretty(&json!(hook_commands))?);
    let _install_guard: Option<HooksInstallLock> = if apply {
        Some(acquire_install_lock(&resolved_codex_home)?)
    } else {
        None
    };

    let existing_config = fs::read_to_string(&config_path).ok();
    let (merged_config, config_status) = merge_features_codex_hooks(existing_config.as_deref());
    let config_changed = existing_config.as_deref() != Some(merged_config.as_str());
    if apply && config_changed {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create config parent directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        write_atomic_text(&config_path, &merged_config)?;
    }

    let hooks_existed = hooks_path.exists();
    if apply
        && hooks_existed
        && fs::symlink_metadata(&hooks_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err(FrameworkError::other(format!(
            "Refusing to update symlinked hooks.json: {}",
            hooks_path.display()
        )));
    }
    let hooks_text = fs::read_to_string(&hooks_path).ok();
    let hooks_value = if let Some(text) = hooks_text.as_deref() {
        Some(
            serde_json::from_str::<Value>(text)
                .map_err(|err| FrameworkError::other(format!("Failed to parse {}: {err}", hooks_path.display())))?,
        )
    } else {
        None
    };
    let (merged_hooks, hooks_stat) = merge_hooks_json(hooks_value, &hook_commands)?;
    let hooks_serialized = serialize_ascii_json_pretty(&merged_hooks)?;
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
        let write_result = write_atomic_text(&hooks_path, &hooks_serialized);
        if let Err(err) = write_result {
            if let Some(backup) = backup_path.as_ref() {
                let _ = fs::copy(backup, &hooks_path);
            }
            return Err(FrameworkError::other(err));
        }
        if !hooks_existed {
            #[cfg(unix)]
            {
                let _ = fs::set_permissions(&hooks_path, fs::Permissions::from_mode(0o644));
            }
        }
    }
    if apply {
        let manifest = json!({
            "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
            "command_digest": command_digest,
        });
        let manifest_text = serialize_ascii_json_pretty(&manifest)?;
        write_atomic_text(
            &resolved_codex_home.join(".router-rs-install.manifest.json"),
            &manifest_text,
        )?;
    }

    Ok(json!({
        "schema_version": "router-rs-codex-install-hooks-v1",
        "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
        "command_digest": command_digest,
        "authority": "rust-codex-install-hooks",
        "codex_home": resolved_codex_home.to_string_lossy().into_owned(),
        "repo_root": resolved_repo_root.to_string_lossy().into_owned(),
        "applied": apply,
        "config_toml": {
            "path": config_path.to_string_lossy().into_owned(),
            "status": mode_status(config_status, mode),
        },
        "hooks_json": {
            "path": hooks_path.to_string_lossy().into_owned(),
            "status": mode_status(hooks_stat.status, mode),
            "events": INSTALL_EVENTS,
            "preserved_existing_entries": hooks_stat.preserved_existing_entries,
            "added_entries": hooks_stat.added_entries,
            "removed_legacy_entries": hooks_stat.removed_legacy_entries,
            "backup_path": backup_path.map(|v| v.to_string_lossy().into_owned()),
        },
        "hook_commands": hook_commands,
    }))
}

pub fn mode_status(status: &'static str, mode: InstallMode) -> &'static str {
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

/// Codex-side atomic write: thin wrapper on top of [`router_rs::atomic_write::write_atomic_text_to_temp`].
///
/// Two intentional differences vs [`router_rs::atomic_write::write_atomic_text`]:
///
/// 1. A `#[cfg(test)]` failure-injection seam (`FORCE_ATOMIC_WRITE_FAIL`) so the installer's
///    error-handling paths stay covered without faking filesystem failures.
/// 2. A unique hidden temp filename (`.<stem>.tmp-<pid>-<nanos>-<nonce>`). Codex installs run
///    against shared `~/.codex` paths where concurrent writers (multiple agents / sessions)
///    can collide on a plain `<ext>.tmp` sidecar; the unique nonce + pid scheme avoids that.
///
/// The actual durable-write algorithm (write → fsync → rename → fsync parent) lives in
/// `atomic_write`, so there is **one** source of truth for the IO sequence.
pub fn write_atomic_text(path: &Path, text: &str) -> Result<(), String> {
    #[cfg(test)]
    if FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.get()) {
        return Err("forced atomic write failure".to_string());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("atomic-write-target");
    let ts_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let nonce = ATOMIC_WRITE_NONCE.fetch_add(1, Ordering::Relaxed);



    let tmp_path = parent.join(format!(
        ".{stem}.tmp-{}-{ts_nanos}-{nonce}",
        std::process::id()
    ));
    router_rs::atomic_write::write_atomic_text_to_temp(path, text, &tmp_path)
}

pub fn serialize_ascii_json_pretty(value: &Value) -> FrameworkResult<String> {
    let pretty = serde_json::to_string_pretty(value)?;
    let mut out = String::with_capacity(pretty.len() + 1);
    for ch in pretty.chars() {
        if ch.is_ascii() {
            out.push(ch);
            continue;
        }
        let mut buf = [0u16; 2];
        for unit in ch.encode_utf16(&mut buf).iter() {
            out.push_str(&format!("\\u{:04x}", unit));
        }
    }
    out.push('\n');
    Ok(out)
}

pub fn hook_event_status_message(host: CodexLifecycleHostKind, event_name: &str) -> &'static str {
    match host.state_dir_leaf {
        ".antigravitycli" => match event_name {
            "SessionStart" => INSTALL_STATUS_ANTIGRAVITY_SESSION_START,
            "PreToolUse" => INSTALL_STATUS_ANTIGRAVITY_PRE_TOOL,
            "UserPromptSubmit" => INSTALL_STATUS_ANTIGRAVITY_USER_PROMPT,
            "PostToolUse" => INSTALL_STATUS_ANTIGRAVITY_POST_TOOL,
            "Stop" => INSTALL_STATUS_ANTIGRAVITY_STOP,
            "SubagentStart" => INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_START,
            "SubagentStop" => INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_STOP,
            _ => "",
        },
        _ => match event_name {
            "SessionStart" => INSTALL_STATUS_SESSION_START,
            "PreToolUse" => INSTALL_STATUS_PRE_TOOL,
            "UserPromptSubmit" => INSTALL_STATUS_USER_PROMPT,
            "PostToolUse" => INSTALL_STATUS_POST_TOOL,
            "Stop" => INSTALL_STATUS_STOP,
            "SubagentStart" => INSTALL_STATUS_SUBAGENT_START,
            "SubagentStop" => INSTALL_STATUS_SUBAGENT_STOP,
            _ => "",
        },
    }
}

pub fn build_install_hook_command(_repo_root: &Path, event: &str) -> String {
    let _ = _repo_root;
    format!(
        "/usr/bin/env bash \"${{SKILL_FRAMEWORK_ROOT:-${{CODEX_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}}}/configs/framework/codex-router-rs-hook.sh\" {event}"
    )
}

pub fn merge_features_codex_hooks(existing: Option<&str>) -> (String, &'static str) {
    match existing {
        None => ("[features]\nhooks = true\n\n".to_string(), "created"),
        Some(text) => {
            let lines = text.lines().collect::<Vec<_>>();
            let mut out = Vec::new();
            let mut in_features = false;
            let mut features_seen = false;
            let mut hooks_set = false;
            for line in lines {
                let stripped = line.trim();
                if stripped.starts_with('[') && stripped.ends_with(']') {
                    if in_features && !hooks_set {
                        out.push("hooks = true".to_string());
                        hooks_set = true;
                    }
                    in_features = stripped == "[features]";
                    if in_features {
                        features_seen = true;
                    }
                    out.push(line.to_string());
                    continue;
                }
                if in_features
                    && (is_named_setting(line, "codex_hooks") || is_named_setting(line, "hooks"))
                {
                    out.push("hooks = true".to_string());
                    hooks_set = true;
                } else {
                    out.push(line.to_string());
                }
            }
            if in_features && !hooks_set {
                out.push("hooks = true".to_string());
            }
            if !features_seen {
                if out.last().is_some_and(|line| !line.trim().is_empty()) {
                    out.push(String::new());
                }
                out.push("[features]".to_string());
                out.push("hooks = true".to_string());
            }
            let merged = format!("{}\n", out.join("\n").trim_end());
            let canonical_existing = format!("{}\n", text.trim_end());
            if (text.ends_with('\n') && merged == canonical_existing) || merged == text {
                (merged, "unchanged")
            } else {
                (merged, "updated")
            }
        }
    }
}

pub fn is_named_setting(line: &str, key: &str) -> bool {
    line.split_once('=')
        .map(|(name, _)| name.trim() == key)
        .unwrap_or(false)
}

pub fn merge_hooks_json(
    existing: Option<Value>,
    hook_commands: &BTreeMap<String, String>,
) -> FrameworkResult<(Value, HooksMergeStat)> {
    merge_hooks_json_for_events(CodexLifecycleHostKind::CODEX, existing, hook_commands, &INSTALL_EVENTS)
}

pub fn merge_hooks_json_for_events(
    host: CodexLifecycleHostKind,
    existing: Option<Value>,
    hook_commands: &BTreeMap<String, String>,
    events: &[&str],
) -> FrameworkResult<(Value, HooksMergeStat)> {
    let created = existing.is_none();
    let mut data = match existing {
        None => json!({}),
        Some(value) => {
            if !value.is_object() {
                return Err(FrameworkError::validation("Invalid hooks.json root type: expected object"));
            }
            value
        }
    };
    let root = data
        .as_object_mut()
        .ok_or_else(|| "Invalid hooks.json root type: expected object".to_string())?;
    if !root.contains_key("hooks") {
        root.insert("hooks".to_string(), json!({}));
    }
    let hooks_root = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Invalid hooks.json: `hooks` must be an object".to_string())?;

    let mut preserved_existing_entries = 0usize;
    let mut added_entries = 0usize;
    let mut removed_legacy_entries = 0usize;

    for event in events {
        let hook_command = hook_commands
            .get(*event)
            .ok_or_else(|| FrameworkError::validation(format!("Missing install hook command for event {event}")))?;
        if !hooks_root.contains_key(*event) {
            hooks_root.insert(event.to_string(), Value::Array(Vec::new()));
        }
        let entries = hooks_root
            .get_mut(*event)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| FrameworkError::validation(format!("Invalid hooks.json: hooks.{event} must be an array")))?;
        removed_legacy_entries += remove_legacy_python_codex_hooks(entries);
        preserved_existing_entries += entries.len();

        let exists = entries.iter().any(|entry| {
            entry
                .as_object()
                .and_then(|obj| obj.get("hooks"))
                .and_then(Value::as_array)
                .is_some_and(|hooks| {
                    hooks.iter().any(|hook| {
                        hook.as_object().is_some_and(|hook_obj| {
                            hook_obj.get("type").and_then(Value::as_str) == Some("command")
                                && hook_obj.get("command").and_then(Value::as_str)
                                    == Some(hook_command.as_str())
                        })
                    })
                })
        });
        if !exists {
            entries.push(json!({
                "hooks": [{
                    "type": "command",
                    "command": hook_command,
                    "timeout": codex_hook_command_timeout_secs(host, event),
                    "statusMessage": hook_event_status_message(host, event),
                }]
            }));
            added_entries += 1;
        }
    }
    let status = if created {
        "created"
    } else if added_entries > 0 || removed_legacy_entries > 0 {
        "updated"
    } else {
        "unchanged"
    };
    Ok((
        data,
        HooksMergeStat {
            status,
            preserved_existing_entries,
            added_entries,
            removed_legacy_entries,
        },
    ))
}

pub fn remove_legacy_python_codex_hooks(entries: &mut Vec<Value>) -> usize {
    let mut removed = 0usize;
    for entry in entries.iter_mut() {
        let Some(hooks) = entry
            .as_object_mut()
            .and_then(|obj| obj.get_mut("hooks"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = hooks.len();
        hooks.retain(|hook| {
            !hook
                .as_object()
                .and_then(|obj| obj.get("command"))
                .and_then(Value::as_str)
                .is_some_and(is_legacy_python_codex_hook_command)
        });
        removed += before.saturating_sub(hooks.len());
    }
    entries.retain(|entry| {
        entry
            .as_object()
            .and_then(|obj| obj.get("hooks"))
            .and_then(Value::as_array)
            .map_or(true, |hooks| !hooks.is_empty())
    });
    removed
}

pub fn is_legacy_python_codex_hook_command(command: &str) -> bool {
    command.contains("review_subagent_gate.py")
        || command.contains(".codex/hooks/review_subagent_gate.py")
}

#[allow(dead_code)]
pub fn attach_codex_hook_observation(mut value: Option<Value>) -> Option<Value> {
    if let Some(ref mut v) = value {
        router_rs::router_rs_observation::attach_router_rs_observation(
            v,
            router_rs::router_rs_observation::HookObservationHost::Codex,
        );
    }
    value
}
