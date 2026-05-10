//! Global mutex for **continuity ledger** mutations under `artifacts/current/**`:
//! `GOAL_STATE.json`, `RFV_LOOP_STATE.json`, session artifact batch writes, and
//! `EVIDENCE_INDEX.json` appends.
//!
//! Phase 2 (`docs/task_state_unified_resolve.md`): one serialization boundary so concurrent
//! hosts / hooks cannot interleave partial updates across these files.

use std::sync::{Mutex, MutexGuard, OnceLock};

fn task_ledger_write_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) struct TaskLedgerWriteGuard<'a> {
    _guard: MutexGuard<'a, ()>,
}

impl TaskLedgerWriteGuard<'_> {
    pub fn acquire() -> Result<Self, String> {
        let guard = task_ledger_write_mutex()
            .lock()
            .map_err(|_| "task ledger write lock poisoned".to_string())?;
        Ok(Self { _guard: guard })
    }
}

/// Phase-2 **apply** boundary: run a closure while holding the ledger mutex (GOAL / RFV / session batch / evidence).
pub(crate) fn apply_task_ledger_mutation<T>(
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _guard = TaskLedgerWriteGuard::acquire()?;
    f()
}

/// Lower-level access for `framework_runtime` session writer / evidence append (same mutex).
pub(crate) fn task_ledger_write_lock() -> &'static Mutex<()> {
    task_ledger_write_mutex()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_serializes_on_same_thread() {
        let a = TaskLedgerWriteGuard::acquire().expect("a");
        drop(a);
        let _b = TaskLedgerWriteGuard::acquire().expect("b");
    }

    #[test]
    fn apply_runs_closure() {
        let v = apply_task_ledger_mutation(|| Ok(7_u8)).expect("apply");
        assert_eq!(v, 7);
    }
}
