export const meta = {
  name: 'migrate-framework-error',
  description: 'Migrate all Result<_, String> public APIs to Result<_, FrameworkError> across the workspace',
  phases: [
    { title: 'Leaf crates' },
    { title: 'Mid crates' },
    { title: 'High crates' },
    { title: 'Verify' },
  ],
}

phase('Leaf crates')

// Group 1: fr-utils (io_utils, json_io, util) — leaf crate, 8 fns
const group1 = await agent(`
面向用户的可见输出使用简体中文。
Migrate ALL pub fn returning Result<_, String> in core/fr-utils/src/ to return Result<_, FrameworkError>.

Import path: use core_errors::FrameworkError;

Files and functions to migrate:

core/fr-utils/src/io_utils.rs:
- pub fn validate_write_path(path: &Path, allowed_root: Option<&Path>) -> Result<(), String>
- pub fn append_text_with_process_lock(path: &Path, payload: &str, context: &str) -> Result<(), String>

core/fr-utils/src/json_io.rs:
- pub fn print_json_value<T: Serialize>(payload: &T) -> Result<(), String>
- pub fn parse_json_input<T>(raw: &str, context: &str) -> Result<T, String>

core/fr-utils/src/util.rs:
- pub fn write_text_if_changed_unlocked(path: &Path, content: &str) -> Result<bool, String>
- pub fn hash_file_for_test(path: &Path) -> Result<String, String>
- pub fn write_json_if_changed_unlocked(path: &Path, payload: &Value) -> Result<bool, String>
- pub fn required_payload_text(payload: &Value, key: &str, context: &str) -> Result<String, String>

For each:
1. Add "use core_errors::FrameworkError;" to the file
2. Change return type from Result<..., String> to Result<..., FrameworkError>
3. Replace Err(format!(...)) with Err(FrameworkError::Validation(format!(...)))
4. Replace Err("static string") or Err("string".to_string()) with Err(FrameworkError::Validation("string"))
5. Replace .map_err(|e| e.to_string()) with just removing it (the ? operator handles FrameworkError→FrameworkError conversion)
6. For any calls to functions that now return FrameworkError internally, just use ?

Also check: write_text_if_changed_unlocked returns Result<bool, String> — internal calls to write_atomic_text return Result<usize, FrameworkError>, so remove .map_err bridge.

After migrating, run: cargo check -p fr-utils 2>&1 | tail -10
If there are errors, fix them.
`, {phase: 'Leaf crates', schema: {type: 'object', properties: {status: {type: 'string'}, fileCount: {type: 'integer'}}}})

// Group 2: core-policy (hook_policy, registry_review_gate) — 2 fns, uses core_errors::FrameworkError
const group2 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn returning Result<_, String> in core/core-policy/src/ to return Result<_, FrameworkError>.

Import path: use core_errors::FrameworkError; (core-policy/error.rs already does: pub use core_errors::FrameworkError;)

Files and functions:

core/core-policy/src/hook_policy.rs:202
- pub fn evaluate_hook_policy_value(payload: Value) -> Result<Value, String>

core/core-policy/src/registry_review_gate.rs:329
- pub fn check_review_gate_registry_snapshot(repo_root: &Path) -> Result<(), String>

For each:
1. Add "use core_errors::FrameworkError;" if not present (check if it gets it from error.rs re-export)
2. Change return type
3. Replace Err(format!(...)) with Err(FrameworkError::Validation(format!(...)))
4. Remove .map_err(|e| e.to_string()) bridges internal to these functions
5. For internal calls to functions returning FrameworkError, just use ?

After: cargo check -p core-policy 2>&1 | tail -10
`, {phase: 'Leaf crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

// Group 3: core-state (step_ledger, task_ledger, task_state_aggregate) — 3 fns
const group3 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn returning Result<_, String> in core/core-state/src/ to return Result<_, FrameworkError>.

Import path: use core_errors::FrameworkError;

Files and functions:

core/core-state/src/step_ledger.rs:25
- pub fn handle_step_ledger_operation(payload: Value) -> Result<Value, String>

core/core-state/src/task_ledger.rs:26
- pub fn task_ledger_path(repo_root: &Path, task_id: &str) -> Result<PathBuf, String>

core/core-state/src/task_state_aggregate.rs:30
- pub fn sync_task_state_aggregate(repo_root: &Path, task_id: &str) -> Result<(), String>

For each:
1. Add "use core_errors::FrameworkError;" if not already present
2. Change return type
3. Replace Err(format!(...)) with Err(FrameworkError::Validation(format!(...)))
4. Remove .map_err(|e| e.to_string()) internal bridges
5. Check: task_ledger_path validates path components with validate_task_id_component which returns Result<_, FrameworkError> — remove the .map_err bridge
6. handle_step_ledger_operation likely has complex internal logic — carefully replace each Err pattern

After: cargo check -p core-state 2>&1 | tail -20
`, {phase: 'Leaf crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

// Group 4: mcp-tool-registry (tool_registry.rs) — 2 fns
const group4 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn returning Result<_, String> in core/mcp-tool-registry/src/tool_registry.rs to return Result<_, FrameworkError>.

Import path: use core_errors::FrameworkError;

Functions:
- pub fn load_tool_records(registry_path: &Path) -> Result<Vec<McpToolRecord>, String>
- pub fn load_tool_records_cached(registry_path: &Path) -> Result<Vec<McpToolRecord>, String>

For each:
1. Add "use core_errors::FrameworkError;"
2. Change return type from Result<Vec<McpToolRecord>, String> to Result<Vec<McpToolRecord>, FrameworkError>
3. Replace Err(format!(...)) with Err(FrameworkError::Validation(format!(...)))
4. Replace Err("static string".into()) with Err(FrameworkError::Validation("static string"))
5. Remove internal .map_err(|e| e.to_string()) bridges

After: cargo check -p mcp-tool-registry 2>&1 | tail -10
`, {phase: 'Leaf crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

// Group 5: eval-route — 1 fn
const group5 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn in core/eval-route/src/lib.rs returning Result<_, String> to Result<_, FrameworkError>.

Import path: use core_errors::FrameworkError;

Function:
- pub fn load_eval_cases(path: &Path) -> Result<EvalCasesPayload, String>

1. Add "use core_errors::FrameworkError;"
2. Change return type
3. Replace Err(format!(...)) with Err(FrameworkError::Validation(format!(...)))
4. Remove internal .map_err bridges

After: cargo check -p eval-route 2>&1 | tail -10
`, {phase: 'Leaf crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

phase('Mid crates')

// Group 6: skill-layer (backfill, refresh, validate, registry) — 5+ fns
const group6 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn returning Result<_, String> in core/skill-layer/src/ to return Result<_, FrameworkError>.

Import path: use core_errors::FrameworkError;

Functions (from the grep output):
core/skill-layer/src/backfill.rs:104 - pub fn backfill_registry(repo_root: &Path, dry_run: bool) -> Result<BackfillReport, String>
core/skill-layer/src/refresh.rs:33 - pub fn validate_skills(repo_root: &Path) -> Result<(), String>
core/skill-layer/src/refresh.rs:56 - pub fn refresh_skills(cmd: &SkillsCommand) -> Result<(), String>
core/skill-layer/src/refresh.rs:124 - pub fn generate_health_manifest(repo_root: &Path) -> Result<(), String>
core/skill-layer/src/validate.rs:34 - pub fn validate_skill_name(name: &str) -> Result<(), String>
core/skill-layer/src/validate.rs:62 - pub fn validate_all(repo_root: &Path) -> Result<ValidationReport, String>
core/skill-layer/src/registry.rs:76 - pub fn list_slugs(&self) -> Result<Vec<String>>

For each:
1. Add "use core_errors::FrameworkError;"
2. Change return type
3. Replace Err patterns with FrameworkError::Validation(...)
4. Remove .map_err bridges

After: cargo check -p skill-layer 2>&1 | tail -20
`, {phase: 'Mid crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

// Group 7: routing-engine (runtime_watch.rs) — 1 fn
const group7 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn in core/routing-engine/src/runtime_watch.rs returning Result<_, String> to Result<_, FrameworkError>.

Import path: use core_errors::FrameworkError;

Function:
- pub fn bootstrap(path: Option<PathBuf>) -> Result<Self, String>

1. Add "use core_errors::FrameworkError;"
2. Change return type
3. Replace Err patterns

After: cargo check -p routing-engine 2>&1 | tail -10
`, {phase: 'Mid crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

// Group 8: research-harness (proof_dag, proof_dag_serialize, subprocess) — 7 fns
const group8 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn returning Result<_, String> in core/research-harness/src/ to return Result<_, FrameworkError>.

Import path: use core_policy::error::FrameworkError; (research-harness has core-policy dep)

Functions:
core/research-harness/src/proof_dag.rs:
- pub fn decompose(&mut self, parent_id: &str, children: Vec<DagNode>, and: bool) -> Result<(), String>
- pub fn verify(&mut self) -> Result<(), String>
- pub fn backtrack(&mut self, node_id: &str) -> Result<(), String>
- pub fn validate_manual_prose_ratio(&self, max_pct: f64) -> Result<(), String>

core/research-harness/src/proof_dag_serialize.rs:
- pub fn serialize_blueprint(bp: &Blueprint) -> Result<String, String>
- pub fn deserialize_blueprint(json: &str) -> Result<Blueprint, String>
- pub fn apply_update(bp: &mut Blueprint, update: &serde_json::Value) -> Result<(), String>

core/research-harness/src/subprocess.rs:
- pub fn run_uv_module(module: &str, input: &Value) -> Result<Value, String>

For each:
1. Add "use core_policy::error::FrameworkError;"
2. Change return type
3. Replace Err patterns with FrameworkError::Validation(...)
4. Remove .map_err bridges

After: cargo check -p research-harness 2>&1 | tail -20
`, {phase: 'Mid crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

phase('High crates')

// Group 9: session-supervisor (lib, process, runtime, team_manager, worker) — ~10 fns
const group9 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn returning Result<_, String> in core/session-supervisor/src/ to return Result<_, FrameworkError>.

Import path: use core_policy::error::FrameworkError;

Functions:
core/session-supervisor/src/lib.rs:52 - pub fn handle_session_supervisor_operation(payload: Value) -> Result<Value, String>
core/session-supervisor/src/process.rs:169 - pub fn terminate_process(pid: u32) -> Result<(), String>
core/session-supervisor/src/process.rs:411 - pub fn list_running_agents(repo_root: &Path) -> Result<Vec<AgentHealthEntry>, String>
core/session-supervisor/src/runtime.rs:22 - pub fn load_store(path: &Path) -> Result<SessionSupervisorStore, String>
core/session-supervisor/src/runtime.rs:37 - pub fn save_store(path: &Path, store: &SessionSupervisorStore) -> Result<(), String>
core/session-supervisor/src/runtime.rs:112 - pub fn resolve_state_path(payload: &Value) -> Result<PathBuf, String>
core/session-supervisor/src/runtime.rs:138 - pub fn now_from_payload(payload: &Value) -> Result<String, String>
core/session-supervisor/src/runtime.rs:146 - pub fn add_seconds_rfc3339(now: &str, seconds: i64) -> Result<String, String>
core/session-supervisor/src/runtime.rs:151 - pub fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, String>
core/session-supervisor/src/team_manager.rs:162 - pub fn with_team_registry_ro<F, T>(repo_root: &Path, f: F) -> Result<T, String> where F: FnOnce(&Value) -> T
core/session-supervisor/src/team_manager.rs:47 - pub fn sanitize_path_segment(raw: &str) -> Result<String, String>
core/session-supervisor/src/team_manager.rs:53 - pub fn sanitize_segment_strict(raw: &str) -> Result<String, String>
core/session-supervisor/src/worker.rs:317 - pub fn worker_ready_for_resume(worker: &WorkerSessionRecord, now: &str) -> Result<bool, String>

For each:
1. Add "use core_policy::error::FrameworkError;"
2. Change return type
3. Replace Err patterns with FrameworkError::Validation(...) or appropriate variant
4. Remove .map_err bridges

SPECIAL: handle_session_supervisor_operation at lib.rs:52 is the entry point called via framework-runtime-hooks. Its caller uses .map_err(|e| e.to_string()) which serves as a shim. The fn pointer type in framework-runtime-hooks still uses String, and that's fine — we KEEP the shim at the hook boundary.

After: cargo check -p session-supervisor 2>&1 | tail -30
`, {phase: 'High crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

// Group 10: fr-exec (live_execute, trace_attach, trace_transport) — ~9 fns
const group10 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn returning Result<_, String> in core/fr-exec/src/ to return Result<_, FrameworkError>.

Import path: use core_policy::error::FrameworkError; (fr-exec has core-policy dep)

Functions:
core/fr-exec/src/live_execute.rs:
- pub fn live_execute_http_client() -> Result<&'static reqwest::blocking::Client, String>
- pub fn validate_live_execute_aggregator_base_url(base_url: &str) -> Result<(), String>
- pub fn extract_chat_completion_content(payload: &Value) -> Result<String, String>

core/fr-exec/src/trace_attach.rs:
- pub fn attach_runtime_event_transport(payload: Value) -> Result<Value, String>
- pub fn subscribe_attached_runtime_events(payload: Value) -> Result<Value, String>
- pub fn cleanup_attached_runtime_event_transport(payload: Value) -> Result<Value, String>

core/fr-exec/src/trace_transport.rs:
- pub fn build_trace_transport_descriptor(payload: Value) -> Result<Value, String>
- pub fn build_trace_handoff_descriptor(payload: Value) -> Result<Value, String>
- pub fn build_checkpoint_resume_manifest(payload: Value) -> Result<Value, String>
- pub fn write_transport_binding_payload(payload: Value) -> Result<Value, String>
- pub fn write_checkpoint_resume_manifest_payload(payload: Value) -> Result<Value, String>
- pub fn write_text_payload(path: &Path, payload: &str) -> Result<usize, String>

For each:
1. Read the file to understand internal error handling
2. Change return type
3. Replace Err patterns
4. Remove .map_err bridges

NOTE: evolution_observer.rs:276 already uses FrameworkError — don't touch it.

After: cargo check -p fr-exec 2>&1 | tail -30
`, {phase: 'High crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

// Group 11: framework-extra (12+ fns across many files)
const group11 = await agent(`
面向用户的可见输出使用简体中文。
Migrate pub fn returning Result<_, String> in core/framework-extra/src/ to return Result<_, FrameworkError>.

Import path: use core_policy::error::FrameworkError; (framework-extra has core-policy dep)

Functions:
core/framework-extra/src/closeout.rs:28 - pub fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> Result<PathBuf, String>
core/framework-extra/src/content_store.rs:56 - impl ContentStore: pub fn put(&self, content: &str) -> Result<String, String>
core/framework-extra/src/content_store.rs:107 - impl ContentStore: pub fn remove_stale(&self, max_age: Duration) -> Result<usize, String>
core/framework-extra/src/contract_summary.rs:26 - pub fn build_framework_contract_summary_envelope(repo_root: &Path) -> Result<Value, String>
core/framework-extra/src/framework_doctor.rs:25 - pub fn run_framework_doctor(repo_root: &Path) -> Result<DoctorResult, String>
core/framework-extra/src/framework_doctor.rs:387 - pub fn run_continuity_audit(repo_root: &Path) -> Result<Value, String>
core/framework-extra/src/framework_doctor.rs:648 - pub fn report_broken_symlinks(repo_root: &Path) -> Result<usize, String>
core/framework-extra/src/orchestration_controller.rs:1052 - pub fn build_runtime_metric_record(payload: Value) -> Result<Value, String>
core/framework-extra/src/prompt_resolver.rs:20 - pub fn resolve_one(&self, raw_hash: &str) -> Result<String, String>
core/framework-extra/src/session_artifacts.rs:38 - pub fn write_framework_session_artifacts(payload: Value) -> Result<Value, String>
core/framework-extra/src/session_call.rs:191 - pub fn init_tracker(repo_root: &Path) -> Result<(), String>
core/framework-extra/src/session_call.rs:234 - pub fn check_anomalies(repo_root: &Path) -> Result<Vec<String>, String>
core/framework-extra/src/session_call.rs:324 - pub fn read_tracker_state(repo_root: &Path) -> Result<Value, String>
core/framework-extra/src/statusline.rs:10 - pub fn build_framework_statusline(repo_root: &Path) -> Result<String, String>

For each:
1. Read the file
2. Add import
3. Change return type
4. Replace Err patterns with FrameworkError::Validation(...) or FrameworkError::Io(...) etc.
5. Remove .map_err bridges where the inner error is FrameworkError
6. If a function already has FrameworkError internal usage, it's easier — just remove the .map_err bridge

SPECIAL: session_artifacts.rs already had bridges removed in the prior session — check what's left.
session_call.rs init_tracker and check_anomalies may call migrated APIs from core-state etc.

After: cargo check -p framework-extra 2>&1 | tail -20
`, {phase: 'High crates', schema: {type: 'object', properties: {status: {type: 'string'}}}})

phase('Verify')

// Group 12: infrastructure crates and verify
const group12 = await agent(`
面向用户的可见输出使用简体中文。
This is the FINAL verification phase. Run: cargo check --workspace 2>&1

1. If 0 errors, report success
2. If there are errors, analyze them and fix ALL of them. The most common issues will be:
   a. Functions in runtime-core, runtime-infra, loop-engine, framework-maint that return Result<_, String> and call migrated functions
   b. The .map_err(|e| e.to_string()) bridges need updating
   c. framework-runtime-hooks fn pointer types still return Result<_, String> — these are plugin boundaries, keep the shims at call sites

3. Specifically check and fix:
   - core/runtime-core/src/task_command.rs (3 fns)
   - core/runtime-infra/src/telemetry_emit.rs (2 fns)
   - core/loop-engine/src/closeout.rs verify_rfv_convergence
   - core/loop-engine/src/env_flags.rs subagent_binary
   - core/framework-maint/src/maint.rs dispatch
   - framework-runtime-hooks fn pointer types — these are touchy since they're function pointers.

4. For framework-runtime-hooks: the fn pointer types use Result<Value, String>. These are PI boundaries through OnceLock. For these, ADD a dep on core-errors and change the fn pointer types. But since they're fn() pointers, the registrants need to match. The issue is: runtime-core registers functions that now return FrameworkError but the hook expects String.

   SOLUTION for framework-runtime-hooks: Change the fn pointer types to use FrameworkError, then update the registration sites in runtime-core and runtime-infra.

5. After ALL fixes, run cargo check --workspace again. If 0 errors, done. If errors remain, fix and loop.

Be thorough. Every error must be fixed. Do NOT leave any compilation errors.
`, {phase: 'Verify', schema: {type: 'object', properties: {status: {type: 'string'}, errors: {type: 'integer'}}}})

return {
  groups: [
    {crate: 'fr-utils', status: group1.status, files: group1.fileCount},
    {crate: 'core-policy', status: group2.status},
    {crate: 'core-state', status: group3.status},
    {crate: 'mcp-tool-registry', status: group4.status},
    {crate: 'eval-route', status: group5.status},
    {crate: 'skill-layer', status: group6.status},
    {crate: 'routing-engine', status: group7.status},
    {crate: 'research-harness', status: group8.status},
    {crate: 'session-supervisor', status: group9.status},
    {crate: 'fr-exec', status: group10.status},
    {crate: 'framework-extra', status: group11.status},
    {crate: 'verify', status: group12.status, finalErrors: group12.errors},
  ]
}
