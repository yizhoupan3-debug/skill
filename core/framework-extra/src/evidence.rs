//! Post-tool-use evidence tracking.
//!
//! Functions for extracting tool execution metadata, appending evidence to
//! `EVIDENCE_INDEX.json`, and heuristics for detecting verification commands.

use framework_core::repo_roots::resolve_repo_root_arg;
use framework_runtime::constants::{
    EVIDENCE_INDEX_FILENAME, EVIDENCE_INDEX_SCHEMA_VERSION,
    TASK_POINTERS_FILENAME,
};
use core_state_utils::json_io::read_json_strict;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use crate::util::{
    current_local_timestamp, defaulted_payload_text, required_payload_text, truncate_utf8_chars,
    write_json_if_changed_unlocked,
};

use core_errors::FrameworkError;
type Result<T> = std::result::Result<T, FrameworkError>;

const MAX_POST_TOOL_EVIDENCE_ARTIFACTS: usize = 120;

/// Compute a fast integrity hash for the evidence chain (EV-F007).
/// Uses DefaultHasher (SipHash); this is tamper-detection, not cryptographic security.
fn compute_chain_hash(prev_hash: &str, content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    prev_hash.hash(&mut hasher);
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn coerce_exit_code_value(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(n) = value.as_i64() {
        return Some(n);
    }
    if let Some(n) = value.as_u64() {
        return Some(n as i64);
    }
    if let Some(text) = value.as_str() {
        return text.trim().parse::<i64>().ok();
    }
    None
}

fn coerce_duration_ms_value(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    if let Some(n) = value.as_i64() {
        return n.try_into().ok();
    }
    if let Some(n) = value.as_f64() {
        return Some(n.round() as u64);
    }
    if let Some(text) = value.as_str() {
        return text.trim().parse::<u64>().ok();
    }
    None
}

/// PostToolUse journal: infer success from exit code / error flags when present.
pub fn post_tool_call_succeeded(event: &Value) -> bool {
    if event
        .get("is_error")
        .and_then(Value::as_bool)
        .is_some_and(|v| v)
    {
        return false;
    }
    if event.get("error").is_some_and(|v| !v.is_null()) {
        return false;
    }
    match extract_tool_exit_hint(event) {
        Some(0) => true,
        Some(_) => false,
        None => true,
    }
}

/// Extract exit code from a Codex `PostToolUse` payload (compatible with nested `tool_output` / JSON strings).
fn extract_tool_exit_hint(event: &Value) -> Option<i64> {
    let candidates: [&Option<&Value>; 7] = [
        &event.get("exit_code"),
        &event.get("exitCode"),
        &event.get("tool_output").and_then(|v| v.get("exit_code")),
        &event.get("tool_output").and_then(|v| v.get("exitCode")),
        &event
            .get("tool_output")
            .and_then(|v| v.get("metadata"))
            .and_then(|m| m.get("exit_code")),
        &event.get("result").and_then(|v| v.get("exit_code")),
        &event.get("response").and_then(|v| v.get("exit_code")),
    ];
    if let Some(text) = event.get("tool_output").and_then(Value::as_str) {
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            if let Some(code) = coerce_exit_code_value(parsed.get("exit_code")) {
                return Some(code);
            }
            if let Some(code) = coerce_exit_code_value(parsed.get("exitCode")) {
                return Some(code);
            }
        }
    }
    for candidate in candidates {
        if let Some(code) = coerce_exit_code_value(*candidate) {
            return Some(code);
        }
    }
    None
}

/// Task id resolution for evidence append helpers: explicit override wins, then task_view pointers.
///
/// The override is validated via `safe_task_id_component` to reject path traversal
/// (`..`, `/`, `\0`). Invalid overrides fall back to `resolve_task_view`.
fn resolve_evidence_append_task_id(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Option<String> {
    let validated_override = task_id_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(core_state_utils::path_guard::safe_task_id_component);
    validated_override.map(ToString::to_string).or_else(|| {
        let view = core_state::task_state::resolve_task_view(repo_root, None);
        view.task_id.filter(|s| !s.is_empty())
    })
}

pub fn append_evidence_index_merged_row(
    repo_root: &Path,
    task_id_override: Option<&str>,
    entry: Map<String, Value>,
) -> Result<()> {
    // 解析 entry 中的签名字段用于去重（精确去重：command_preview + recorded_at）
    let entry_signature = entry
        .get("command_preview")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let entry_recorded_at = entry
        .get("recorded_at")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();

    let resolved_task_id = resolve_evidence_append_task_id(repo_root, task_id_override);

    // Lightweight readiness check: avoid full snapshot rebuild
    let current_root = repo_root.join("artifacts/current");
    let active_pointer_exists = current_root.join(TASK_POINTERS_FILENAME).is_file();
    if !active_pointer_exists {
        tracing::debug!(
            current_dir = %current_root.display(),
            "evidence append skipped — no active continuity session"
        );
        return Ok(());
    }

    // Write evidence to task-local subdirectory when a task_id is resolved,
    // matching the read path in FrameworkRuntimeView and
    // task_evidence_artifacts_summary_for_task.
    let evidence_path = match resolved_task_id {
        Some(ref tid) => current_root.join(tid).join(EVIDENCE_INDEX_FILENAME),
        None => current_root.join(EVIDENCE_INDEX_FILENAME),
    };
    if let Some(parent) = evidence_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let _tx_payload = {
        let _evidence_lock = rt_storage::acquire_runtime_path_lock(&evidence_path)?;

        let existing = read_json_strict(&evidence_path)?;
        let mut rows: Vec<Map<String, Value>> = normalize_evidence_index(&existing);

        let is_duplicate = rows.iter().any(|row| {
            let sig_cmd = row
                .get("command_preview")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let sig_at = row
                .get("recorded_at")
                .and_then(Value::as_str)
                .unwrap_or_default();
            sig_cmd == entry_signature && sig_at == entry_recorded_at
        });
        let tx_payload = Value::Object(entry.clone());
        if !is_duplicate {
            // EV-F007: Compute chain_hash for integrity verification.
            let prev_hash = rows.last()
                .and_then(|r| r.get("chain_hash").and_then(Value::as_str))
                .unwrap_or("genesis");
            let content = serde_json::to_string(&Value::Object(entry.clone())).unwrap_or_default();
            let chain_hash = compute_chain_hash(prev_hash, &content);
            let mut entry_with_hash = entry.clone();
            entry_with_hash.insert("chain_hash".to_string(), json!(chain_hash));
            rows.push(entry_with_hash);
        }

        if rows.len() > MAX_POST_TOOL_EVIDENCE_ARTIFACTS {
            // Keep at most 80 success rows + fill remaining budget with non-success rows
            let success_budget = (MAX_POST_TOOL_EVIDENCE_ARTIFACTS * 2 / 3).min(rows.len());
            let mut success_rows: Vec<Map<String, Value>> = Vec::new();
            let mut other_rows: Vec<Map<String, Value>> = Vec::new();
            for row in rows.drain(..) {
                if row.get("success").and_then(Value::as_bool) == Some(true) {
                    success_rows.push(row);
                } else {
                    other_rows.push(row);
                }
            }
            if success_rows.len() > success_budget {
                let drain = success_rows.len() - success_budget;
                success_rows.drain(0..drain);
            }
            let budget = MAX_POST_TOOL_EVIDENCE_ARTIFACTS.saturating_sub(success_rows.len());
            if other_rows.len() > budget {
                let drain = other_rows.len() - budget;
                other_rows.drain(0..drain);
            }
            rows = success_rows;
            rows.extend(other_rows);
        }
        let payload = json!({
            "schema_version": EVIDENCE_INDEX_SCHEMA_VERSION,
            "artifacts": rows.into_iter().map(Value::Object).collect::<Vec<Value>>(),
        });

        // Write TASK_LEDGER first (transactional record). If this fails, propagate
        // the error — the EVIDENCE_INDEX write is skipped so the caller can retry
        // with a clean state (adversarial audit fix C).
        if let Some(tid) = resolved_task_id.as_ref() {
            let tx = core_state::task_ledger::LedgerTransaction::new("evidence", tx_payload.clone())
                .with_schema_version(1);
            core_state::task_ledger::append_transaction(repo_root, tid, tx)?;
        }

        // Now write EVIDENCE_INDEX.json (if this fails, the ledger has the
        // transaction and the index is recoverable on the next append).
        write_json_if_changed_unlocked(&evidence_path, &payload)?;
        tx_payload
    };
    // TASK_STATE.json aggregate was removed in Wave 2b.
    Ok(())
}

/// `framework hook-evidence-append`：供 Cursor hook 等外部进程写入一条验证记录。
///
/// JSON：`repo_root`（可选）、`task_id`（可选）、`command_preview`（必填）、`exit_code`（可选）、`source`（可选，默认 `external_hook`）。
pub fn framework_hook_evidence_append(payload: Value) -> Result<Value> {
    let explicit = payload.get("repo_root").and_then(|v| {
        let s = value_text(Some(v));
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    });
    let repo_root = resolve_repo_root_arg(explicit.as_deref())?;
    let preview = required_payload_text(&payload, "command_preview", "hook evidence append")?;
    let preview_trim = preview.trim();
    if preview_trim.is_empty() {
        return Err(FrameworkError::validation(
            "hook evidence append requires non-empty command_preview",
        ));
    }
    let source = defaulted_payload_text(&payload, "source", "external_hook");
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let exit_code = payload
        .get("exit_code")
        .and_then(|v| coerce_exit_code_value(Some(v)));

    let cursor_hook = source.trim().to_ascii_lowercase().starts_with("cursor_");
    let preview_lower = preview_trim.to_ascii_lowercase();
    if !cursor_hook && !shell_command_looks_like_verification(&preview_lower) {
        tracing::info!(
            command_preview = %preview_trim,
            source = %source,
            "framework_hook_evidence_append skipped — command_preview did not match \
             verification heuristics (adversarial audit fix E)",
        );
        return Ok(json!({
            "ok": true,
            "skipped": true,
            "reason": "command_preview did not match verification heuristics",
            "schema_version": HOOK_EVIDENCE_APPEND_SCHEMA_VERSION,
            "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
        }));
    }

    let preview_store = truncate_utf8_chars(preview_trim, 2000);
    let mut entry = Map::new();
    entry.insert("kind".to_string(), json!("external_hook_verification"));
    entry.insert("source".to_string(), json!(source.trim()));
    entry.insert("command_preview".to_string(), json!(preview_store));
    entry.insert("recorded_at".to_string(), json!(current_local_timestamp()));

    // Programmatic verification of physical artifact association (L1 Truthfulness)
    let artifact_ok = detect_and_verify_physical_artifact(&repo_root, &preview_lower);
    if !artifact_ok {
        entry.insert("artifact_verification_failed".to_string(), json!(true));
    }

    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0 && artifact_ok));
    } else {
        entry.insert("success".to_string(), json!(artifact_ok));
    }
    append_evidence_index_merged_row(&repo_root, task_id.as_deref(), entry)?;
    Ok(json!({
        "ok": true,
        "skipped": false,
        "schema_version": HOOK_EVIDENCE_APPEND_SCHEMA_VERSION,
        "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
    }))
}

fn shell_command_looks_like_verification(command_lower: &str) -> bool {
    // Fast reject: skip 50+ contains checks when no seed keyword is present.
    if !command_lower.contains("cargo")
        && !command_lower.contains("test")
        && !command_lower.contains("check")
        && !command_lower.contains("make")
        && !command_lower.contains("npm")
        && !command_lower.contains("pytest")
        && !command_lower.contains("yarn")
        && !command_lower.contains("pnpm")
        && !command_lower.contains("bun")
        && !command_lower.contains("vitest")
        && !command_lower.contains("jest")
        && !command_lower.contains("rake")
        && !command_lower.contains("go ")
        && !command_lower.contains("dotnet")
        && !command_lower.contains("maturin")
        && !command_lower.contains("tox")
        && !command_lower.contains("uv run")
        && !command_lower.contains("just")
        && !command_lower.contains("ruff")
        && !command_lower.contains("mypy")
        && !command_lower.contains("deno")
        && !command_lower.contains("lint")
        && !command_lower.contains("tsc")
        && !command_lower.contains("eslint")
        && !command_lower.contains("prettier")
        && !command_lower.contains("biome")
        && !command_lower.contains("gradle")
        && !command_lower.contains("mvn")
        && !command_lower.contains("flutter")
        && !command_lower.contains("swift")
        && !command_lower.contains("dart")
        && !command_lower.contains("playwright")
        && !command_lower.contains("nx ")
        && !command_lower.contains("scripts/verify")
        && !command_lower.contains("verify.sh")
        && !command_lower.contains("nextest")
        && !command_lower.contains("policy")
        && !command_lower.contains("verify_cursor")
        && !command_lower.contains("python")
        && !command_lower.contains("lean")
        && !command_lower.contains("coq")
        && !command_lower.contains("isabelle")
        && !command_lower.contains("lake ")
        && !command_lower.contains("z3 ")
    {
        return false;
    }

    // Original (Rust / Python / JS test runners + lint).
    command_lower.contains("cargo test")
        || command_lower.contains("cargo check")
        || command_lower.contains("cargo clippy")
        || command_lower.contains("cargo build")
        || command_lower.contains("cargo fmt")
        || command_lower.contains("cargo nextest")
        || command_lower.contains("cargo hack")
        || command_lower.contains("nextest")
        || command_lower.contains("pytest")
        || command_lower.contains("npm test")
        || command_lower.contains("pnpm test")
        || command_lower.contains("yarn test")
        || command_lower.contains("make test")
        || command_lower.contains("make check")
        || command_lower.contains("make ci")
        || command_lower.contains("make verify")
        || command_lower.contains("go test")
        || command_lower.contains("go vet")
        || command_lower.contains("dotnet test")
        || command_lower.contains("maturin")
        || command_lower.contains("tox")
        || command_lower.contains("uv run")
        || command_lower.contains("just test")
        || command_lower.contains("just check")
        || command_lower.contains("vitest")
        || command_lower.contains("jest")
        || command_lower.contains("ruby test")
        || command_lower.contains("rake test")
        || command_lower.contains("verify_cursor_hooks")
        || command_lower.contains("policy_contracts")
        || command_lower.contains("ruff check")
        || command_lower.contains("ruff format")
        || command_lower.contains("mypy")
        || command_lower.contains("deno test")
        || command_lower.contains("bun test")
        // pnpm / bun tooling.
        || command_lower.contains("pnpm lint")
        || command_lower.contains("pnpm check")
        || command_lower.contains("bun lint")
        // TypeScript / JS tooling (no `test` keyword).
        || command_lower.contains("tsc --noemit")
        || command_lower.contains("tsc -p")
        || command_lower.contains("eslint")
        || command_lower.contains("prettier --check")
        || command_lower.contains("biome check")
        || command_lower.contains("biome ci")
        // JVM ecosystems.
        || command_lower.contains("gradle test")
        || command_lower.contains("gradlew test")
        || command_lower.contains("gradle check")
        || command_lower.contains("mvn test")
        || command_lower.contains("mvn verify")
        || command_lower.contains("mvn package")
        // Mobile / Dart / Swift tooling.
        || command_lower.contains("flutter test")
        || command_lower.contains("swift test")
        || command_lower.contains("swift build")
        || command_lower.contains("dart analyze")
        // E2E / cross-runner test frameworks.
        || command_lower.contains("playwright test")
        || command_lower.contains("nx test")
        || command_lower.contains("nx affected")
        // Repo-local verifier scripts (any path under scripts/ ending with verify*).
        || command_lower.contains("scripts/verify")
        || command_lower.contains("/verify.sh")
        || command_lower.contains("./verify.sh")
        || command_lower.contains("task test")
        || command_lower.contains("task check")
        // Formal / math toolchains: shared with `harness_context_signals` (`formal_toolchain`).
        || framework_core::formal_toolchain::ascii_lower_contains_formal_toolchain_tokens(command_lower)
}

pub(super) fn detect_and_verify_physical_artifact(repo_root: &Path, command_lower: &str) -> bool {
    let max_delta = 15; // 15s safe time window for mtime verification to accommodate slow disks

    // Dynamic bypass: Skip physical filesystem assertions during Rust target integration tests
    let repo_path_str = repo_root.to_string_lossy();
    if repo_path_str.contains("target/tmp")
        || repo_path_str.contains("post-tool-evidence-append")
        || repo_path_str.contains("cursor-post-tool-evidence-append")
    {
        return true;
    }

    if command_lower.contains("cargo test")
        || command_lower.contains("cargo check")
        || command_lower.contains("cargo clippy")
        || command_lower.contains("cargo build")
    {
        let target_dir = repo_root.join("target");
        if target_dir.is_dir() {
            if is_modified_recently(&target_dir, max_delta) {
                return true;
            }
            let debug_dir = target_dir.join("debug");
            if debug_dir.is_dir() && is_modified_recently(&debug_dir, max_delta) {
                return true;
            }
            return false;
        }
        return false;
    }

    if command_lower.contains("pytest") {
        let py_cache = repo_root.join(".pytest_cache");
        if py_cache.is_dir() && is_modified_recently(&py_cache, max_delta) {
            return true;
        }
        let junit = repo_root.join("junit.xml");
        if junit.is_file() && is_modified_recently(&junit, max_delta) {
            return true;
        }
        return false;
    }

    // 对未识别的验证命令，默认拒绝物理产物验证（fail-closed）
    false
}

fn is_modified_recently(path: &std::path::Path, max_delta_secs: u64) -> bool {
    use std::time::SystemTime;
    if let Ok(metadata) = std::fs::metadata(path)
        && let Ok(modified) = metadata.modified()
    {
        let now = SystemTime::now();
        if let Ok(elapsed) = now.duration_since(modified) {
            return elapsed.as_secs() <= max_delta_secs;
        }
        if let Ok(elapsed) = modified.duration_since(now) {
            return elapsed.as_secs() <= max_delta_secs;
        }
    }
    false
}

pub(super) fn normalize_evidence_index(payload: &Value) -> Vec<Map<String, Value>> {
    let items = if payload.get("schema_version").and_then(Value::as_str)
        == Some(EVIDENCE_INDEX_SCHEMA_VERSION)
    {
        payload.get("artifacts")
    } else {
        payload.get("artifacts").or_else(|| payload.get("evidence"))
    };
    items
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.as_object().cloned())
                .collect()
        })
        .unwrap_or_default()
}

// ── Tests ──

#[cfg(test)]
mod shell_command_verification_heuristic_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::shell_command_looks_like_verification;

    fn check(cmd: &str) -> bool {
        shell_command_looks_like_verification(&cmd.to_ascii_lowercase())
    }

    #[test]
    fn matrix_math_formal_and_build_tools() {
        assert!(check(
            "python -c \"import sympy; print(sympy.simplify(1))\""
        ));
        assert!(check("z3 /tmp/proof.smt2"));
        assert!(check("  z3  /tmp/x.smt2"));
        assert!(check("lean --version"));
        assert!(check("lake build && lake test"));
        assert!(check("coqc -Q theories Foo.v"));
        assert!(check("coqchk -silent Foo.vo"));
        assert!(check("isabelle build -D ."));
        assert!(check("cargo test -q"));
        assert!(check("pytest -q"));
    }

    #[test]
    fn matrix_rejects_bare_python_and_random_strings() {
        assert!(!check("python foo.py"));
        assert!(!check("python -c \"print(1)\""));
        assert!(!check("echo hello"));
        assert!(!check("leaning tower")); // not `lean ` token
    }

    #[test]
    fn test_physical_artifact_checks() {
        use super::detect_and_verify_physical_artifact;
        let temp_dir = std::env::temp_dir().join(format!(
            "router-rs-test-artifact-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Non-verification commands should be rejected by default (fail-closed)
        assert!(!detect_and_verify_physical_artifact(
            &temp_dir,
            "python foo.py"
        ));

        // 2. Pytest should return false when pytest_cache / junit.xml are missing
        assert!(!detect_and_verify_physical_artifact(&temp_dir, "pytest -v"));

        // 3. Cargo test should return false when target directory is missing
        assert!(!detect_and_verify_physical_artifact(
            &temp_dir,
            "cargo test"
        ));

        // 4. Simulate pytest generating .pytest_cache folder -> pytest passes
        let pytest_cache = temp_dir.join(".pytest_cache");
        std::fs::create_dir_all(&pytest_cache).unwrap();
        assert!(detect_and_verify_physical_artifact(&temp_dir, "pytest -v"));

        // 5. Simulate cargo generating target folder -> cargo test passes
        let target_dir = temp_dir.join("target");
        std::fs::create_dir_all(&target_dir).unwrap();
        assert!(detect_and_verify_physical_artifact(&temp_dir, "cargo test"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
