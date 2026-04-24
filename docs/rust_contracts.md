# Runtime Rust Contracts

## Purpose

This document freezes the first-pass contracts for the Rust migration of the
local runtime.

It is the runtime-specific contract source of truth for:

- `aionrs_fusion_docs/codex_dual_entry_rust_checklist.md`
- `aionrs_fusion_docs/codex_dual_entry_next_phase_checklist.md`
- `framework_runtime`
- future Rust crates that replace Python hot paths
- `docs/runtime_observability_contract.md`
- `docs/runtime_sandbox_contract.md`
- `docs/runtime_compaction_contract.md`

It must stay compatible with existing project artifacts and must not fork the
meaning of existing routing and runtime files.
The `aionrs_fusion_docs/*` references above are compatibility-facing narrative
artifacts only; they may explain migration state, but they must not redefine
runtime truth or re-promote `aionrs` / `AionUI` / `upgrade_compatibility_matrix`
as steady-state authority.

## Current Codex Dual-Entry Boundary

For the current phase, Rust owns the default live runtime path as well as the
contract / artifact / parity lane.

The `router-rs --profile-json --framework-profile <path>` output now carries:

- `companion_projection` for legacy compatibility only
- `codex_common_adapter` for shared Codex contract projection
- `codex_desktop_adapter` as the canonical interactive desktop contract
- `codex_cli_adapter` for the headless CLI contract
- `codex_dual_entry_parity_snapshot` as the primary Desktop / CLI shared-contract
  parity artifact
- `codex_desktop_alias_retirement_status` as the Rust-side alias exit contract
- optional `compatibility_lane` for explicit continuity-only payloads

The legacy alias payload is no longer serialized into `--profile-json` by
default; continuity consumers must opt in explicitly with
`--include-legacy-alias-artifact`. When that opt-in is enabled for
`--profile-json`, the alias stays quarantined under
`compatibility_lane.codex_desktop_host_adapter` instead of reappearing as a
top-level peer field.

The `router-rs --profile-artifacts-json --framework-profile <path>` output
mirrors the first-class Codex artifact set directly, so downstream consumers no
longer need to unpack the bundle just to read parity or alias-retirement
contracts. `prepare_session(...)` and dry-run preview already route through
`router-rs`, and normal live execution now stays Rust-only by default. The
former compatibility fallback is now retired; explicit fallback requests are
rejected instead of reopening the Python live path.

That first-class set now also includes
`cli_family_capability_discovery` plus
`execution_controller_contract` plus
`delegation_contract` plus
`supervisor_state_contract` plus
`execution_kernel_delegate_family` plus
`execution_kernel_delegate_impl` plus
`execution_kernel_live_response_serialization_contract`, so Rust can publish a
stable discovery contract for `codex cli` / `claude cli` / `gemini cli`
support, freeze the shared execution-controller / delegation / supervisor-state
control-plane artifacts, and freeze the live / dry-run execution-kernel shape
without keeping any Python-owned steady-state blocker list alive.

`emit_framework_contract_artifacts(...)` now calls `router-rs` directly for the
first-class contract set. The old `rust_python_artifact_parity_report.json`
lane is retired: there is no second Python artifact truth to compare against.
If a contract drifts, fix the Rust compiler or the thin Python call boundary.

Rust may mirror Claude-specific hook-compatible host projection metadata in
`claude_code_adapter`, but that mirror remains host-private contract data. It
does not mean Rust owns Claude hook execution semantics, hook policy resolution,
or host config materialization.

This output is allowed to mirror framework truth and parity artifacts.
It is not allowed to turn `codexcli` into framework control truth or to rewrite
runtime-kernel semantics.

Rust must therefore follow these dual-entry invariants:

- `codex_desktop_adapter` is the canonical desktop identity for parity, bundle
  emission, and future extracted artifacts.
- `codex_desktop_host_adapter` is mirror-only compatibility surface inside the
  explicit compatibility lane; it must not gain standalone schema drift,
  host-only semantics, or controller meaning.
- `compatibility_lane` is the only allowed `--profile-json` surface for
  continuity-only alias payloads; first-class bundle peers stay canonical-only.
- explicit `--profile-artifacts-json --include-legacy-alias-artifact` output is
  allowed only as a continuity transport exception; it does not re-promote the
  alias into the canonical Rust peer set.
- `upgrade_compatibility_matrix` may survive only as a secondary compatibility
  inventory / smoke artifact; it must not replace
  `codex_dual_entry_parity_snapshot` or any first-class Rust parity contract as
  the primary regression baseline.
- `aionrs` / `AionUI` compatibility stays in upstream-safe legacy lanes, not in
  the forward Rust runtime center.
- `codexcli` remains a headless execution entrypoint, never framework truth.

## External Benchmark: DeerFlow 2.0

The current external benchmark for the next runtime wave is DeerFlow 2.0 on
`bytedance/deer-flow` `main`, not a separate `DeerFlow2` repository.

What is worth borrowing:

- harness/app split instead of letting transport adapters own runtime truth
- explicit run-manager semantics for `reject` / `interrupt` style conflicts
- embedded runtime migration pattern instead of permanent thin-proxy execution
- resumable stream bridge instead of ad-hoc polling only
- unified store/checkpointer backend seams

What is not worth copying directly:

- reflection-heavy provider wiring as the long-term kernel center
- process-global config assumptions
- LangGraph-shaped runtime details as if they were the final Rust kernel target

The benchmark implication for this repo is:

- Rust should keep moving toward authoritative routing/compiler ownership
- runtime kernel work should deepen around owned state/control-plane seams
- avoid a big-bang rewrite; move one kernel slice at a time under frozen
  contracts

## Current Status Ledger

### 已实现

The active runtime wave is now Rust-authoritative across the default runtime:

- `route_engine_mode` defaults to `rust`, so routing authority is Rust; the old
  `python` route engine is retired, and only diagnostic `shadow` / `verify`
  modes remain
- live execution and dry-run preview stay Rust-only by default, and
  compatibility live fallback is retired with explicit requests rejected
- the runtime control plane now publishes a Rust-owned authority descriptor for
  `router` / `state` / `trace` / `memory` / `background`
- the `framework_runtime/` Python package is retired; framework snapshot,
  contract summary, memory recall, session artifact writing, and framework MCP
  now use `router-rs::framework_runtime` surfaces directly
- framework truth stays unchanged; the migration closes default-Python
  authority, not by forking artifacts or host semantics

The implemented runtime control-plane surface in this wave is:

- background runs now support explicit `multitask_strategy` semantics
  (`reject` / `interrupt`) instead of only implicit duplicate-session failure
- runtime now exposes a Rust-owned `session-supervisor` control plane for
  external Claude / Codex workers, including durable worker records,
  tmux session identity, normalized `blocked_rate_limit` state, and
  resume scheduling without any `.omc/**` dependency
- runtime traces now emit resumable `seq` / `cursor` metadata and expose a
  replay window seam from the JSONL event stream
- runtime now exposes an in-memory `RuntimeEventBridge` for
  `subscribe/resume/heartbeat/cleanup`, so host-facing consumers have a real
  control-plane seam instead of cursor metadata only
- runtime now exposes a versioned `describe_runtime_event_transport(...)`
  descriptor for the local polling bridge, so hosts can discover transport
  kind, schema versions, and latest resumable cursor without inventing a
  second stream contract
- runtime now also exposes `describe_runtime_event_handoff(...)`, which joins
  that transport descriptor with replay/checkpoint anchors for cross-process
  or remote attach without pretending SSE/network delivery already exists
- `trace_resume_manifest_path` now remains only as checkpoint/recovery metadata;
  attach consumers should treat the binding artifact as the primary attach
  descriptor and `describe_runtime_event_handoff(...)` as the recommended
  remote attach seam
- process-external attach now also works when the active checkpoint backend is
  SQLite: if the binding/resume/trace paths are logical storage paths instead
  of materialized JSON files, the attach bridge resolves them through the
  runtime SQLite backing store instead of silently degrading to filesystem-only
- that transport seam is now explicitly host-facing and remote-capable in
  contract shape, while still honestly implemented as a local poll bridge:
  `transport_family=host-facing-bridge`, `endpoint_kind=runtime_method`, and
  cleanup only drops bridge cache while preserving replay-based reseed
- the same checkpointer family now persists a JSON transport binding artifact,
  and resume manifests carry the current transport binding path as a recovery
  anchor
- runtime now exposes a filesystem `RuntimeCheckpointer` seam so
  `TRACE_RESUME_MANIFEST.json`, trace paths, and background-state path
  discovery converge on one runtime-owned backend interface
- the checkpointer and durable background state now share the same
  backend-family abstraction; filesystem remains the default concrete backend,
  while compaction is a gated minimal lane on supported backends and unsupported
  backends fail closed
- runtime execution now enters through a single `ExecutionKernel` seam owned
  by `ExecutionEnvironmentService`, rather than keeping dry/live branching and
  run-output normalization inline in `runtime.py`
- live execution now enters a Rust-owned out-of-process slice through
  `router-rs --execute-json`, so the actual live model invocation and response
  normalization are no longer owned by Python
- `prepare_session(...)` and dry-run preview already route through
  `router-rs`, so Python no longer owns the default preview path
- the execution-kernel contract now treats normal live operation as
  Rust-only steady state; explicit compatibility fallback requests are rejected
  instead of reopening the Python live path
- the next safe slice is now also externalized as
  `execution_kernel_live_response_serialization_contract`, which freezes the
  current `RunTaskResponse` shape plus the response metadata invariants for
  live primary, deterministic dry-run, and the retired
  compatibility-fallback metadata lane
- `execution_kernel_delegate_family` and
  `execution_kernel_delegate_impl` are now also part of the stable
  execution-kernel contract descriptor, so callers can read delegate
  family/impl directly from the shared contract lane without reopening a live
  Python fallback branch
- the runtime control plane now also emits an explicit
  `kernel_metadata_bridge`, so steady-state execution-kernel metadata field
  names and owner markers are projected from Rust instead of being reauthored
  in Python
- CLI-family capability discovery now also reports supervisor-driver ownership,
  resume examples, rate-limit auto-resume support, and framework-native
  `autopilot` / `deepinterview` alias entrypoints for Codex and Claude
- OMC retirement contract now also freezes that `autopilot` and
  `deepinterview` inherit the original OMC core capability bar while this repo
  enforces a stricter implementation bar for root-cause discovery, verification
  evidence, recovery, and bounded convergence
- the contract no longer carries a blocker list; compatibility-only metadata
  is isolated to retirement descriptors and does not drive runtime branching
- compatibility fallback now survives only as a retired contract surface:
  the old `rust_execute_fallback_to_python` request surface has been removed,
  and normal live/dry-run execution no longer re-enter the Python kernel path
- compatibility-fallback metadata is tracked only as retired legacy fields, not
  as a runnable steady-state capability on the live kernel
- interrupt-style background replacements now use a reserved session takeover
  handoff before the new job queues, reducing the old release-then-requeue race
- pending takeover reservations are now persisted as part of the durable
  background-state contract so restart recovery does not silently drop the
  replacement intent
- background queue admission now checks an explicit admitted-job count instead
  of peeking into `Semaphore._value`
- runtime health now exposes a Rust-owned control-plane descriptor, so default
  routing and control-plane authority can be verified without inferring from any
  Python projection layer

One additional Rust-authority slice is now implemented in this wave:

- the old `scripts/route.py` Python shim is retired; route/search CLI calls go
  directly to `router-rs`, so the non-runtime CLI surface no longer carries a
  second Python route authority
- `router-rs` now also emits a stable route-policy payload for
  `shadow/verify/rust`, so route mode / primary-authority decisions stay in
  Rust
- `router-rs` route JSON now carries an explicit authority marker and decision
  schema version, and consumers must validate those fields before trusting the
  result
- `router-rs` now also owns the stable `RouteDiffReport` compare path for
  `shadow` / `verify` / rollback semantics, so Python no longer computes the
  Rust-side mismatch vocabulary locally
- typed
  `route_contract / route_policy_contract / route_report_contract / route_snapshot_contract`
  entrypoints are now Rust-owned contract surfaces instead of ad hoc raw JSON
  helper lanes
- an unknown Rust-selected skill must fail closed in consumers instead of
  silently drifting into host-side fallback interpretation
- typed route-contract consumption now also reaches fixture/live parity
  regressions, and the adapter supports explicit `runtime_path` /
  `manifest_path` overrides so fixture-backed Rust parity no longer needs to
  route around the adapter through raw transport helpers
- the browser-mcp host consumer now replays runtime events through the Rust
  attach descriptor / handoff / binding-artifact contract instead of treating
  `binding/handoff/resume` as a hand-assembled second attach protocol
- runtime observability is no longer only vocabulary-ready; the repo now ships
  concrete Rust-first exporter-descriptor / metric-record / dashboard helpers,
  and the same contract is projected through runtime health

### 已退休

- the compatibility live fallback runtime path is retired; explicit requests
  are rejected instead of reopening the Python live path
- the old blocker-list narration is retired from the live kernel contract and
  survives only in retirement descriptors
- `codex_desktop_host_adapter` remains compatibility-only mirror surface and is
  no longer treated as a default or canonical peer

### 下一 safe slice

- keep route-side compatibility helpers retired, so adjacent evaluation/test
  helpers cannot reintroduce privileged raw-JSON consumers
- keep pushing execution-kernel metadata / naming bridge canonicalization
  toward Rust without reopening a Python live authority path
- keep the native install/bootstrap lane closed: a fresh machine should keep
  landing on the current Rust-first default, and installer/status regressions
  should fail if the bootstrap payload drifts away from that contract
- keep lane-4 closure intact: browser-mcp stays on the Rust attach descriptor
  contract, while fallback / continuity host consumers remain explicit opt-in
  lanes instead of drifting back into the default outward surface
- only reopen integrator/regenerate when a later contract change actually
  alters generated outputs or docs, instead of refreshing global outputs just
  because the installer lane was re-verified

## Contract 0: Desktop Alias Retirement Path

Purpose:

- define the Rust-side exit path for `codex_desktop_host_adapter`

Required invariants:

- while the alias is emitted, its payload semantics must mirror
  `codex_desktop_adapter`; allowed differences are limited to adapter identity,
  compatibility annotations, or explicit legacy alias metadata
- no Rust-owned artifact may treat `codex_desktop_host_adapter` as the
  canonical desktop key once `codex_desktop_adapter` exists
- retirement of the alias must not re-open an `aionrs` / `AionUI` first
  narrative and must not make `codexcli` the control-plane fallback

Exit criteria:

- downstream callers and docs have migrated to `codex_desktop_adapter`
- `codex_dual_entry_parity_snapshot` stays green without alias-specific
  semantics
- continuity artifacts record the remaining risk, rollback point, and any
  edge-local translation shim before the alias is removed from default bundle
  emission

## Contract Rules

- Contract changes must be explicit and versioned.
- Rust may replace implementations, not silently redefine semantics.
- Existing artifact names remain canonical unless a migration plan is approved.
- Missing implementations may be deferred, but their boundary must still be
  written down here.

## Contract 1: Route Result

Purpose:

- represent the final selected route for one task

Compatibility targets:

- `router-rs --route-json`
- `router-rs --route-policy-json`
- `router-rs --route-report-json`

Required fields:

- `task: string`
- `session_id: string`
- `selected_skill: string`
- `overlay_skill: string | null`
- `layer: string`
- `score: float`
- `reasons: string[]`
- `prompt_preview: string | null`
- `route_engine: rust`
- `diagnostic_route_mode: none | shadow | verify`
- `route_diagnostic_report: route diagnostic report | null`

Invariants:

- exactly one primary owner
- at most one overlay
- `layer` must match the selected skill layer
- `overlay_skill` must never equal `selected_skill`
- if no viable hit exists, fallback still returns a valid primary owner

Rust migration note:

- Rust owns scoring, route picking, route-mode policy, and canonical route
  snapshot shaping through versioned route, snapshot, and diagnostic-report
  contracts
- Python no longer owns a primary route-result lane, rollback lane, or parity
  diff vocabulary in the runtime contract

## Contract 1A: Route Policy

Purpose:

- define the stable Rust-only route-mode policy shared by route consumers,
  health reporting, and shadow/verify selection

Compatibility targets:

- `router-rs --route-policy-json`
- `router-rs --runtime-control-plane-json`

Required fields:

- `mode: verify | shadow | rust`
- `diagnostic_route_mode: none | shadow | verify`
- `primary_authority: rust`
- `route_result_engine: rust`
- `diagnostic_report_required: boolean`
- `strict_verification_required: boolean`

Invariants:

- `primary_authority` and `route_result_engine` must stay `rust`
- `mode=rust` must set `diagnostic_route_mode=none`
- `mode=shadow` must require a diagnostic report and disable strict verification
- `mode=verify` must require a diagnostic report and enable strict verification

## Contract 1B: Route Diagnostic Report

Purpose:

- define the stable Rust-owned diagnostic vocabulary shared by runtime traces,
  response metadata, and future soak dashboards

Compatibility targets:

- `router-rs --route-report-json`
- `PrepareSessionResponse.route_diagnostic_report`
- `RoutingResult.route_diagnostic_report`
- `route.selected` trace payload

Required fields:

- `mode: verify | shadow`
- `primary_engine: rust`
- `evidence_kind: rust-owned-snapshot`
- `strict_verification: boolean`
- `verification_passed: boolean`
- `route_snapshot.engine: rust`
- `route_snapshot.selected_skill: string`
- `route_snapshot.overlay_skill: string | null`
- `route_snapshot.layer: string`
- `route_snapshot.score_bucket: string`
- `route_snapshot.reasons_class: string`

Invariants:

- `primary_engine` must stay `rust`
- `evidence_kind` must stay `rust-owned-snapshot`
- `mode=shadow` must set `strict_verification=false`
- `mode=verify` must set `strict_verification=true`
- `verification_passed` reports whether the Rust-only verification contract
  stayed aligned with the emitted live route
- no parallel `shadow_route_report` artifact name remains supported; the
  payload lives in canonical trace / response metadata only

## Contract 2: Middleware Context

Purpose:

- define the mutable runtime context that flows through middleware phases

Compatibility targets:

- `MiddlewareContext`
- `MiddlewareChain.execute()`

Required fields:

- `task: string`
- `session_id: string`
- `user_id: string`
- `routing_result: route result`
- `prompt: string`
- `memory_facts: string[]`
- `active_subagent_count: integer`
- `metadata: object`

Invariants:

- `routing_result` is immutable in meaning even if wrapped in a mutable context
- `prompt` may be rewritten by middleware
- `metadata` may accumulate execution details but must stay JSON-serializable

Rust migration note:

- Rust may own the context data model and middleware execution engine later
- prompt-policy text assembly remains Python-owned only where explicitly called
  out by the host runtime; it must not become a second profile/shared-contract
  compiler
- Rust-owned live execution ignores caller-supplied `prompt` /
  `prompt_preview` fields at the kernel boundary and shapes the live execute
  prompt internally instead

## Contract 3: Background Job State

Purpose:

- define durable lifecycle states for queued and running work

Compatibility targets:

- `BackgroundRunStatus`
- `enqueue_background_run()`
- `get_background_status()`
- `_run_background_job()`

Required fields:

- `job_id: string`
- `session_id: string | null`
- `status: queued | running | interrupt_requested | interrupted | retry_scheduled | retry_claimed | retry_exhausted | completed | failed`
- `result_ref: object | null`
- `error: string | null`
- `created_at: timestamp`
- `updated_at: timestamp`

Required control fields:

- `attempt: integer`
- `retry_count: integer`
- `max_attempts: integer`
- `timeout_seconds: integer | null`
- `claimed_by: string | null`
- `backoff_base_seconds: number`
- `backoff_multiplier: number`
- `max_backoff_seconds: number | null`

Required lifecycle timestamps:

- `claimed_at: timestamp | null`
- `next_retry_at: timestamp | null`
- `retry_scheduled_at: timestamp | null`
- `retry_claimed_at: timestamp | null`
- `interrupt_requested_at: timestamp | null`
- `interrupted_at: timestamp | null`
- `last_attempt_started_at: timestamp | null`
- `last_attempt_finished_at: timestamp | null`
- `last_failure_at: timestamp | null`

Invariants:

- state transitions must be monotonic and valid
- one session must not be actively claimed by multiple jobs at once
- retry/backoff policy must be explicit in durable state, not inferred only
  from trace history
- `interrupt_requested` must always resolve to `interrupted` or a terminal
  failure state
- `completed`, `failed`, `interrupted`, and `retry_exhausted` are terminal

Rust migration note:

- this is a priority Rust target

## Contract 4: Trace Event

Purpose:

- record reconstructable runtime events with stable schema

Compatibility targets:

- `TRACE_METADATA.json`
- response metadata from live runs
- future JSONL or OpenTelemetry export

Required fields:

- `event_id: string`
- `ts: timestamp`
- `session_id: string`
- `job_id: string | null`
- `kind: string`
- `stage: string`
- `payload: object`
- `schema_version: string`

Suggested kinds:

- `session.prepared`
- `route.selected`
- `middleware.enter`
- `middleware.exit`
- `run.started`
- `run.completed`
- `run.failed`
- `job.queued`
- `job.claimed`
- `job.interrupt_requested`
- `job.interrupted`
- `job.retry_scheduled`
- `job.retry_claimed`
- `job.retry_exhausted`
- `job.completed`
- `job.failed`

Invariants:

- every event is append-only
- payload must be JSON-serializable
- schema version must be explicit

Rust migration note:

- Rust owns event typing and the framework/control-plane writer surfaces
- OpenTelemetry-ready shared dimensions, metrics catalog, and JSONL vocabulary
  mapping are defined in `docs/runtime_observability_contract.md`

## Contract 5: Artifact Compatibility

Purpose:

- prevent Rust migration from creating duplicate runtime artifacts

Canonical artifacts:

- `SESSION_SUMMARY.md`
- `NEXT_ACTIONS.json`
- `EVIDENCE_INDEX.json`
- `TRACE_METADATA.json`
- `.supervisor_state.json`
- `execution_controller_contract.json`
- `delegation_contract.json`
- `supervisor_state_contract.json`

Invariants:

- these remain the primary continuity artifacts
- Rust code may validate, produce, or update them
- Rust code must not invent shadow replacements for the same meaning

## Contract 6: Memory Service Boundary

Status:

- frozen for the current lightweight filesystem-backed implementation
- deterministic kernel is explicitly scoped so Rust can replace mechanics
  without taking over Python policy

Compatibility targets:

- `load_facts(user_id) -> string[]`
- `save_facts(user_id, facts) -> void`
- `extract_facts_sync(conversation) -> string[]`
- `retrieve_facts(user_id, limit?) -> memory rows[]`
- `contract_snapshot(user_id) -> memory contract object`

Required storage semantics:

- storage path is deterministic from `user_id`
- persisted payload keeps insertion order
- persisted schema version remains explicit

Required retrieval semantics:

- retrieval order is stable insertion order
- `rank` is `1`-based retrieval order
- `limit` truncates after ranking, not before dedupe

Required dedupe semantics:

- dedupe is case-insensitive
- the first surviving insertion wins
- empty strings and whitespace-only rows are discarded

Required provenance semantics:

- every retrieved row exposes provenance
- provenance currently includes:
  - `kind = filesystem.user-facts.v1`
  - `storage_path = absolute path to the user fact file`

Rust-ready deterministic kernel:

- compile the contract-provided regex patterns
- normalize extracted fact values with stable whitespace collapse
- dedupe facts case-insensitively while preserving first surviving insertion
- build retrieval rows with stable `rank` and provenance
- apply `limit` only after the ranked row set exists

Python-owned policy boundary:

- which extraction patterns should be active for a given host/runtime contract
- whether an LLM-derived extraction policy should augment regex extraction later
- any future prompt wording or summarization policy layered above fact storage

Rule:

- model-based memory extraction policy stays Python-owned
- Rust may replace the storage/extraction kernel only if these semantics stay stable

## Contract 7: Compression Boundary

Status:

- frozen for deterministic pruning mechanics; prompt wording policy remains Python-owned

Known boundary:

- input: prompt text plus token budget
- output: compressed prompt text plus deterministic compression metadata

Deterministic mechanics suitable for Rust later:

- token estimation
- section ranking
- truncation
- artifact offload decision

Current Rust-ready kernel line:

- deterministic compression mechanics live in `ContextEngineer.compress_contract(...)`
- Rust may replace those mechanics only if it preserves the same byte-level
  contract for retained sections, omission markers, and truncation markers

Required compression metadata:

- `schema_version`
- `input_token_estimate`
- `output_token_estimate`
- `omitted_sections`
- `strategy`
- `truncated`
- `artifact_offload_decision`

Required mechanics:

- if the prompt already fits, output must be byte-identical
- structured prompts prefer `head[3] + notice + tail[2]` before raw truncation
- `artifact_offload_decision` is explicit even when the value is currently always `false`
- zero-token budgets must return a deterministic omission marker
- truncation appends a deterministic tail-truncation marker

Policy that stays Python-owned for now:

- summarization prompt design
- section wording
- human-readable compression guidance
- any future model-assisted rewrite that changes semantic content instead of
  deterministic pruning only

## Contract 8: Sandbox Boundary

Status:

- frozen as a contract with a contract-backed minimal implementation already
  live in the host runtime

Source of truth:

- `docs/runtime_sandbox_contract.md`

Required boundary:

- lifecycle states
- capability policy
- budget enforcement semantics
- async cleanup semantics
- failure isolation and recoverability rules

Rule:

- runtime implementation may evolve, but it must stay compatible with the
  machine-readable sandbox contract before execution semantics move into Rust

## Contract 9: Compaction Boundary

Status:

- frozen as a contract-backed minimal implementation that is already live on
  supported backends

Source of truth:

- `docs/runtime_compaction_contract.md`

Required boundary:

- snapshot schema
- delta replay contract
- generation rollover policy
- artifact ref strategy
- consistency invariants

Rule:

- future `trace-core-rs` or runtime compaction work may change storage shape,
  but must preserve the compacted replay semantics defined in the compaction
  contract

## Immediate Execution Checklist

- [x] Keep route result semantics stable while moving scoring to Rust.
- [x] Add shadow-route diff vocabulary and single-switch Rust rollback control.
- [x] Replace in-memory background bookkeeping with a durable state contract.
- [x] Introduce typed trace events before broader runtime refactors.
- [x] Freeze explicit interrupt / retry / backoff semantics in the runtime contract.
- [x] Freeze deterministic memory/compression contracts needed for later Rust cores.
- [x] Freeze observability / sandbox / compaction contracts before implementation migration.

## Non-Goals

- defining a new routing theory
- replacing `owner / gate / overlay`
- inventing shadow artifacts
- embedding prompt-heavy policy logic in Rust first
