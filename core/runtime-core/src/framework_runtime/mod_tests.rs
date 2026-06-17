//! Tests for framework_runtime module.

use super::*;

mod post_tool_duration_tests {
    use super::{extract_post_tool_duration_ms, post_tool_call_succeeded};
    use serde_json::json;

    #[test]
    fn extract_post_tool_duration_ms_reads_top_level_and_nested_fields() {
        assert_eq!(
            extract_post_tool_duration_ms(&json!({"duration_ms": 42})),
            Some(42)
        );
        assert_eq!(
            extract_post_tool_duration_ms(&json!({"durationMs": "99"})),
            Some(99)
        );
        assert_eq!(
            extract_post_tool_duration_ms(&json!({
                "tool_output": { "duration_ms": 7 }
            })),
            Some(7)
        );
        assert_eq!(
            extract_post_tool_duration_ms(&json!({
                "tool_output": "{\"durationMs\": 15}"
            })),
            Some(15)
        );
        assert_eq!(extract_post_tool_duration_ms(&json!({})), None);
    }

    #[test]
    fn post_tool_call_succeeded_honors_error_and_exit_code() {
        assert!(!post_tool_call_succeeded(&json!({"is_error": true})));
        assert!(!post_tool_call_succeeded(&json!({"exit_code": 1})));
        assert!(post_tool_call_succeeded(&json!({"exit_code": 0})));
        assert!(post_tool_call_succeeded(&json!({"tool_name": "Read"})));
    }
}

mod evidence_lock_order_tests {
    // NOTE: These tests are fragile — they read source code as text via include_str!
    // and search for function/variable names. Refactoring or renaming may break them
    // even if the runtime behavior is unchanged.

    #[test]
    fn append_evidence_index_merged_row_does_not_use_task_ledger_flock() {
        let src = include_str!("mod.rs");
        let start = src
            .find("fn append_evidence_index_merged_row")
            .expect("append_evidence_index_merged_row");
        let rest = &src[start..];
        let end = rest
            .find("\npub fn framework_hook_evidence_append")
            .unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(
            !body.contains("apply_task_ledger_mutation"),
            "evidence append must not nest task-ledger flock (L3→L1 deadlock risk)"
        );
        assert!(
            body.contains("acquire_runtime_path_lock"),
            "evidence append must use path flock"
        );
    }

    #[test]
    fn append_evidence_index_merged_row_does_not_call_append_transaction_under_l2() {
        let src = include_str!("mod.rs");
        let start = src
            .find("fn append_evidence_index_merged_row")
            .expect("append_evidence_index_merged_row");
        let rest = &src[start..];
        let end = rest
            .find("\npub fn framework_hook_evidence_append")
            .unwrap_or(rest.len());
        let body = &rest[..end];
        let lock_pos = body
            .find("acquire_runtime_path_lock")
            .expect("acquire_runtime_path_lock");
        let append_pos = body
            .find("append_transaction(")
            .expect("append_transaction after L2 block");
        let l2_block_end = body[lock_pos..append_pos]
            .rfind('}')
            .expect("L2 block closes before append_transaction");
        assert!(
            lock_pos + l2_block_end < append_pos,
            "append_transaction must run only after L2 path lock is released"
        );
        assert!(
            !body[lock_pos..lock_pos + l2_block_end].contains("append_transaction("),
            "must not call append_transaction while holding L2 lock"
        );
    }
}

#[cfg(test)]
mod resolve_repo_root_tests {
    use super::resolve_repo_root_arg;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn resolve_repo_root_walks_up_from_scripts_router_rs_subdir() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-root-resolve-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        fs::write(
            tmp.join("configs/framework/RUNTIME_REGISTRY.json"),
            r#"{"schema_version":"framework-runtime-registry-v2","framework_commands":{}}"#,
        )
        .unwrap();
        fs::create_dir_all(tmp.join("core/router-rs/src")).unwrap();
        fs::write(
            tmp.join("core/router-rs/Cargo.toml"),
            "[package]\nname = \"router-rs\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let subdir = tmp.join("core/router-rs/src");
        let resolved = resolve_repo_root_arg(Some(subdir.as_path())).unwrap();
        let expect = tmp.canonicalize().unwrap_or_else(|_| tmp.clone());
        assert_eq!(resolved, expect);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_repo_root_unchanged_when_no_framework_markers() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-no-marker-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&tmp).unwrap();
        let resolved = resolve_repo_root_arg(Some(tmp.as_path())).unwrap();
        let expect = tmp.canonicalize().unwrap_or_else(|_| tmp.clone());
        assert_eq!(resolved, expect);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_repo_root_from_cargo_manifest_dir_matches_framework_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let resolved = resolve_repo_root_arg(Some(manifest_dir.as_path())).unwrap();
        let expect = manifest_dir
            .join("../..")
            .canonicalize()
            .expect("skill repo root should resolve");
        assert_eq!(
            resolved, expect,
            "router-rs crate cwd must resolve to framework repo root for continuity/RUNTIME_REGISTRY"
        );
    }
}

#[cfg(test)]
mod truncate_utf8_tests {
    use super::truncate_utf8_chars;

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate_utf8_chars("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate_utf8_chars("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_string_cut() {
        let result = truncate_utf8_chars("hello world", 5);
        assert!(result.len() <= 5);
        assert!(result.starts_with("hello"));
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate_utf8_chars("", 10), "");
    }

    #[test]
    fn truncate_multibyte_utf8() {
        // "你好世界" = 4 chars, 12 bytes; max_chars=3 should keep 3 chars
        let result = truncate_utf8_chars("你好世界", 3);
        assert_eq!(result, "你好世");
    }
}

#[cfg(test)]
mod detect_verify_tests {
    use super::detect_and_verify_physical_artifact;
    use std::fs;
    use std::path::Path;

    #[test]
    fn detect_file_path_returns_true() {
        let dir = std::env::temp_dir().join("runtime_core_test_detect");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join("output.txt");
        fs::write(&file, "test").unwrap();
        assert!(detect_and_verify_physical_artifact(
            &dir,
            &format!("cat {}", file.display()).to_ascii_lowercase(),
        ));
        let _ = fs::remove_dir_all(&dir);
    }
}
