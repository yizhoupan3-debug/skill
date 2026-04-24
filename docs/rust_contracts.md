# Runtime Rust Contracts

## Purpose

This document freezes the Rust-owned runtime contracts for this repository.
Older migration notes may still mention Python cutover work, but current runtime
truth lives in `router-rs` and related Rust tools.

It is the contract source of truth for:

- routing and route diagnostics
- profile / adapter / artifact compilation
- execution response shape
- runtime control-plane descriptors
- framework runtime snapshot / memory / artifact continuity
- trace transport, checkpointing, compaction, observability, and sandbox policy

## Current Boundary

Rust owns the default runtime and contract path.

- `router-rs --route-json`, `--route-policy-json`, `--route-report-json` own route decisions.
- `router-rs --profile-json` and `--profile-artifacts-json` own profile, adapter, parity, capability discovery, compatibility inventory, and shared contract artifacts.
- `router-rs --execute-json` owns the live/dry-run execution response contract.
- `router-rs --framework-runtime-snapshot-json`, `--framework-contract-summary-json`, `--framework-memory-recall-json`, `--framework-session-artifact-write-json`, `--framework-memory-policy-json`, and `--framework-prompt-compression-json` own framework runtime read/write/policy surfaces.
- `router-rs --sync-host-entrypoints-json` owns repo host-entrypoint materialization.
- `router-rs --host-integration ...` owns native install, bootstrap, skill install, memory automation, and related host integration flows.
- Rust memory policy persistence writes SQLite rows and, by default, appends deduped stable facts to `decisions.md`; `stable_journal: false` is the explicit opt-out.

Retired surfaces stay retired:

- no `framework_runtime/` Python package
- no Python route shim
- no Python live fallback
- no Python/Rust parity report
- no Python artifact emitter as a second truth
- no OMC or `.omc/**` runtime state

## Current Status Ledger

### 已实现

- Routing authority defaults to Rust; the old `python` route engine is retired.
- Live execution and dry-run preview stay Rust-only by default.
- Compatibility live fallback request surface has been removed; historical fallback metadata may appear only as retired legacy evidence.
- Runtime control plane publishes Rust-owned authority for `router`, `state`, `trace`, `memory`, and `background`.
- `framework_runtime/` Python package is retired; framework snapshot, contract summary, memory recall, session artifact writing, prompt/memory policy, and framework MCP use `router-rs` surfaces.
- Memory policy extraction reports source/fact counts and can persist to both `memory.sqlite3` and the stable `decisions.md` journal without introducing a Python writer.
- Host entrypoint sync and native integration are Rust-owned through `router-rs`.
- Runtime traces expose resumable `seq` / `cursor` metadata, transport binding artifacts, handoff descriptors, and process-external attach resolution.
- Runtime storage exposes backend-family capability discovery, digest verification, and fail-closed parity between store/checkpointer/trace/state families.
- SQLite is the strongest local backend for WAL, consistent append, compaction, and snapshot-delta support; filesystem remains compatibility-safe default storage.
- Session supervisor and background state expose Rust-owned tmux/session/rate-limit/resume control-plane records without OMC dependency.
- Observability vocabulary, exporter descriptor, metric catalog, dashboard schema, and metric record payloads are Rust-owned.
- Sandbox lifecycle contract is frozen and has a minimal Rust-owned control-plane surface.

### 已退休

- `scripts/route.py`
- `scripts/materialize_cli_host_entrypoints.py`
- `scripts/install_codex_native_integration.py`
- `scripts/write_session_artifacts.py`
- `scripts/framework_hook_bridge.py`
- `scripts/runtime_background_cli.py`
- `framework_runtime/`
- `pytest.ini` and Python test entrypoints
- `rust_execute_fallback_to_python`
- `rust_python_artifact_parity_report.json`
- default emission of fallback host artifacts
- OMC / `oh-my-claudecode` as live runtime, prompt, plugin, or state dependency

### 下一 safe slice

- Harden remote-capable attach/handoff/binding/replay semantics so every consumer uses the same descriptor contract.
- Deepen backend-family compaction and snapshot-delta behavior without changing logical state meaning.
- Expand sandbox lifecycle enforcement without claiming a remote sandbox backend before it exists.
- Keep host integration Rust-only and fail if generated entrypoints drift.
- Refresh docs and generated routing outputs only when contract changes require it.

## Contract Rules

- Contract changes must be explicit and versioned.
- Rust may replace implementations, not silently redefine semantics.
- Host-private fields stay in host projection or compatibility lanes; they must not enter framework truth.
- Compatibility inventory is not a regression baseline.
- Generated host entrypoints are projections, not hand-authored truth.
- Any new Python runtime, routing, artifact, hook, or host-integration implementation is a regression unless explicitly approved as a host-private edge script.

## Host / Adapter Invariants

- `cli_common_adapter` is the canonical CLI-family shared contract.
- `codex_common_adapter` is only a Codex compatibility naming view.
- `codex_desktop_adapter` is the canonical desktop identity.
- `codex_cli_adapter`, `claude_code_adapter`, and `gemini_cli_adapter` are thin host projections.
- `codex_desktop_host_adapter` is a retired compatibility alias and can appear only through explicit continuity opt-in.
- `aionrs_companion_adapter`, `aionui_host_adapter`, and `generic_host_adapter` are retired inventory rows, not default peer adapters.
- `codexcli` is a headless execution entrypoint, never framework truth.

## Route Contract

Required route result fields:

- `task`
- `session_id`
- `selected_skill`
- `overlay_skill`
- `layer`
- `score`
- `reasons`
- `prompt_preview`
- `route_engine`
- `diagnostic_route_mode`
- `route_diagnostic_report`

Invariants:

- exactly one primary owner
- at most one overlay
- `route_engine` and primary authority stay Rust
- unknown selected skills fail closed in consumers
- fallback selection may choose a safe owner, but must not reintroduce a Python route authority

## Runtime Control Contracts

Runtime control-plane payloads must keep these owner markers stable:

- `rust-route-core`
- `rust-route-compiler`
- `rust-runtime-control-plane`
- `rust-runtime-storage`
- `rust-runtime-trace-io`
- `rust-framework-runtime-read-model`
- `rust-framework-session-artifact-writer`
- `rust-framework-memory-policy`
- `rust-framework-prompt-policy`

## External Benchmark

DeerFlow 2.0 remains a useful benchmark for decomposition ideas:

- harness/app split
- explicit run-manager conflict semantics
- resumable stream bridge
- unified store/checkpointer seams
- sandbox lifecycle boundaries

It is not a template to copy directly. This repo keeps its own Rust-owned state
machine and avoids LangGraph-shaped or reflection-heavy runtime assumptions.
