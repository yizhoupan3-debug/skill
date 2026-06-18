use super::common::*;
use super::*;

use serde_json::json;
use std::fs;
use std::path::Path;


#[test]
fn runtime_storage_operation_round_trips_filesystem_payload() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let storage_root = std::env::temp_dir();
    let storage_root_text = storage_root.display().to_string();
    let path = storage_root.join(format!("router-rs-runtime-storage-{nonce}.txt"));
    let _ = fs::remove_file(&path);

    let write = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: path.display().to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(storage_root_text.clone()),
        payload_text: Some("alpha".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write payload");
    assert_eq!(write.schema_version, RUNTIME_STORAGE_SCHEMA_VERSION);
    assert_eq!(write.authority, RUNTIME_STORAGE_AUTHORITY);
    assert!(write.exists);
    assert_eq!(write.bytes_written, Some(5));
    assert_eq!(
        write.backend_capabilities["supports_atomic_replace"],
        json!(true)
    );
    assert_eq!(
        write.payload_sha256.as_deref(),
        Some("8ed3f6ad685b959ead7022518e1af76cd816f8e8ec7ccdda1ed4018e8f2223f8")
    );

    let append = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "append_text".to_string(),
        path: path.display().to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(storage_root_text.clone()),
        payload_text: Some("-beta".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("append payload");
    assert!(append.exists);
    assert_eq!(append.bytes_written, Some(5));
    assert_eq!(
        append.payload_sha256.as_deref(),
        Some("a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432")
    );

    let read = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: path.display().to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(storage_root_text.clone()),
        payload_text: None,
        expected_sha256: Some(
            "a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432".to_string(),
        ),
        max_bytes: None,
        tail_lines: None,
    })
    .expect("read payload");
    assert_eq!(read.payload_text.as_deref(), Some("alpha-beta"));
    assert_eq!(read.verified, Some(true));
    assert_eq!(
        read.payload_sha256.as_deref(),
        Some("a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432")
    );

    let verify = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "verify_text".to_string(),
        path: path.display().to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(storage_root_text),
        payload_text: None,
        expected_sha256: Some(
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        ),
        max_bytes: None,
        tail_lines: None,
    })
    .expect("verify payload");
    assert_eq!(verify.verified, Some(false));

    let _ = fs::remove_file(path);
}


#[test]
fn runtime_storage_operation_round_trips_sqlite_payload() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("router-rs-runtime-storage-root-{nonce}"));
    let db_path = root.join("runtime_checkpoint_store.sqlite3");
    let artifact_path = root.join("runtime-data").join("TRACE_RESUME_MANIFEST.json");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create sqlite root");

    let write = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: artifact_path.display().to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(root.display().to_string()),
        payload_text: Some("{\"status\":\"ok\"}".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("sqlite write payload");
    assert_eq!(write.backend_family, "sqlite");
    assert_eq!(
        write.backend_capabilities["supports_sqlite_wal"],
        json!(true)
    );
    assert_eq!(
        write.sqlite_db_path.as_deref(),
        Some(db_path.display().to_string().as_str())
    );
    assert_eq!(
        write.storage_root.as_deref(),
        Some(root.display().to_string().as_str())
    );
    assert!(db_path.exists());

    let read = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: artifact_path.display().to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("sqlite read payload");
    assert_eq!(read.payload_text.as_deref(), Some("{\"status\":\"ok\"}"));

    let _ = fs::remove_dir_all(root);
}


#[test]
fn runtime_checkpoint_control_plane_normalizes_backend_family_catalog() {
    let root = temp_dir_path("checkpoint-control-plane");
    let response = build_checkpoint_control_plane_compiler_payload(json!({
            "control_plane_descriptor": {
                "schema_version": "router-rs-runtime-control-plane-v1",
                "authority": "rust-runtime-control-plane",
                "services": {
                    "trace": {
                        "authority": "rust-runtime-control-plane",
                        "role": "trace-and-handoff",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-trace-store"
                    },
                    "state": {
                        "authority": "rust-runtime-control-plane",
                        "role": "durable-background-state",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-state-store"
                    }
                }
            },
            "capabilities": {
                "backend_family": "sqlite3",
                "store_backend_family": "sqlite",
                "trace_backend_family": "sqlite",
                "state_backend_family": "sqlite"
            },
            "paths": {
                "trace_output_path": root.join("TRACE_METADATA.json").display().to_string(),
                "event_stream_path": root.join("TRACE_EVENTS.jsonl").display().to_string(),
                "resume_manifest_path": root.join("TRACE_RESUME_MANIFEST.json").display().to_string(),
                "background_state_path": root.join("runtime_background_jobs.json").display().to_string(),
                "event_transport_dir": root.join("runtime_event_transports").display().to_string()
            }
        }))
        .expect("checkpoint control plane");
    let control_plane = &response["checkpoint_control_plane"];

    assert_eq!(control_plane["backend_family"], json!("sqlite"));
    assert_eq!(
        control_plane["trace_service"]["delegate_kind"],
        json!("filesystem-trace-store")
    );
    assert_eq!(
        control_plane["state_service"]["delegate_kind"],
        json!("filesystem-state-store")
    );
    assert_eq!(control_plane["supports_compaction"], json!(true));
    assert_eq!(control_plane["supports_snapshot_delta"], json!(true));
    assert_eq!(control_plane["supports_consistent_append"], json!(true));
    assert_eq!(control_plane["supports_sqlite_wal"], json!(true));
    assert_eq!(
        control_plane["backend_family_catalog"]["strongest_local_backend_family"],
        json!("sqlite")
    );
    assert_eq!(
        control_plane["backend_family_parity"]["aligned"],
        json!(true)
    );
    assert_eq!(
        control_plane["backend_family_parity"]["compaction_eligible"],
        json!(true)
    );
}


#[test]
fn runtime_checkpoint_control_plane_rejects_mixed_backend_families() {
    let root = temp_dir_path("checkpoint-control-plane-mismatch");
    let err = build_checkpoint_control_plane_compiler_payload(json!({
            "capabilities": {
                "backend_family": "sqlite",
                "store_backend_family": "filesystem"
            },
            "paths": {
                "background_state_path": root.join("runtime_background_jobs.json").display().to_string(),
                "event_transport_dir": root.join("runtime_event_transports").display().to_string()
            }
        }))
        .expect_err("mixed backend families should fail closed");

    assert!(err.contains("backend family mismatch"));
}

#[test]
fn write_text_payload_rejects_path_traversal_with_dotdot() {
    let traversal_path = Path::new("/tmp/legitimate/../../../etc/evil.conf");
    let result = write_text_payload(traversal_path, "malicious content");
    assert!(
        result.is_err(),
        "write_text_payload must reject '..' path traversal"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("must not contain '..' traversal segments"),
        "error should mention traversal rejection, got: {err}"
    );
}

#[test]
fn write_text_payload_rejects_relative_dotdot_traversal() {
    let traversal_path = Path::new("artifacts/../../../escape.txt");
    let result = write_text_payload(traversal_path, "escaped");
    assert!(
        result.is_err(),
        "write_text_payload must reject relative '..' traversal"
    );
}

#[test]
fn write_text_payload_rejects_symlink_write_target() {
    let dir = temp_dir_path("symlink-reject");
    fs::create_dir_all(&dir).expect("create test dir");
    let real_path = dir.join("real-target.txt");
    let symlink_path = dir.join("symlink-alias.txt");
    fs::write(&real_path, "original").expect("write real file");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&real_path, &symlink_path).expect("create symlink");
        let result = write_text_payload(&symlink_path, "via symlink");
        assert!(
            result.is_err(),
            "write_text_payload must reject symlink targets"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("must not be a symlink"),
            "error should mention symlink rejection, got: {err}"
        );
    }
    fs::remove_dir_all(&dir).expect("cleanup symlink test dir");
}

#[test]
fn write_text_payload_allows_valid_paths() {
    let output_path = temp_json_path("valid-path-write");
    let payload = "safe content\n";
    let bytes = write_text_payload(&output_path, payload).expect("valid path should succeed");
    assert_eq!(bytes, payload.len());
    let persisted = fs::read_to_string(&output_path).expect("read back");
    assert_eq!(persisted, payload);
    fs::remove_file(&output_path).expect("cleanup");
}

