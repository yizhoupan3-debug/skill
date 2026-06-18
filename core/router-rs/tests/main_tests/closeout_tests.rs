use super::common::*;
use super::*;

use serde_json::json;


#[test]
fn framework_session_artifact_write_blocks_completion_without_closeout_record() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _strict = CloseoutStrictEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-missing");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-missing",
        "task": "Closeout missing",
        "phase": "validation",
        "status": "completed",
        "summary": "claimed done with no closeout record",
        "focus": true,
        "next_actions": []
    }))
    .expect_err("missing closeout_record must block completion claim");
    assert!(
        err.contains("closeout_record"),
        "error must reference missing closeout_record, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_blocks_completion_without_closeout_when_ci_unsets_env() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _ci = CiHardUnsetCloseoutEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-ci-unset-env");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-ci-unset",
        "task": "CI unset closeout env",
        "phase": "validation",
        "status": "completed",
        "summary": "claimed done with no closeout record under CI",
        "focus": true,
        "next_actions": []
    }))
    .expect_err("missing closeout_record must block completion claim when CI without explicit env");
    assert!(
        err.contains("closeout_record"),
        "error must reference missing closeout_record, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_blocks_completion_without_closeout_when_github_actions_unsets_env(
) {
    let _lock = crate::test_env_sync::process_env_lock();
    let _ga = GithubActionsHardUnsetCloseoutEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-gha-unset-env");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-gha-unset",
        "task": "GHA unset closeout env",
        "phase": "validation",
        "status": "completed",
        "summary": "claimed done with no closeout record under GITHUB_ACTIONS",
        "focus": true,
        "next_actions": []
    }))
    .expect_err(
        "missing closeout_record must block completion claim when GITHUB_ACTIONS without explicit env",
    );
    assert!(
        err.contains("closeout_record"),
        "error must reference missing closeout_record, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_allows_completion_without_closeout_when_ci_and_closeout_env_off(
) {
    let _lock = crate::test_env_sync::process_env_lock();
    let _ci_off = CiWithCloseoutDisabledEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-ci-with-env-off");
    let output_dir = repo_root.join("artifacts").join("current");
    let written = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-ci-env-off",
        "task": "CI but closeout enforcement off",
        "phase": "validation",
        "status": "completed",
        "summary": "CI with ROUTER_RS_CLOSEOUT_ENFORCEMENT=0, no closeout_record",
        "focus": true,
        "next_actions": []
    }))
    .expect(
        "completion write should succeed when CI but explicit closeout env disables enforcement",
    );
    assert!(
        written.get("closeout_evaluation").is_none(),
        "expected no closeout_evaluation when enforcement skipped, got: {written}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_blocks_completion_without_closeout_when_closeout_env_empty_string(
) {
    let _lock = crate::test_env_sync::process_env_lock();
    let _empty = LocalNonCiEmptyCloseoutEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-empty-env");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-empty-env",
        "task": "Empty ROUTER_RS_CLOSEOUT_ENFORCEMENT",
        "phase": "validation",
        "status": "completed",
        "summary": "non-CI with empty string closeout env",
        "focus": true,
        "next_actions": []
    }))
    .expect_err("empty ROUTER_RS_CLOSEOUT_ENFORCEMENT must not be treated as unset/local-soft");
    assert!(
        err.contains("closeout_record"),
        "error must reference missing closeout_record, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_allows_completion_without_closeout_when_env_disables() {
    struct EnvCloseoutGuard {
        prior: Option<String>,
    }
    impl EnvCloseoutGuard {
        fn set(value: &str) -> Self {
            let prior = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
            unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", value) };
            Self { prior }
        }
    }
    impl Drop for EnvCloseoutGuard {
        fn drop(&mut self) {
            match &self.prior {
                Some(v) => unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
                None => unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
            }
        }
    }

    let _lock = crate::test_env_sync::process_env_lock();
    let _guard = EnvCloseoutGuard::set("0");
    let repo_root = temp_dir_path("framework-session-closeout-env-off");
    let output_dir = repo_root.join("artifacts").join("current");
    let written = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-env-off",
        "task": "Closeout env off",
        "phase": "validation",
        "status": "completed",
        "summary": "personal mode without closeout_record",
        "focus": true,
        "next_actions": []
    }))
    .expect("completion write should succeed when closeout enforcement is disabled by env");
    assert!(
        written.get("closeout_evaluation").is_none(),
        "expected no closeout_evaluation when enforcement skipped"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_in_progress_ignores_malformed_closeout_record() {
    // In-progress checkpoints often carry partial / draft closeout records
    // that would fail strict deny_unknown_fields parsing. The artifact
    // write path must NOT block in-progress writes on incidental record
    // malformation: pre-completion validation is the caller's job.
    let repo_root = temp_dir_path("framework-session-closeout-inprogress");
    let output_dir = repo_root.join("artifacts").join("current");
    let response = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-inprog",
        "task": "Closeout in-progress",
        "phase": "execution",
        "status": "in_progress",
        "summary": "still working",
        "focus": true,
        "next_actions": [],
        // Unknown field would normally trip deny_unknown_fields, but the
        // record must be ignored when status is not a completion claim.
        "closeout_record": {
            "schema_version": "closeout-record-v1",
            "task_id": "co-inprog",
            "verification_status": "not_run",
            "summary": "draft",
            "unexpected_extension_field": "ignored on in-progress"
        }
    }))
    .expect("in-progress write must succeed even with malformed record");
    assert!(
        response.get("closeout_evaluation").is_none(),
        "in-progress writes must not attach closeout_evaluation, got: {response}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_blocks_completion_with_failed_command() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _strict = CloseoutStrictEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-bad");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-bad",
        "task": "Closeout bad",
        "phase": "validation",
        "status": "passed",
        "summary": "Done with failing command",
        "focus": true,
        "next_actions": [],
        // verification_status=passed but a recorded command exited 1.
        // closeout_enforcement R3 must block this claim.
        "closeout_record": {
            "schema_version": "closeout-record-v1",
            "task_id": "co-bad",
            "verification_status": "passed",
            "summary": "Done with failing command",
            "commands_run": [
                {"command": "cargo test", "exit_code": 1}
            ]
        }
    }))
    .expect_err("failed command in passed record must block completion");
    assert!(
        err.contains("closeout_enforcement blocked"),
        "error must reference closeout enforcement block, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


