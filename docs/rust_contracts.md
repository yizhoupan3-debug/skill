# Runtime Rust Contracts

## Purpose

This document freezes the Rust-owned runtime contracts for this repository.
Historical migration notes live under `docs/history/`; this file describes only
the current runtime truth in `router-rs` and related Rust tools.

Upper-layer control-plane narrative (L1–L5, evidence/resume injection boundaries):
[`harness_architecture.md`](harness_architecture.md). Host adapter portability (portable core, event→CLI pointers, new-host checklist):
[`host_adapter_contract.md`](host_adapter_contract.md). Steady-state doc index:
[`README.md`](README.md) in this directory.

It is the contract source of truth for:

- routing and route diagnostics
- profile / explicit host projection compilation
- execution response shape
- runtime control-plane descriptors
- framework runtime snapshot / artifact continuity
- trace transport, checkpointing, compaction, observability, and sandbox policy

## Harness architecture (control plane)

Upper-level layering for hooks, continuity artifacts, and evidence flows lives in [`harness_architecture.md`](harness_architecture.md) (L1–L5 model, extension rules). **Closed-set host ids** and install/sync alignment with manifests: `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` (see [`host_adapter_contract.md`](host_adapter_contract.md)). Operator nudge strings for RFV / Autopilot hooks are loaded from `configs/framework/HARNESS_OPERATOR_NUDGES.json` (`harness_operator_nudges`); disable with `ROUTER_RS_HARNESS_OPERATOR_NUDGES=0`. Rust contracts below remain the implementation authority.

## Current Boundary

Rust owns the default runtime and contract path.

- `router-rs route <query>` owns route decisions; route diagnostics use the Rust stdio route policy/report operations.
- `router-rs profile emit` and `router-rs profile artifacts` own the shared framework profile plus explicit Codex projection artifacts.
- Rust stdio `execute` operation owns the live/dry-run execution response contract.
- `router-rs framework snapshot`, `contract-summary`, `session-artifact-write`, `hook-evidence-append`, and `prompt-compression` own framework runtime read/write/policy surfaces. Cursor `PostToolUse` may append `cursor_post_tool_verification` rows (terminal tools + verification-shaped commands) alongside Codex `codex_post_tool_verification` and `rust-lint`’s `cursor_rust_lint` hook evidence.
- Stdio op `framework_hook_evidence_append` mirrors `router-rs framework hook-evidence-append --input-json …` for scripted callers appending rows to `EVIDENCE_INDEX.json` under continuity (same payload shape as the CLI).
- `router-rs codex sync` owns repo host-entrypoint materialization.
- `router-rs framework host-integration ...` owns native install/status/remove, bootstrap, projection, and related host integration flows. `router-rs codex host-integration ...` is a thin compatibility alias only.

## Current Status Ledger

### 当前真源

- Routing authority is Rust.
- Live execution and dry-run preview use Rust stdio.
- Runtime control plane publishes Rust-owned authority for `router`, `state`, `trace`, storage, and `background`.
- Framework snapshot, contract summary, session artifact writing, hook evidence append (CLI + stdio), and prompt policy use direct `router-rs` surfaces.
- Host entrypoint sync and native integration are Rust-owned through `router-rs`; the **closed-set supported host projections** are defined by **`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`** (install-skills/tool spellings derive from `framework_host_targets` in router-rs); the checkout default lists `codex-cli` and `cursor`.
- Runtime traces expose resumable `seq` / `cursor` metadata, transport binding artifacts, handoff descriptors, and process-external attach resolution.
- Runtime storage exposes backend-family capability discovery, digest verification, and fail-closed alignment between store/checkpointer/trace/state families.
- SQLite is the strongest local backend for WAL, consistent append, compaction, and snapshot-delta support; filesystem remains the safe default storage.
- Session supervisor and background state expose Rust-owned tmux/session/rate-limit/resume control-plane records without external runtime dependency.
- Observability vocabulary, exporter descriptor, metric catalog, dashboard schema, and metric record payloads are Rust-owned.
- Sandbox lifecycle contract is frozen and has a minimal Rust-owned control-plane surface.

### 默认面边界

- Do not add a second route authority, default artifact emitter, host-specific generated layer, or parallel runtime state root.
- Generated host entrypoints are limited sync outputs, not hand-authored truth.
- Historical migration inventory belongs in `docs/history/`, not in steady-state contracts.

### 下一 safe slice

- Harden remote-capable attach/handoff/binding/replay semantics so every consumer uses the same descriptor contract.
- Deepen backend-family compaction and snapshot-delta behavior without changing logical state meaning.
- Expand sandbox lifecycle enforcement without claiming a remote sandbox backend before it exists.
- Keep host integration Rust-only and fail if generated entrypoints drift.
- Refresh docs and generated routing outputs only when contract changes require it.

## Contract Rules

- Contract changes must be explicit and versioned.
- Rust may replace implementations, not silently redefine semantics.
- Host-private fields stay under explicit host projection payloads such as `codex_profile.codex_host_payload`; they must not enter framework core truth.
- Active contracts must describe current owners and outputs, not migration inventory.
- Any alternate runtime, routing, artifact, hook, or host-integration implementation is a regression unless explicitly approved as a host-private edge script.

## Host Projection Invariants

- The shared framework core is the profile authority; host projections are closed-set and explicit.
- Supported host projections are **exactly** the ids enumerated under **`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`** (not a hard-coded second source); the bundled registry currently lists `codex-cli` and `cursor`.
- `codex_profile` is the Codex projection artifact and may carry Codex-private payload fields.
- Generated host projections are disposable install targets and must remain thin bootstrap pointers to the Rust core.
- `framework host-integration remove` removes only framework-owned projection files and manifest-recorded settings keys; user-authored files and unrelated settings are preserved.
- `framework host-integration compatibility-aliases` is the machine-readable inventory for retained aliases such as `install-skills`, `codex host-integration`, and `--repo-root`; each entry must include owner, reason, primary command, kept policy, removal condition, and `independent_behavior: false`.
- `configs/framework/GENERATED_ARTIFACTS.json` declares checked-in generated artifacts with schema `framework-generated-artifacts-manifest-v1`; `framework host-integration generated-artifacts-status` is a manifest-backed byte-for-byte drift gate that regenerates declared artifacts in an isolated temporary root, compares manifest-declared outputs, reports undeclared generated framework artifacts across reverse-reference surfaces, and rejects expanded host-private paths in shared artifacts.

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
- fallback selection may choose a safe owner from `SKILL_MANIFEST.json`, but must not introduce a second route authority
- generated framework command aliases must name an existing manifest owner as `canonical_owner`; deleted historical owners may only appear under `docs/history/`

## Runtime Control Contracts

Runtime control-plane payloads must keep these owner markers stable:

- `rust-route-core`
- `rust-route-compiler`
- `rust-runtime-control-plane`
- `rust-runtime-storage`
- `rust-runtime-trace-io`
- `rust-framework-runtime-read-model`
- `rust-framework-session-artifact-writer`
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
