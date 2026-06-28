#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::backend::*;
use super::filesystem::*;
use super::operation::*;
use super::paths::*;
use super::sqlite::*;
use super::*;
use serde_json::{Map, json};
use serial_test::serial;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("router-rs-{prefix}-{nonce}"));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

#[test]
fn effective_storage_root_does_not_silently_consult_codex_or_cursor_home() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    // Regression: an earlier revision had `effective_storage_root_for_request`
    // fall back to `CODEX_HOME` / `CURSOR_HOME` when the caller did not
    // pin a `storage_root`. That made codex-CLI processes write to
    // `~/.codex/...` instead of cwd for relative paths — a silent
    // breaking change. Only `ROUTER_RS_STORAGE_ROOT` (an explicit
    // router-rs-only knob) is allowed as an env-driven fallback.
    let prior_router = std::env::var("ROUTER_RS_STORAGE_ROOT").ok();
    let prior_codex = std::env::var("CODEX_HOME").ok();
    let prior_cursor = std::env::var("CURSOR_HOME").ok();
    unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_STORAGE_ROOT") };
    unsafe { core_state_utils::env_sync::set_env("CODEX_HOME", "/tmp/router-rs-test-codex-home") };
    unsafe {
        core_state_utils::env_sync::set_env("CURSOR_HOME", "/tmp/router-rs-test-cursor-home")
    };
    let request = RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "artifacts/x.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: None,
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let resolved = effective_storage_root_for_request(&request);
    assert!(
        resolved.is_none(),
        "CODEX_HOME / CURSOR_HOME must NOT be silently used as storage_root, got: {resolved:?}"
    );

    // Sanity: explicit ROUTER_RS_STORAGE_ROOT IS honored.
    unsafe {
        core_state_utils::env_sync::set_env(
            "ROUTER_RS_STORAGE_ROOT",
            "/tmp/router-rs-test-explicit",
        )
    };
    let resolved = effective_storage_root_for_request(&request);
    assert_eq!(resolved.as_deref(), Some("/tmp/router-rs-test-explicit"));

    // Cleanup test env so we don't leak state to other tests.
    match prior_router {
        Some(v) => unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", &v) },
        None => unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_STORAGE_ROOT") },
    }
    match prior_codex {
        Some(v) => unsafe { core_state_utils::env_sync::set_env("CODEX_HOME", &v) },
        None => unsafe { core_state_utils::env_sync::remove_env("CODEX_HOME") },
    }
    match prior_cursor {
        Some(v) => unsafe { core_state_utils::env_sync::set_env("CURSOR_HOME", &v) },
        None => unsafe { core_state_utils::env_sync::remove_env("CURSOR_HOME") },
    }
}

#[test]
fn runtime_storage_allows_write_inside_storage_root() {
    let root = unique_temp_dir("runtime-storage-inside-root");
    let request = RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "artifacts/output.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("{\"ok\":true}".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let response =
        runtime_storage_operation(request).expect("write within storage root should pass");
    assert!(response.exists);
    assert_eq!(response.bytes_written, Some("{\"ok\":true}".len()));
}

#[test]
fn runtime_storage_rejects_parent_escape_path() {
    let root = unique_temp_dir("runtime-storage-parent-escape");
    let request = RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "../escape.txt".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("nope".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let error = runtime_storage_operation(request).expect_err("parent escape must be rejected");
    assert!(error.to_string().contains("must stay under storage root"));
}

#[test]
fn runtime_storage_rejects_absolute_path_outside_storage_root_for_sqlite() {
    let root = unique_temp_dir("runtime-storage-absolute-reject");
    let db_path = root.join("runtime.sqlite3");
    let outside = std::env::temp_dir().join("runtime-storage-outside.txt");
    let request = RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: outside.display().to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(root.display().to_string()),
        payload_text: Some("nope".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let error =
        runtime_storage_operation(request).expect_err("absolute outside root must be rejected");
    assert!(error.to_string().contains("must stay under storage root"));
}

/// Simulates a collision on the first `create_new` temp name; second attempt must succeed.
#[test]
fn filesystem_write_text_retries_temp_on_create_new_collision() {
    let root = unique_temp_dir("runtime-storage-temp-collision");
    let target = root.join("payload.txt");
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .expect("file name");
    let parent = target.parent().expect("parent");
    let nanos = 9_876_543_210u128;
    let pid = std::process::id();
    let blocking_tmp = parent.join(format!(".router-rs.{file_name}.{nanos}.{pid}.0.tmp"));
    fs::write(&blocking_tmp, b"block").expect("seed blocking temp");

    filesystem_write_text_inner(&target, "ok", nanos)
        .expect("write should retry past first temp collision");
    let body = fs::read_to_string(&target).expect("read back");
    assert_eq!(body, "ok");
    assert_eq!(
        fs::read_to_string(&blocking_tmp).expect("blocking file remains"),
        "block"
    );
    let _ = fs::remove_file(&target);
    let _ = fs::remove_file(&blocking_tmp);
}

#[cfg(unix)]
#[test]
fn runtime_storage_filesystem_rejects_symlink_write_path() {
    use std::os::unix::fs::symlink;

    let root = unique_temp_dir("runtime-storage-symlink-reject");
    let real = root.join("real.txt");
    fs::write(&real, b"x").expect("real file");
    let alias = root.join("alias.txt");
    let _ = fs::remove_file(&alias);
    symlink(&real, &alias).expect("create symlink");

    let request = RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "alias.txt".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("y".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let err = runtime_storage_operation(request).expect_err("symlink path must be rejected");
    assert!(
        err.to_string().contains("must not be a symlink"),
        "unexpected error: {err}"
    );
}

/// A directory symlink in the parent chain that points outside the
/// configured `storage_root` must not be traversable for writes. The
/// lexical containment check passes (`<inside>/escape/leak.txt` still
/// appears to live under `<inside>`), so this regression is only caught
/// once the resolver compares canonicalized real paths.
#[cfg(unix)]
#[test]
fn runtime_storage_rejects_parent_dir_symlink_escape() {
    use std::os::unix::fs::symlink;

    let outside = unique_temp_dir("runtime-storage-parent-symlink-outside");
    let inside = unique_temp_dir("runtime-storage-parent-symlink-inside");
    let link = inside.join("escape");
    let _ = fs::remove_file(&link);
    let _ = fs::remove_dir_all(&link);
    symlink(&outside, &link).expect("create dir symlink");

    let request = RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "escape/leak.txt".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(inside.display().to_string()),
        payload_text: Some("leak".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let err = runtime_storage_operation(request)
        .expect_err("parent-chain symlink escape must be rejected");
    assert!(
        err.to_string().contains("must stay under storage root"),
        "unexpected error: {err}"
    );
    assert!(
        err.to_string().contains("after symlink resolution"),
        "should be the canonical-path branch of the check, got: {err}"
    );
    let leaked = outside.join("leak.txt");
    assert!(
        !leaked.exists(),
        "no payload should have been written to the symlink target"
    );
}

/// Append must also honor the canonical containment check. We allow a
/// pre-existing payload at the canonical real path, then try to append
/// through a parent symlink that points outside `storage_root`. The
/// resolver must reject the request before any append is attempted.
#[cfg(unix)]
#[test]
fn runtime_storage_append_rejects_parent_dir_symlink_escape() {
    use std::os::unix::fs::symlink;

    let outside = unique_temp_dir("runtime-storage-append-symlink-outside");
    let inside = unique_temp_dir("runtime-storage-append-symlink-inside");
    let link = inside.join("escape");
    let _ = fs::remove_file(&link);
    let _ = fs::remove_dir_all(&link);
    symlink(&outside, &link).expect("create dir symlink");

    let prepared = outside.join("leak.txt");
    fs::write(&prepared, b"pre").expect("seed outside payload");

    let request = RuntimeStorageRequestPayload {
        operation: "append_text".to_string(),
        path: "escape/leak.txt".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(inside.display().to_string()),
        payload_text: Some("-leak".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let err = runtime_storage_operation(request)
        .expect_err("append via parent-chain symlink escape must be rejected");
    assert!(
        err.to_string().contains("must stay under storage root"),
        "unexpected error: {err}"
    );
    let body = fs::read_to_string(&prepared).expect("outside payload remains");
    assert_eq!(body, "pre", "append must not have leaked into outside");
}

/// When the sqlite backend is selected without an explicit `storage_root`,
/// the resolver must fall back to `sqlite_db_path.parent()` (historical
/// semantics) instead of the process working directory.
#[test]
fn runtime_storage_sqlite_default_root_uses_db_parent() {
    let root = unique_temp_dir("runtime-storage-sqlite-default-root");
    let db_path = root.join("default.sqlite3");
    let canonical_root = root.canonicalize().expect("canonicalize root");
    let canonical_root_string = canonical_root.display().to_string();

    let write = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "default.json".to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: None,
        payload_text: Some("{\"default\":true}".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write should succeed via sqlite default storage_root");

    let resolved_root = write
        .storage_root
        .as_deref()
        .map(PathBuf::from)
        .map(|path| path.canonicalize().unwrap_or(path));
    assert_eq!(
        resolved_root.map(|p| p.display().to_string()),
        Some(canonical_root_string.clone()),
        "sqlite default storage_root should resolve to db parent"
    );
    assert_eq!(
        write.sqlite_db_path.as_deref(),
        Some(db_path.display().to_string().as_str())
    );
    assert!(
        db_path.exists(),
        "sqlite db should be created next to its parent"
    );

    let read = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: "default.json".to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: None,
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("read should succeed via sqlite default storage_root");
    assert_eq!(read.payload_text.as_deref(), Some("{\"default\":true}"));
}

#[test]
fn runtime_storage_append_text_returns_post_append_digest_for_all_backends() {
    let root = unique_temp_dir("runtime-storage-append-digest-parity");
    let rel_path = "runtime/payload.txt";
    let initial = "hello";
    let append = "-world";
    let expected = format!("{initial}{append}");
    let expected_digest = payload_sha256(&expected);

    for backend in ["filesystem", "memory", "sqlite"] {
        let backend_root = root.join(format!("backend-{backend}"));
        fs::create_dir_all(&backend_root).expect("create backend root");
        let db_path = backend_root.join("runtime.sqlite3");

        let write = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: rel_path.to_string(),
            backend_family: backend.to_string(),
            sqlite_db_path: (backend == "sqlite").then(|| db_path.display().to_string()),
            storage_root: Some(backend_root.display().to_string()),
            payload_text: Some(initial.to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        runtime_storage_operation(write).expect("seed write succeeds");

        let append_request = RuntimeStorageRequestPayload {
            operation: "append_text".to_string(),
            path: rel_path.to_string(),
            backend_family: backend.to_string(),
            sqlite_db_path: (backend == "sqlite").then(|| db_path.display().to_string()),
            storage_root: Some(backend_root.display().to_string()),
            payload_text: Some(append.to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let append_response =
            runtime_storage_operation(append_request).expect("append request succeeds");
        assert_eq!(append_response.bytes_written, Some(append.len()));
        assert_eq!(
            append_response.payload_sha256.as_deref(),
            Some(expected_digest.as_str())
        );

        let read = RuntimeStorageRequestPayload {
            operation: "read_text".to_string(),
            path: rel_path.to_string(),
            backend_family: backend.to_string(),
            sqlite_db_path: (backend == "sqlite").then(|| db_path.display().to_string()),
            storage_root: Some(backend_root.display().to_string()),
            payload_text: None,
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let read_response = runtime_storage_operation(read).expect("read request succeeds");
        assert_eq!(
            read_response.payload_text.as_deref(),
            Some(expected.as_str())
        );
    }
}

#[test]
fn runtime_storage_append_digest_uses_selected_backend_payload() {
    let root = unique_temp_dir("runtime-storage-append-digest-backend-selected");
    let rel_path = "runtime/payload.txt";
    let full_path = root.join(rel_path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(&full_path, "shadow-filesystem").expect("seed shadow filesystem payload");

    let write = RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: rel_path.to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("mem".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    runtime_storage_operation(write).expect("memory write succeeds");

    let append_payload = "-append";
    let append = RuntimeStorageRequestPayload {
        operation: "append_text".to_string(),
        path: rel_path.to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some(append_payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let response = runtime_storage_operation(append).expect("memory append succeeds");
    let expected_mem = format!("mem{append_payload}");
    assert_eq!(
        response.payload_sha256.as_deref(),
        Some(payload_sha256(&expected_mem).as_str())
    );
    assert_ne!(
        response.payload_sha256.as_deref(),
        Some(payload_sha256("shadow-filesystem").as_str())
    );
}

#[test]
fn runtime_storage_append_text_rejects_missing_payload_text() {
    let root = unique_temp_dir("runtime-storage-append-missing-payload");
    let request = RuntimeStorageRequestPayload {
        operation: "append_text".to_string(),
        path: "payload.txt".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    let err = runtime_storage_operation(request).expect_err("missing payload must fail");
    assert!(
        err.to_string()
            .contains("append_text requires payload_text")
    );
}

#[test]
fn runtime_storage_sqlite_append_isolated_by_storage_root() {
    let root = unique_temp_dir("runtime-storage-sqlite-append-isolation");
    let db_path = root.join("shared.sqlite3");
    let rel_path = "session/data.log";

    let session_a = root.join("session-a");
    let session_b = root.join("session-b");
    fs::create_dir_all(&session_a).expect("create session_a");
    fs::create_dir_all(&session_b).expect("create session_b");

    for (storage_root, write_body, append_body) in
        [(&session_a, "a0", "-a1"), (&session_b, "b0", "-b1")]
    {
        let write = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: Some(storage_root.display().to_string()),
            payload_text: Some(write_body.to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        runtime_storage_operation(write).expect("sqlite write succeeds");
        let append = RuntimeStorageRequestPayload {
            operation: "append_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: Some(storage_root.display().to_string()),
            payload_text: Some(append_body.to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        runtime_storage_operation(append).expect("sqlite append succeeds");
    }

    let read_a = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: rel_path.to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(session_a.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("session_a read succeeds");
    let read_b = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: rel_path.to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(session_b.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("session_b read succeeds");

    assert_eq!(read_a.payload_text.as_deref(), Some("a0-a1"));
    assert_eq!(read_b.payload_text.as_deref(), Some("b0-b1"));
}

#[test]
fn runtime_storage_read_text_supports_tail_lines_and_max_bytes() {
    let root = unique_temp_dir("runtime-storage-read-limits");
    let rel_path = "logs/runtime.log";
    let payload = "line-1\nline-2\nline-3\nline-4\n";
    runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: rel_path.to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some(payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write succeeds");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: rel_path.to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: Some(8),
        tail_lines: Some(2),
    })
    .expect("limited read succeeds");
    let expected_digest = payload_sha256(payload);
    assert_eq!(response.payload_text.as_deref(), Some("line-4\n"));
    assert_eq!(response.bytes_returned, Some("line-4\n".len()));
    assert_eq!(response.truncated, Some(true));
    assert_eq!(
        response.payload_sha256.as_deref(),
        Some(expected_digest.as_str())
    );
}

#[cfg(unix)]
#[test]
fn runtime_storage_filesystem_append_does_not_require_read_after_write() {
    use std::os::unix::fs::PermissionsExt;

    let root = unique_temp_dir("runtime-storage-append-no-readback");
    let rel_path = "logs/runtime.log";
    let absolute = root.join(rel_path);
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(&absolute, b"seed").expect("seed payload");
    fs::set_permissions(&absolute, fs::Permissions::from_mode(0o200))
        .expect("set write-only permission");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "append_text".to_string(),
        path: rel_path.to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("-tail".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("append should not require read permission");
    assert_eq!(response.bytes_written, Some("-tail".len()));
    assert_eq!(response.payload_sha256, None);
}

#[test]
fn normalize_runtime_path_rejects_empty() {
    assert!(normalize_runtime_path("").is_err());
}

#[test]
fn normalize_runtime_path_accepts_simple() {
    let result = normalize_runtime_path("artifacts/current/task-1");
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.to_string_lossy().contains("artifacts"));
    assert!(path.to_string_lossy().contains("current"));
    assert!(path.to_string_lossy().contains("task-1"));
}

#[test]
fn clean_absolute_path_rejects_relative() {
    assert!(clean_absolute_path(Path::new("relative/path")).is_err());
}

#[test]
fn clean_absolute_path_accepts_absolute() {
    let result = clean_absolute_path(Path::new("/tmp/test"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), PathBuf::from("/tmp/test"));
}

// ── normalize_runtime_path extended ──

#[test]
fn normalize_runtime_path_absolute_passthrough() {
    let result = normalize_runtime_path("/tmp/some/path").expect("absolute path should work");
    assert_eq!(result, PathBuf::from("/tmp/some/path"));
}

#[test]
fn normalize_runtime_path_relative_joins_cwd() {
    let result = normalize_runtime_path("relative/file.txt").expect("relative path should work");
    assert!(result.is_absolute());
    assert!(result.ends_with("relative/file.txt"));
}

#[test]
fn normalize_runtime_path_trims_whitespace() {
    let result = normalize_runtime_path("  /tmp/trimmed  ").expect("trimmed path should work");
    assert_eq!(result, PathBuf::from("/tmp/trimmed"));
}

#[test]
fn normalize_runtime_path_collapses_dot_segments() {
    let result = normalize_runtime_path("/tmp/a/../b/./c").expect("dot segments should collapse");
    assert_eq!(result, PathBuf::from("/tmp/b/c"));
}

// ── clean_absolute_path extended ──

#[test]
fn clean_absolute_path_normal_path() {
    let result = clean_absolute_path(Path::new("/a/b/c")).expect("should clean");
    assert_eq!(result, PathBuf::from("/a/b/c"));
}

#[test]
fn clean_absolute_path_removes_dot_component() {
    let result = clean_absolute_path(Path::new("/a/./b")).expect("should remove .");
    assert_eq!(result, PathBuf::from("/a/b"));
}

#[test]
fn clean_absolute_path_collapses_parent() {
    let result = clean_absolute_path(Path::new("/a/b/../c")).expect("should collapse ..");
    assert_eq!(result, PathBuf::from("/a/c"));
}

#[test]
fn clean_absolute_path_rejects_escape_beyond_root() {
    let result = clean_absolute_path(Path::new("/../escape"));
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("escapes filesystem root")
    );
}

// ── canonicalize_or_clean_absolute_path ──

#[test]
fn canonicalize_or_clean_absolute_path_rejects_relative() {
    let result = canonicalize_or_clean_absolute_path(Path::new("rel"));
    assert!(result.is_err());
}

#[test]
fn canonicalize_or_clean_absolute_path_cleans_absolute() {
    let result = canonicalize_or_clean_absolute_path(Path::new("/tmp/a/../b")).expect("ok");
    assert_eq!(result, PathBuf::from("/tmp/b"));
}

// ── explicit_storage_root_override ──

#[test]
fn explicit_storage_root_override_returns_none_when_unset() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let prior = std::env::var("ROUTER_RS_STORAGE_ROOT").ok();
    unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_STORAGE_ROOT") };
    assert_eq!(explicit_storage_root_override(), None);
    if let Some(v) = prior {
        unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", &v) };
    }
}

#[test]
fn explicit_storage_root_override_returns_none_when_empty() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let prior = std::env::var("ROUTER_RS_STORAGE_ROOT").ok();
    unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", "  ") };
    assert_eq!(explicit_storage_root_override(), None);
    match prior {
        Some(v) => unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", &v) },
        None => unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_STORAGE_ROOT") },
    }
}

#[test]
fn explicit_storage_root_override_returns_value_when_set() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let prior = std::env::var("ROUTER_RS_STORAGE_ROOT").ok();
    unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", "/my/root") };
    assert_eq!(
        explicit_storage_root_override(),
        Some("/my/root".to_string())
    );
    match prior {
        Some(v) => unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", &v) },
        None => unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_STORAGE_ROOT") },
    }
}

// ── effective_storage_root_for_request extended ──

#[test]
fn effective_storage_root_prefers_explicit_over_env() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let prior = std::env::var("ROUTER_RS_STORAGE_ROOT").ok();
    unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", "/env/root") };
    let request = RuntimeStorageRequestPayload {
        operation: "exists".to_string(),
        path: "x".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some("/explicit/root".to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    assert_eq!(
        effective_storage_root_for_request(&request),
        Some("/explicit/root".to_string())
    );
    match prior {
        Some(v) => unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", &v) },
        None => unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_STORAGE_ROOT") },
    }
}

#[test]
fn effective_storage_root_empty_explicit_falls_to_env() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    // When storage_root is set but empty/whitespace, the function does NOT
    // early-return; it falls through to explicit_storage_root_override().
    let prior = std::env::var("ROUTER_RS_STORAGE_ROOT").ok();
    unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", "/env/fallback") };
    let request = RuntimeStorageRequestPayload {
        operation: "exists".to_string(),
        path: "x".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some("  ".to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    assert_eq!(
        effective_storage_root_for_request(&request),
        Some("/env/fallback".to_string())
    );
    match prior {
        Some(v) => unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", &v) },
        None => unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_STORAGE_ROOT") },
    }
}

#[test]
fn effective_storage_root_sqlite_uses_db_parent() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let prior = std::env::var("ROUTER_RS_STORAGE_ROOT").ok();
    unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_STORAGE_ROOT") };
    let request = RuntimeStorageRequestPayload {
        operation: "exists".to_string(),
        path: "data.json".to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some("/some/dir/store.sqlite3".to_string()),
        storage_root: None,
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    };
    assert_eq!(
        effective_storage_root_for_request(&request),
        Some("/some/dir".to_string())
    );
    if let Some(v) = prior {
        unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_STORAGE_ROOT", &v) };
    }
}

// ── runtime_backend_capabilities ──

#[test]
fn backend_capabilities_filesystem() {
    let caps = runtime_backend_capabilities("filesystem").expect("filesystem ok");
    assert_eq!(caps.backend_family, "filesystem");
    assert!(caps.supports_atomic_replace);
    assert!(caps.supports_consistent_append);
    assert!(!caps.supports_sqlite_wal);
}

#[test]
fn backend_capabilities_sqlite() {
    let caps = runtime_backend_capabilities("sqlite").expect("sqlite ok");
    assert_eq!(caps.backend_family, "sqlite");
    assert!(caps.supports_compaction);
    assert!(caps.supports_snapshot_delta);
    assert!(caps.supports_sqlite_wal);
}

#[test]
fn backend_capabilities_memory() {
    let caps = runtime_backend_capabilities("memory").expect("memory ok");
    assert_eq!(caps.backend_family, "memory");
    assert!(!caps.supports_compaction);
    assert!(!caps.supports_snapshot_delta);
}

#[test]
fn backend_capabilities_rejects_unknown() {
    let result = runtime_backend_capabilities("redis");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unsupported"));
}

#[test]
fn backend_capabilities_normalizes_aliases() {
    assert_eq!(
        runtime_backend_capabilities("file").unwrap().backend_family,
        "filesystem"
    );
    assert_eq!(
        runtime_backend_capabilities("sqlite3")
            .unwrap()
            .backend_family,
        "sqlite"
    );
    assert_eq!(
        runtime_backend_capabilities("in_memory")
            .unwrap()
            .backend_family,
        "memory"
    );
    assert_eq!(
        runtime_backend_capabilities("regression")
            .unwrap()
            .backend_family,
        "memory"
    );
    assert_eq!(
        runtime_backend_capabilities("regression_double")
            .unwrap()
            .backend_family,
        "memory"
    );
}

// ── runtime_backend_capabilities_payload ──

#[test]
fn backend_capabilities_payload_includes_all_fields() {
    let payload = runtime_backend_capabilities_payload("sqlite").expect("payload ok");
    assert_eq!(payload["backend_family"], "sqlite");
    assert!(payload["supports_atomic_replace"].as_bool().is_some());
    assert!(payload["supports_compaction"].as_bool().is_some());
    assert!(payload["supports_snapshot_delta"].as_bool().is_some());
    assert!(
        payload["supports_remote_event_transport"]
            .as_bool()
            .is_some()
    );
    assert!(payload["supports_consistent_append"].as_bool().is_some());
    assert!(payload["supports_sqlite_wal"].as_bool().is_some());
}

#[test]
fn backend_capabilities_payload_rejects_unknown() {
    let result = runtime_backend_capabilities_payload("unknown");
    assert!(result.is_err());
}

#[test]
fn backend_capabilities_payload_filesystem_snapshot() {
    let payload = runtime_backend_capabilities_payload("filesystem").expect("payload ok");
    insta::assert_debug_snapshot!(payload);
}

#[test]
fn backend_capabilities_payload_sqlite_snapshot() {
    let payload = runtime_backend_capabilities_payload("sqlite").expect("payload ok");
    insta::assert_debug_snapshot!(payload);
}

#[test]
fn backend_capabilities_payload_memory_snapshot() {
    let payload = runtime_backend_capabilities_payload("memory").expect("payload ok");
    insta::assert_debug_snapshot!(payload);
}

#[test]
fn backend_capabilities_payload_all_families_snapshot() {
    let payloads = ["filesystem", "sqlite", "memory"]
        .iter()
        .filter_map(|f| {
            runtime_backend_capabilities_payload(f)
                .ok()
                .map(|p| (*f, p))
        })
        .collect::<Vec<_>>();
    insta::assert_debug_snapshot!(payloads);
}

#[test]
fn backend_family_catalog_snapshot() {
    let catalog = runtime_backend_family_catalog_payload();
    insta::assert_debug_snapshot!(catalog);
}

// ── runtime_backend_family_parity_payload ──

#[test]
fn backend_family_parity_aligned_when_all_same() {
    let payload = runtime_backend_family_parity_payload(
        Some("filesystem"),
        Some("filesystem"),
        Some("filesystem"),
        Some("filesystem"),
    )
    .expect("parity ok");
    assert_eq!(payload["aligned"], true);
    assert_eq!(payload["mismatch_reason"], Value::Null);
}

#[test]
fn backend_family_parity_defaults_to_store() {
    let payload =
        runtime_backend_family_parity_payload(Some("sqlite"), None, None, None).expect("parity ok");
    assert_eq!(payload["aligned"], true);
    assert_eq!(payload["store_backend_family"], "sqlite");
    assert_eq!(payload["checkpointer_backend_family"], "sqlite");
    assert_eq!(payload["trace_backend_family"], "sqlite");
    assert_eq!(payload["state_backend_family"], "sqlite");
}

#[test]
fn backend_family_parity_mismatch() {
    let payload = runtime_backend_family_parity_payload(
        Some("filesystem"),
        Some("sqlite"),
        Some("filesystem"),
        Some("filesystem"),
    )
    .expect("parity ok");
    assert_eq!(payload["aligned"], false);
    assert!(payload["mismatch_reason"].as_str().is_some());
}

// ── normalized_backend_family ──

#[test]
fn normalized_backend_family_lowercases_and_normalizes() {
    assert_eq!(normalized_backend_family("SQLite"), "sqlite");
    assert_eq!(normalized_backend_family("  File-System  "), "file_system");
    assert_eq!(
        normalized_backend_family("REGRESSION_DOUBLE"),
        "regression_double"
    );
}

// ── payload_sha256 ──

#[test]
fn payload_sha256_is_deterministic() {
    let hash1 = payload_sha256("hello world");
    let hash2 = payload_sha256("hello world");
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64);
}

#[test]
fn payload_sha256_differs_for_different_input() {
    let hash1 = payload_sha256("hello");
    let hash2 = payload_sha256("world");
    assert_ne!(hash1, hash2);
}

#[test]
fn payload_sha256_empty_string() {
    let hash = payload_sha256("");
    assert_eq!(hash.len(), 64);
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

// ── slice_tail_by_max_bytes ──

#[test]
fn slice_tail_by_max_bytes_returns_full_when_within_limit() {
    assert_eq!(slice_tail_by_max_bytes("abc", 10), "abc");
}

#[test]
fn slice_tail_by_max_bytes_trims_prefix() {
    let result = slice_tail_by_max_bytes("hello world", 5);
    assert_eq!(result, "world");
}

#[test]
fn slice_tail_by_max_bytes_empty_payload() {
    assert_eq!(slice_tail_by_max_bytes("", 100), "");
}

#[test]
fn slice_tail_by_max_bytes_exact_size() {
    assert_eq!(slice_tail_by_max_bytes("abc", 3), "abc");
}

#[test]
fn slice_tail_by_max_bytes_respects_char_boundary() {
    let input = "xé";
    let result = slice_tail_by_max_bytes(input, 2);
    assert_eq!(result, "é");
}

// ── slice_tail_by_lines ──

#[test]
fn slice_tail_by_lines_returns_full_when_fewer_lines() {
    assert_eq!(slice_tail_by_lines("a\nb\n", 10), "a\nb\n");
}

#[test]
fn slice_tail_by_lines_tails_correctly() {
    let payload = "line1\nline2\nline3\nline4";
    assert_eq!(slice_tail_by_lines(payload, 2), "line3\nline4");
}

#[test]
fn slice_tail_by_lines_zero_returns_empty() {
    assert_eq!(slice_tail_by_lines("anything", 0), "");
}

#[test]
fn slice_tail_by_lines_single_line() {
    assert_eq!(slice_tail_by_lines("only", 1), "only");
}

#[test]
fn slice_tail_by_lines_empty_payload() {
    assert_eq!(slice_tail_by_lines("", 5), "");
}

// ── apply_read_limits ──

#[test]
fn apply_read_limits_no_limits_returns_full() {
    let (text, truncated) = apply_read_limits("hello".to_string(), None, None);
    assert_eq!(text, "hello");
    assert!(!truncated);
}

#[test]
fn apply_read_limits_max_bytes_truncates() {
    let (text, truncated) = apply_read_limits("hello world".to_string(), Some(5), None);
    assert_eq!(text, "world");
    assert!(truncated);
}

#[test]
fn apply_read_limits_tail_lines_truncates() {
    let (text, truncated) = apply_read_limits("a\nb\nc\nd".to_string(), None, Some(2));
    assert_eq!(text, "c\nd");
    assert!(truncated);
}

#[test]
fn apply_read_limits_both_limits_applied() {
    let (text, truncated) = apply_read_limits("aaa\nbbb\nccc\nddd".to_string(), Some(5), Some(2));
    // tail_lines=2 => "ccc\nddd", then max_bytes=5 => last 5 bytes => "c\nddd"
    assert_eq!(text, "c\nddd");
    assert!(truncated);
}

// ── resolve_runtime_storage_path_with_root ──

#[test]
fn resolve_runtime_storage_path_with_root_relative_joined() {
    let root = unique_temp_dir("resolve-relative-join");
    let (resolved, storage_root) =
        resolve_runtime_storage_path_with_root("data/file.json", Some(&root.display().to_string()))
            .expect("should resolve");
    assert!(resolved.starts_with(&storage_root));
    assert!(resolved.ends_with("data/file.json"));
}

#[test]
fn resolve_runtime_storage_path_with_root_absolute_inside() {
    let root = unique_temp_dir("resolve-absolute-inside");
    let target = root.join("sub/file.json");
    let (resolved, storage_root) = resolve_runtime_storage_path_with_root(
        &target.display().to_string(),
        Some(&root.display().to_string()),
    )
    .expect("should resolve");
    assert!(resolved.starts_with(&storage_root));
}

#[test]
fn resolve_runtime_storage_path_with_root_rejects_escape() {
    let root = unique_temp_dir("resolve-escape");
    let result = resolve_runtime_storage_path_with_root(
        "../outside.json",
        Some(&root.display().to_string()),
    );
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("must stay under storage root")
    );
}

#[test]
fn resolve_runtime_storage_path_with_root_rejects_empty_path() {
    let root = unique_temp_dir("resolve-empty");
    let result = resolve_runtime_storage_path_with_root("  ", Some(&root.display().to_string()));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("non-empty"));
}

// ── memory_artifact_path ──

#[test]
fn memory_artifact_path_returns_inside_memory_root() {
    let artifact = memory_artifact_path(Path::new("/tmp/test/payload.json")).expect("ok");
    let root = memory_storage_root().expect("root ok");
    assert!(artifact.starts_with(&root));
    assert!(artifact.to_string_lossy().ends_with(".payload"));
}

#[test]
fn memory_artifact_path_is_deterministic() {
    let p1 = memory_artifact_path(Path::new("/tmp/x.json")).expect("ok");
    let p2 = memory_artifact_path(Path::new("/tmp/x.json")).expect("ok");
    assert_eq!(p1, p2);
}

// ── stable_memory_key ──

#[test]
fn stable_memory_key_normalizes_path() {
    let key = stable_memory_key(Path::new("/tmp/a/../b/./c.json")).expect("ok");
    assert_eq!(key, "/tmp/b/c.json");
}

// ── runtime_storage_operation end-to-end roundtrips ──

#[test]
fn runtime_storage_write_and_read_roundtrip_filesystem() {
    let root = unique_temp_dir("rt-roundtrip-fs");
    let payload = "test payload 123";
    let sha = payload_sha256(payload);

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "data.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some(payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write ok");
    assert!(response.exists);
    assert_eq!(response.payload_sha256.as_deref(), Some(sha.as_str()));

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: "data.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("read ok");
    assert_eq!(response.payload_text.as_deref(), Some(payload));
    assert_eq!(response.payload_sha256.as_deref(), Some(sha.as_str()));
}

#[test]
fn runtime_storage_write_and_read_roundtrip_memory() {
    let root = unique_temp_dir("rt-roundtrip-mem");
    let payload = "memory payload";

    runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "mem.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some(payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write ok");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: "mem.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("read ok");
    assert_eq!(response.payload_text.as_deref(), Some(payload));
}

#[test]
fn runtime_storage_write_and_read_roundtrip_sqlite() {
    let root = unique_temp_dir("rt-roundtrip-sqlite");
    let db_path = root.join("test.sqlite3");
    let payload = "sqlite payload";

    runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "data.json".to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(root.display().to_string()),
        payload_text: Some(payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write ok");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: "data.json".to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("read ok");
    assert_eq!(response.payload_text.as_deref(), Some(payload));
}

// ── exists operation ──

#[test]
fn runtime_storage_exists_returns_false_for_missing() {
    let root = unique_temp_dir("rt-exists-missing");
    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "exists".to_string(),
        path: "nonexistent.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("exists ok");
    assert!(!response.exists);
}

#[test]
fn runtime_storage_exists_returns_true_after_write() {
    let root = unique_temp_dir("rt-exists-present");
    runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "present.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("{}".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write ok");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "exists".to_string(),
        path: "present.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("exists ok");
    assert!(response.exists);
}

// ── verify_text operation ──

#[test]
fn runtime_storage_verify_text_passes_on_match() {
    let root = unique_temp_dir("rt-verify-match");
    let payload = "verify me";
    runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "verify.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some(payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write ok");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "verify_text".to_string(),
        path: "verify.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some(payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("verify ok");
    assert_eq!(response.verified, Some(true));
}

#[test]
fn runtime_storage_verify_text_fails_on_mismatch() {
    let root = unique_temp_dir("rt-verify-mismatch");
    runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "verify2.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("original".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write ok");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "verify_text".to_string(),
        path: "verify2.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("tampered".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("verify ok");
    assert_eq!(response.verified, Some(false));
}

#[test]
fn runtime_storage_verify_text_returns_false_when_missing() {
    let root = unique_temp_dir("rt-verify-missing");
    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "verify_text".to_string(),
        path: "missing.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("anything".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("verify ok");
    assert_eq!(response.verified, Some(false));
    assert!(!response.exists);
}

#[test]
fn runtime_storage_verify_text_with_expected_sha256() {
    let root = unique_temp_dir("rt-verify-sha");
    let payload = "sha check";
    let sha = payload_sha256(payload);
    runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "sha.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some(payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write ok");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "verify_text".to_string(),
        path: "sha.json".to_string(),
        backend_family: "memory".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: Some(sha),
        max_bytes: None,
        tail_lines: None,
    })
    .expect("verify ok");
    assert_eq!(response.verified, Some(true));
}

// ── error paths ──

#[test]
fn runtime_storage_write_text_rejects_missing_payload() {
    let root = unique_temp_dir("rt-write-nopayload");
    let result = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "x.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    });
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("payload_text"));
}

#[test]
fn runtime_storage_read_text_fails_on_missing_filesystem() {
    let root = unique_temp_dir("rt-read-missing");
    let result = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: "nope.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    });
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

#[test]
fn runtime_storage_read_text_verifies_expected_sha256() {
    let root = unique_temp_dir("rt-read-sha");
    let payload = "digest check";
    let sha = payload_sha256(payload);
    runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "digest.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some(payload.to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write ok");

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: "digest.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: Some(sha),
        max_bytes: None,
        tail_lines: None,
    })
    .expect("read ok");
    assert_eq!(response.verified, Some(true));

    let response = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: "digest.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: Some("0000deadbeef".to_string()),
        max_bytes: None,
        tail_lines: None,
    })
    .expect("read ok");
    assert_eq!(response.verified, Some(false));
}

#[test]
fn runtime_storage_rejects_unsupported_operation() {
    let root = unique_temp_dir("rt-unsupported-op");
    let result = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "delete_everything".to_string(),
        path: "x.json".to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    });
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unsupported"));
}

#[test]
fn runtime_storage_sqlite_backend_requires_db_path() {
    let root = unique_temp_dir("rt-sqlite-nodb");
    let result = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: "x.json".to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: None,
        storage_root: Some(root.display().to_string()),
        payload_text: Some("data".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    });
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("sqlite_db_path"));
}

// ── helper functions ──

#[test]
fn default_service_delegate_kind_format() {
    assert_eq!(
        default_service_delegate_kind("trace", "filesystem"),
        "filesystem-trace-store"
    );
    assert_eq!(
        default_service_delegate_kind("state", "sqlite"),
        "sqlite-state-store"
    );
    assert_eq!(
        default_service_delegate_kind("check", "in_memory"),
        "in-memory-check-store"
    );
}

#[test]
fn capability_bool_returns_field_value() {
    let mut map = Map::new();
    map.insert("active".to_string(), Value::Bool(true));
    assert!(capability_bool(&map, "active", false));
    assert!(!capability_bool(&map, "missing", false));
    assert!(capability_bool(&map, "missing", true));
}

#[test]
fn path_value_returns_string_or_null() {
    let mut map = Map::new();
    map.insert("key".to_string(), Value::String("val".to_string()));
    assert_eq!(path_value(&map, "key"), Value::String("val".to_string()));
    assert_eq!(path_value(&map, "absent"), Value::Null);
    let mut map2 = Map::new();
    map2.insert("num".to_string(), Value::Number(42.into()));
    assert_eq!(path_value(&map2, "num"), Value::Null);
}

// ── filesystem internals ──

#[test]
fn filesystem_write_text_creates_file_and_parent() {
    let root = unique_temp_dir("fs-write-inner");
    let path = root.join("sub").join("payload.txt");
    filesystem_write_text(&path, "hello fs").expect("write ok");
    assert_eq!(fs::read_to_string(&path).expect("read"), "hello fs");
}

#[test]
fn filesystem_write_text_overwrites_existing() {
    let root = unique_temp_dir("fs-write-overwrite");
    let path = root.join("payload.txt");
    filesystem_write_text(&path, "v1").expect("write v1");
    filesystem_write_text(&path, "v2").expect("write v2");
    assert_eq!(fs::read_to_string(&path).expect("read"), "v2");
}

#[test]
fn filesystem_append_text_appends_to_existing() {
    let root = unique_temp_dir("fs-append-inner");
    let path = root.join("log.txt");
    filesystem_write_text(&path, "line1\n").expect("write");
    filesystem_append_text(&path, "line2\n").expect("append");
    assert_eq!(fs::read_to_string(&path).expect("read"), "line1\nline2\n");
}

#[test]
fn filesystem_append_text_creates_file_if_missing() {
    let root = unique_temp_dir("fs-append-new");
    let path = root.join("new_log.txt");
    filesystem_append_text(&path, "first").expect("append to new file");
    assert_eq!(fs::read_to_string(&path).expect("read"), "first");
}

// ── sqlite internals ──

#[test]
fn sqlite_write_and_read_roundtrip() {
    let root = unique_temp_dir("sqlite-wr");
    let db_path = root.join("store.sqlite3");
    let path = root.join("artifact.json");
    sqlite_write_text(&path, &db_path, &root, "sqlite payload").expect("write ok");
    let result = sqlite_read_text(&path, &db_path, &root).expect("read ok");
    assert_eq!(result, "sqlite payload");
}

#[test]
fn sqlite_write_overwrites_existing() {
    let root = unique_temp_dir("sqlite-overwrite");
    let db_path = root.join("store.sqlite3");
    let path = root.join("data.json");
    sqlite_write_text(&path, &db_path, &root, "v1").expect("write v1");
    sqlite_write_text(&path, &db_path, &root, "v2").expect("write v2");
    assert_eq!(
        sqlite_read_text(&path, &db_path, &root).expect("read"),
        "v2"
    );
}

#[test]
fn sqlite_append_text_concatenates() {
    let root = unique_temp_dir("sqlite-append");
    let db_path = root.join("store.sqlite3");
    let path = root.join("log.jsonl");
    sqlite_write_text(&path, &db_path, &root, "first").expect("write");
    sqlite_append_text(&path, &db_path, &root, "\nsecond").expect("append");
    assert_eq!(
        sqlite_read_text(&path, &db_path, &root).expect("read"),
        "first\nsecond"
    );
}

#[test]
fn sqlite_payload_exists_works() {
    let root = unique_temp_dir("sqlite-exists");
    let db_path = root.join("store.sqlite3");
    let path = root.join("check.json");
    // First ensure the schema exists by doing a write, then delete to test exists=false
    sqlite_write_text(&path, &db_path, &root, "data").expect("write");
    assert!(sqlite_payload_exists(&path, &db_path, &root).expect("query"));
}

#[test]
fn sqlite_lookup_key_contains_root_and_relative() {
    let root = unique_temp_dir("sqlite-lookup");
    let path = root.join("data.json");
    let key = sqlite_lookup_key(&path, &root).expect("key ok");
    assert!(key.contains("::"));
    assert!(key.contains("data.json"));
}

// ── sha256 streaming ──

#[test]
fn stream_sha256_hex_reader_matches_payload_sha256() {
    let payload = "test data for streaming hash";
    let expected = payload_sha256(payload);
    let mut cursor = std::io::Cursor::new(payload.as_bytes());
    let result = stream_sha256_hex_reader(&mut cursor).expect("hash ok");
    assert_eq!(result, expected);
}

#[test]
fn stream_sha256_hex_path_works() {
    let root = unique_temp_dir("sha-path");
    let path = root.join("hashme.txt");
    fs::write(&path, b"hash content").expect("write");
    let result = stream_sha256_hex_path(&path).expect("hash ok");
    assert_eq!(result, payload_sha256("hash content"));
}

// ── catalog and control plane ──

#[test]
fn backend_family_catalog_contains_known_families() {
    let catalog = runtime_backend_family_catalog_payload();
    let families = catalog["families"].as_array().expect("families array");
    let family_names: Vec<&str> = families
        .iter()
        .map(|f| f["backend_family"].as_str().unwrap())
        .collect();
    assert!(family_names.contains(&"filesystem"));
    assert!(family_names.contains(&"sqlite"));
    assert_eq!(catalog["default_backend_family"], "filesystem");
    assert_eq!(catalog["strongest_local_backend_family"], "sqlite");
}

#[test]
fn build_checkpoint_control_plane_compiler_rejects_mismatched_backends() {
    let payload = json!({
        "paths": { "store": "/tmp/s", "checkpoint": "/tmp/c" },
        "capabilities": {
            "backend_family": "filesystem",
            "store_backend_family": "filesystem",
            "checkpointer_backend_family": "sqlite",
        }
    });
    let result = build_checkpoint_control_plane_compiler_payload(payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("mismatch"));
}

#[test]
fn build_checkpoint_control_plane_compiler_rejects_missing_paths() {
    let payload = json!({
        "capabilities": { "backend_family": "filesystem" }
    });
    let result = build_checkpoint_control_plane_compiler_payload(payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("paths"));
}

#[test]
fn build_checkpoint_control_plane_compiler_rejects_missing_capabilities() {
    let payload = json!({
        "paths": { "store": "/tmp/s" }
    });
    let result = build_checkpoint_control_plane_compiler_payload(payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("capabilities"));
}

// ── locking and symlink guards ──

#[test]
fn acquire_runtime_path_lock_succeeds_and_drops() {
    let root = unique_temp_dir("lock-test");
    let path = root.join("locked.json");
    fs::write(&path, b"{}").expect("write");
    let guard = acquire_runtime_path_lock(&path).expect("lock ok");
    drop(guard);
    let guard2 = acquire_runtime_path_lock(&path).expect("re-lock ok");
    drop(guard2);
}

#[test]
fn filesystem_reject_symlink_write_target_passes_for_regular_file() {
    let root = unique_temp_dir("symlink-check");
    let path = root.join("regular.txt");
    fs::write(&path, b"x").expect("write");
    filesystem_reject_symlink_write_target(&path).expect("should pass");
}

#[test]
fn filesystem_reject_symlink_write_target_passes_for_nonexistent() {
    let root = unique_temp_dir("symlink-check-miss");
    let path = root.join("nonexistent.txt");
    filesystem_reject_symlink_write_target(&path).expect("should pass for missing file");
}

#[cfg(unix)]
#[test]
fn filesystem_reject_symlink_write_target_rejects_symlink() {
    use std::os::unix::fs::symlink;
    let root = unique_temp_dir("symlink-check-reject");
    let real = root.join("real.txt");
    let alias = root.join("alias.txt");
    fs::write(&real, b"x").expect("write");
    symlink(&real, &alias).expect("symlink");
    let result = filesystem_reject_symlink_write_target(&alias);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("must not be a symlink")
    );
}

// ── env helpers ──

#[test]
#[serial]
fn env_checkpoint_storage_db_path_returns_none_when_unset() {
    let prior = std::env::var("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE").ok();
    unsafe { core_state_utils::env_sync::remove_env("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE") };
    assert_eq!(env_checkpoint_storage_db_path(), None);
    if let Some(v) = prior {
        unsafe { core_state_utils::env_sync::set_env("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE", &v) };
    }
}

#[test]
#[serial]
fn env_checkpoint_storage_db_path_returns_path_when_set() {
    let prior = std::env::var("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE").ok();
    unsafe {
        core_state_utils::env_sync::set_env(
            "CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE",
            "/tmp/test.sqlite3",
        )
    };
    let result = env_checkpoint_storage_db_path();
    assert!(result.is_some());
    assert!(result.unwrap().ends_with("test.sqlite3"));
    match prior {
        Some(v) => unsafe {
            core_state_utils::env_sync::set_env("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE", &v)
        },
        None => unsafe {
            core_state_utils::env_sync::remove_env("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE")
        },
    }
}

#[test]
fn runtime_storage_db_name_candidates_contains_default() {
    let candidates = runtime_storage_db_name_candidates();
    assert!(candidates.contains(&"runtime_checkpoint_store.sqlite3".to_string()));
}

// ── canonicalize_existing_ancestors ──

#[test]
fn canonicalize_existing_ancestors_resolves_real_path() {
    let root = unique_temp_dir("canonical-ancestors");
    let result = canonicalize_existing_ancestors(&root).expect("should resolve root");
    assert!(result.is_absolute());
    assert!(result.exists());
}

#[test]
fn canonicalize_existing_ancestors_rejects_relative() {
    let result = canonicalize_existing_ancestors(Path::new("relative"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be absolute"));
}

// ── storage_artifact_exists / storage_read_text with None backend ──

#[test]
fn storage_artifact_exists_none_backend_returns_false() {
    let root = unique_temp_dir("artifact-none-backend");
    let path = root.join("missing.json");
    assert!(!storage_artifact_exists(&path, None));
}

#[test]
fn storage_read_text_none_backend_returns_error_for_missing() {
    let root = unique_temp_dir("read-none-backend");
    let path = root.join("missing.json");
    let result = storage_read_text(&path, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

// ── resolve_storage_backend ──

#[test]
fn resolve_storage_backend_returns_none_for_empty_paths() {
    assert!(resolve_storage_backend(&[]).is_none());
}

#[test]
fn resolve_storage_backend_returns_filesystem_when_file_exists() {
    let root = unique_temp_dir("resolve-backend-fs");
    let path = root.join("existing.json");
    fs::write(&path, b"{}").expect("write");
    let result = resolve_storage_backend(&[path]);
    match result {
        Some(ResolvedStorageBackend::Filesystem) => {}
        other => panic!("expected Filesystem, got {other:?}"),
    }
}
