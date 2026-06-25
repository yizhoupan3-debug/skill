use crate::types::{
    AntiDriftState, CircuitBreaker, CurrentRun, LoopError, LoopPhase, LoopRunState, RunHistoryEntry,
};
use std::fs;
use std::path::{Path, PathBuf};

/// Canonical `now_iso` — re-exported from `framework-kernel`.
pub use framework_kernel::time::now_iso;

/// Filename used for persisting loop run state: `LOOP_RUN_STATE.json`.
pub const LOOP_RUN_STATE_FILENAME: &str = "LOOP_RUN_STATE.json";
/// Schema version string written into every LOOP_RUN_STATE.json: `loop-run-state-v1`.
pub const LOOP_RUN_STATE_SCHEMA_VERSION: &str = "loop-run-state-v1";
/// Filename for the exclusive loop lock file: `.loop-active`.
pub const LOOP_LOCK_FILENAME: &str = ".loop-active";
/// Maximum age (in seconds) before a loop lock is considered stale and can be overridden.
pub const LOOP_LOCK_MAX_AGE_SECS: u64 = 3600;

/// Return the path to `LOOP_RUN_STATE.json` for the given loop under `artifacts/loop/{loop_id}/`.
pub fn loop_state_path(repo_root: &Path, loop_id: &str) -> PathBuf {
    repo_root
        .join("artifacts")
        .join("loop")
        .join(loop_id)
        .join(LOOP_RUN_STATE_FILENAME)
}

/// Return the artifacts directory path for a loop: `artifacts/loop/{loop_id}/`.
pub fn loop_artifacts_dir(repo_root: &Path, loop_id: &str) -> PathBuf {
    repo_root.join("artifacts").join("loop").join(loop_id)
}

/// Return the evidence directory path for a specific action: `artifacts/loop/{loop_id}/evidence/{action_id}/`.
pub fn loop_evidence_dir(repo_root: &Path, loop_id: &str, action_id: &str) -> PathBuf {
    loop_artifacts_dir(repo_root, loop_id)
        .join("evidence")
        .join(action_id)
}

/// Return the reports directory path for a loop: `artifacts/loop/{loop_id}/reports/`.
pub fn loop_reports_dir(repo_root: &Path, loop_id: &str) -> PathBuf {
    loop_artifacts_dir(repo_root, loop_id).join("reports")
}

/// Return the closeout file path for a specific action in a run:
/// `artifacts/loop/{loop_id}/closeout/{run_id}-{action_id}.json`.
pub fn closeout_path(repo_root: &Path, loop_id: &str, run_id: &str, action_id: &str) -> PathBuf {
    repo_root
        .join("artifacts")
        .join("loop")
        .join(loop_id)
        .join("closeout")
        .join(format!("{run_id}-{action_id}.json"))
}

/// Return the path to the loop lock file: `.loop-active` in the repo root.
pub fn lock_path(repo_root: &Path) -> PathBuf {
    repo_root.join(LOOP_LOCK_FILENAME)
}

/// Return the kill signal file path for a loop: `.loop-kill/{loop_id}`.
pub fn kill_signal_path(repo_root: &Path, loop_id: &str) -> PathBuf {
    repo_root.join(".loop-kill").join(loop_id)
}

/// Read the persisted loop run state from `LOOP_RUN_STATE.json` on disk.
/// Returns `Ok(None)` when the file does not exist.
pub fn read_loop_state(repo_root: &Path, loop_id: &str) -> Result<Option<LoopRunState>, LoopError> {
    let path = loop_state_path(repo_root, loop_id);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| LoopError::Io(format!("read {}: {e}", path.display())))?;
    let state: LoopRunState = serde_json::from_str(&raw)
        .map_err(|e| LoopError::Serde(format!("parse {}: {e}", path.display())))?;
    Ok(Some(state))
}

/// Atomically write the loop run state to `LOOP_RUN_STATE.json` using
/// core-state's canonical atomic write (fsync + POSIX rename).
pub fn write_loop_state(
    repo_root: &Path,
    loop_id: &str,
    state: &LoopRunState,
) -> Result<(), LoopError> {
    let path = loop_state_path(repo_root, loop_id);
    let text = serde_json::to_string_pretty(state)
        .map_err(|e| LoopError::Serde(format!("serialize state: {e}")))?;
    core_state_utils::atomic_write::write_atomic_text(&path, &text).map_err(LoopError::Io)
}

/// Create a new initial `LoopRunState` with the given loop ID and profile.
/// The phase is set to `Pending` with no active run and an empty history.
pub fn create_initial_state(loop_id: &str, profile: &str) -> LoopRunState {
    let now = now_iso();
    LoopRunState {
        schema_version: LOOP_RUN_STATE_SCHEMA_VERSION.to_string(),
        loop_id: loop_id.to_string(),
        profile: profile.to_string(),
        phase: LoopPhase::Pending.as_str().to_string(),
        last_heartbeat: now.clone(),
        current_run: None,
        history: Vec::new(),
        circuit_breaker: CircuitBreaker::default(),
        anti_drift: AntiDriftState::default(),
        last_refreshed_at: now,
    }
}

/// Transition the loop runner to a new phase, updating the heartbeat and refresh timestamp.
pub fn transition_phase(state: &mut LoopRunState, new_phase: LoopPhase) {
    state.phase = new_phase.as_str().to_string();
    state.last_heartbeat = now_iso();
    state.last_refreshed_at = now_iso();
}

/// Initialise a new run within the loop state, setting the run ID and started-at timestamp.
pub fn start_new_run(state: &mut LoopRunState, run_id: &str) {
    state.current_run = Some(CurrentRun {
        run_id: run_id.to_string(),
        started_at: now_iso(),
        discovery: None,
        unconsumed_findings: Vec::new(),
        dispatch: std::collections::HashMap::new(),
        closeout_aggregate: None,
        report_path: None,
    });
}

/// Mark the current run as finished, archiving it to the run history and clearing the current run.
pub fn finish_run(state: &mut LoopRunState, result: &str) {
    if let Some(ref run) = state.current_run {
        state.history.push(RunHistoryEntry {
            run_id: run.run_id.clone(),
            phase: state.phase.clone(),
            result: result.to_string(),
        });
    }
    state.current_run = None;
}

/// Generate a unique run ID string in the format `run-{YYYYMMDD}-{HHMM}-{SS}`.
pub fn generate_run_id(_loop_id: &str) -> String {
    let now = chrono::Utc::now();
    format!(
        "run-{}-{}-{}",
        now.format("%Y%m%d"),
        now.format("%H%M"),
        now.format("%S")
    )
}

/// Update the heartbeat timestamp of the loop run state to the current time.
pub fn update_heartbeat(state: &mut LoopRunState) {
    state.last_heartbeat = now_iso();
    state.last_refreshed_at = now_iso();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_initial_state() {
        let state = create_initial_state("test-loop", "loop-auto");
        assert_eq!(state.loop_id, "test-loop");
        assert_eq!(state.profile, "loop-auto");
        assert_eq!(state.phase, "pending");
        assert!(state.current_run.is_none());
        assert!(state.history.is_empty());
    }

    #[test]
    fn test_state_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let loop_id = "roundtrip-test";

        let mut state = create_initial_state(loop_id, "loop-auto");
        start_new_run(&mut state, &generate_run_id(loop_id));
        transition_phase(&mut state, LoopPhase::Discovering);
        write_loop_state(root, loop_id, &state).unwrap();

        let loaded = read_loop_state(root, loop_id).unwrap().unwrap();
        assert_eq!(loaded.loop_id, loop_id);
        assert_eq!(loaded.phase, "discovering");
        assert!(loaded.current_run.is_some());
    }

    #[test]
    fn test_read_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let result = read_loop_state(tmp.path(), "no-such-loop").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_finish_run() {
        let mut state = create_initial_state("t", "loop-auto");
        start_new_run(&mut state, "run-1");
        transition_phase(&mut state, LoopPhase::Running);
        finish_run(&mut state, "success");

        assert!(state.current_run.is_none());
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].result, "success");
    }

    #[test]
    fn test_generate_run_id_format() {
        let id = generate_run_id("my-loop");
        assert!(id.starts_with("run-"));
        assert!(id.contains('-'));
    }

    #[test]
    fn test_loop_lock_path() {
        let tmp = TempDir::new().unwrap();
        let p = lock_path(tmp.path());
        assert_eq!(p.file_name().unwrap(), ".loop-active");
    }

    #[test]
    fn test_kill_signal_path() {
        let tmp = TempDir::new().unwrap();
        let p = kill_signal_path(tmp.path(), "daily-triage");
        assert!(p.to_string_lossy().contains(".loop-kill"));
        assert!(p.to_string_lossy().contains("daily-triage"));
    }
}
