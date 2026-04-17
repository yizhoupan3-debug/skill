# Runtime Rust Contracts

## Purpose

This document freezes the first-pass contracts for the Rust migration of the
local runtime.

It is the runtime-specific contract source of truth for:

- `aionrs_fusion_docs/codex_dual_entry_rust_checklist.md`
- `aionrs_fusion_docs/codex_dual_entry_next_phase_checklist.md`
- `codex_agno_runtime`
- future Rust crates that replace Python hot paths
- `docs/runtime_observability_contract.md`
- `docs/runtime_sandbox_contract.md`
- `docs/runtime_compaction_contract.md`

It must stay compatible with existing project artifacts and must not fork the
meaning of existing routing and runtime files.

## Current Codex Dual-Entry Boundary

For the current phase, Rust stays in the contract / artifact / parity lane.

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
`--profile-json`, the alias is quarantined under
`compatibility_lane.codex_desktop_host_adapter` instead of reappearing as a
top-level peer field.

The `router-rs --profile-artifacts-json --framework-profile <path>` output
mirrors the first-class Codex artifact set directly, so downstream consumers no
longer need to unpack the bundle just to read parity or alias-retirement
contracts. Once the alias retirement gate is green, this artifact mode no
longer emits `codex_desktop_host_adapter` by default; compatibility consumers
must opt in explicitly with `--include-legacy-alias-artifact`, and should treat
that alias payload as a continuity-only top-level transport artifact rather
than a peer contract surface or first-class Rust output.

That first-class set now also includes
`cli_family_capability_discovery` plus
`execution_controller_contract` plus
`delegation_contract` plus
`supervisor_state_contract` plus
`execution_kernel_live_fallback_retirement_status` plus
`execution_kernel_live_response_serialization_contract`, so Rust can publish a
stable discovery contract for `codex cli` / `claude cli` / `gemini cli`
support, freeze the shared execution-controller / delegation / supervisor-state
control-plane artifacts, and expose a host-neutral fallback-retirement status
artifact without turning any one CLI host into framework truth.

When Python emits the same first-class contract set through
`emit_framework_contract_artifacts(...)`, it now also writes
`rust_python_artifact_parity_report.json`. That report treats the first-class
Codex artifacts as the parity target, and `router-rs` now compiles
`codex_desktop_alias_retirement_status.inventory_summary` directly from the
same repo-side alias scan lane instead of leaving that summary Python-owned.

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
- `codex_desktop_host_adapter` is mirror-only compatibility surface; it must
  not gain standalone schema drift, host-only semantics, or controller meaning.
- `compatibility_lane` is the only allowed `--profile-json` surface for
  continuity-only alias payloads; first-class bundle peers stay canonical-only.
- explicit `--profile-artifacts-json --include-legacy-alias-artifact` output is
  allowed only as a continuity transport exception; it does not re-promote the
  alias into the canonical Rust peer set.
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

## Current Implementation Wave

The active runtime wave is no longer just alias quarantine.

The current priorities are:

- keep Rust authoritative on route/parity/compiler surfaces
- deepen runtime control-plane behavior against the frozen contracts for:
  - route diff/reporting
  - durable background job state
  - typed trace/runtime telemetry
  - sandbox lifecycle
  - compaction/replay
- keep framework truth unchanged while tightening runtime semantics

One runtime control-plane slice is already implemented in this wave:

- background runs now support explicit `multitask_strategy` semantics
  (`reject` / `interrupt`) instead of only implicit duplicate-session failure
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
- the checkpointer and durable background state now already share the same
  backend-family abstraction, even though the only concrete implementation is
  still filesystem-backed and compaction/snapshot-delta are not live yet
- runtime execution now enters through a single `ExecutionKernel` seam owned
  by `ExecutionEnvironmentService`, rather than keeping dry/live branching and
  run-output normalization inline in `runtime.py`
- live execution now enters a Rust-owned out-of-process slice through
  `router-rs --execute-json`, so the actual live model invocation and response
  normalization are no longer owned by Python
- that live execute handoff no longer depends on a Python-provided
  `prompt_preview`; Python still computes prompt text for session previews,
  deterministic dry-run payloads, and the optional compatibility fallback, but
  Rust now shapes the live request prompt internally once the execution handoff
  begins, and direct callers may not override that live prompt authority with a
  caller-supplied `prompt_preview`
- the runtime `run_task(...)` live path also no longer eagerly builds the
  Python preview / middleware prompt chain before handoff; public
  `prepare_session(...)` still exposes preview text, but normal live execution
  now leaves `ctx.prompt` empty unless the request is dry-run
- the execution-kernel contract now treats normal live operation as
  `execution_kernel_contract_mode=rust-live-primary`; Python remains on the
  seam for deterministic dry-run support plus an explicit compatibility
  fallback controlled by `CODEX_AGNO_RUST_EXECUTE_FALLBACK_TO_PYTHON`
- that fallback retirement state is now also externalized as a first-class
  artifact, `execution_kernel_live_fallback_retirement_status`, so the current
  blockers and guardrails are available through the shared contract lane before
  any runtime-control-flow removal work starts
- that artifact now also freezes the public execution-kernel metadata fields,
  current dry-run/live delegate truth, and retirement gates such as
  `dry_run_delegate_still_python_owned` and
  `in_process_replacement_complete=false`
- the next safe slice is now also externalized as
  `execution_kernel_live_response_serialization_contract`, which freezes the
  current `RunTaskResponse` shape plus the response metadata invariants for
  live primary, compatibility fallback, and deterministic dry-run paths
- `execution_kernel_delegate_family` and
  `execution_kernel_delegate_impl` are now also part of the stable
  execution-kernel contract descriptor, so callers can read delegate
  family/impl directly from the shared contract lane even when live fallback is
  disabled
- it now also enumerates the remaining Python-owned retirement surfaces:
  dry-run prompt-preview generation, compatibility fallback agent factory,
  compatibility live response serialization, and fallback-reason metadata
- `execution_kernel_fallback_reason` remains compatibility-owned response
  metadata: it is externalized for retirement/parity purposes, but it is not
  promoted to framework truth or used to drive runtime branching
- this response-serialization artifact still records
  `compatibility_live_response_serialization` as Python-owned implementation
  territory; externalizing the contract does not imply Python runtime control
  flow has been retired
- compatibility fallback semantics are now reported separately through
  `execution_kernel_live_fallback_enabled` and
  `execution_kernel_live_fallback_mode=compatibility|disabled`, instead of
  encoding fallback state inside the primary live contract mode string
- when compatibility fallback is disabled, normal live execution stays
  Rust-only, `execution_kernel_live_fallback*` metadata may be `null`, and
  dry-run delegate metadata still points at the Python kernel adapter
- interrupt-style background replacements now use a reserved session takeover
  handoff before the new job queues, reducing the old release-then-requeue race
- pending takeover reservations are now persisted as part of the durable
  background-state contract so restart recovery does not silently drop the
  replacement intent
- background queue admission now checks an explicit admitted-job count instead
  of peeking into `Semaphore._value`

One additional Rust-authority slice is now implemented in this wave:

- `scripts/route.py` is now a Rust transport shim for search / route output,
  so the non-runtime CLI surface no longer carries a second Python route
  authority
- `router-rs` now also emits a stable route-policy payload for
  `python/shadow/verify/rust` plus rollback activation, so route mode /
  rollback / primary-authority decisions are no longer hardcoded in Python
  `RouterService`
- `router-rs` route JSON now carries an explicit authority marker and decision
  schema version, and the Python runtime adapter validates those fields before
  trusting the result
- `router-rs` now also owns the stable `RouteDiffReport` compare path for
  `shadow` / `verify` / rollback semantics, so Python no longer computes the
  Rust-side mismatch vocabulary locally

The boundary is still explicit:

- Rust is not yet the live in-process runtime kernel
- `RouterService` only hydrates the Python runtime router when the Rust-emitted
  route policy says `python_route_required=true`
- the next Rust convergence target is to make Rust-only live execution the
  default operational mode and then retire the remaining Python live fallback
  compatibility path entirely
- the next runtime target is to push the new event bridge beyond in-memory
  local delivery into stronger non-filesystem and consumer-handoff transport
  boundaries

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

- `codex_agno_runtime.router.SkillRouter.route()`
- `PrepareSessionResponse`
- `RoutingResult`

Required fields:

- `task: string`
- `session_id: string`
- `selected_skill: string`
- `overlay_skill: string | null`
- `layer: string`
- `score: float`
- `reasons: string[]`
- `prompt_preview: string | null`
- `route_engine: python | rust`
- `rollback_to_python: boolean`
- `shadow_route_report: route diff report | null`

Invariants:

- exactly one primary owner
- at most one overlay
- `layer` must match the selected skill layer
- `overlay_skill` must never equal `selected_skill`
- if no viable hit exists, fallback still returns a valid primary owner

Rust migration note:

- Rust owns scoring, route picking, route-mode / rollback policy, and
  canonical route snapshot shaping through versioned route and snapshot
  contracts
- Python may continue hydrating full skill bodies and prompt previews

## Contract 1A: Route Policy

Purpose:

- define the stable route-mode policy shared by `RouterService`, health
  reporting, and rollback/shadow/verify selection

Compatibility targets:

- `RouterService.route()`
- `RouterService.health()`
- `RustRouteAdapter.route_policy()`

Required fields:

- `mode: python | verify | shadow | rust`
- `rollback_active: boolean`
- `python_route_required: boolean`
- `primary_authority: python | rust`
- `route_result_engine: python | rust`
- `shadow_engine: python | rust | null`
- `diff_report_required: boolean`
- `verify_parity_required: boolean`

Invariants:

- `rollback_active` may only be true when `mode=rust`
- `python_route_required=true` means Python route execution is required for the
  current lane; it does not make Python the live runtime kernel
- `primary_authority` and `route_result_engine` must match the engine whose
  route result is returned to the Python runtime
- `shadow_engine` names the comparison lane only; it may be null for pure
  single-engine execution
- verify mode must require both a diff report and parity enforcement

## Contract 1B: Route Diff Report

Purpose:

- define the stable shadow / verify / rollback vocabulary shared by runtime
  traces, response metadata, and future soak dashboards

Compatibility targets:

- `RouterService.route()`
- `PrepareSessionResponse.shadow_route_report`
- `RoutingResult.shadow_route_report`
- `route.selected` trace payload

Required fields:

- `mode: python | verify | shadow | rust`
- `primary_engine: python | rust`
- `shadow_engine: python | rust | null`
- `mismatch: boolean`
- `mismatch_fields: string[]`
- `selected_skill_match: boolean`
- `overlay_skill_match: boolean`
- `layer_match: boolean`
- `score_bucket_match: boolean`
- `reasons_class_match: boolean`
- `rollback_active: boolean`
- `python.selected_skill: string`
- `python.overlay_skill: string | null`
- `python.layer: string`
- `python.score_bucket: string`
- `python.reasons_class: string`
- `rust.selected_skill: string`
- `rust.overlay_skill: string | null`
- `rust.layer: string`
- `rust.score_bucket: string`
- `rust.reasons_class: string`

Invariants:

- `selected_skill / overlay_skill / layer` are the critical parity gates for
  verify-mode acceptance
- `score_bucket` and `reasons_class` are soak-observability fields and may
  drift without blocking runtime execution
- shadow mode executes only the primary engine result while always preserving
  the full diff payload
- rollback mode may return the Python route while Rust policy still records
  Rust shadow evidence
- no new shadow artifact name may be introduced for this payload; it lives in
  canonical trace / response metadata only

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
- prompt-policy text assembly remains Python-owned for session preview,
  middleware mutation, deterministic dry-run surfaces, and the explicit
  compatibility fallback only
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

- Rust should own event typing and writing
- Python may still provide event producers initially
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

## Contract 8: Sandbox Boundary

Status:

- frozen as a contract and regression skeleton; implementation remains pending

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

- frozen as a design contract and regression skeleton; implementation remains
  pending

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
