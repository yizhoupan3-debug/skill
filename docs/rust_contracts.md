# Runtime Rust Contracts

## Purpose

This document freezes the Rust-owned runtime contracts for this repository.
Historical migration notes live under `docs/history/`; this file describes only
the current runtime truth in `router-rs` and related Rust tools.

It is the contract source of truth for:

- routing and route diagnostics
- profile / Codex artifact compilation
- execution response shape
- runtime control-plane descriptors
- framework runtime snapshot / memory / artifact continuity
- trace transport, checkpointing, compaction, observability, and sandbox policy

## Current Boundary

Rust owns the default runtime and contract path.

- `router-rs route <query>` owns route decisions; route diagnostics use the Rust stdio route policy/report operations.
- `router-rs profile emit` and `router-rs profile artifacts` own the Codex profile and `codex_profile` artifact.
- Rust stdio `execute` operation owns the live/dry-run execution response contract.
- `router-rs framework snapshot`, `contract-summary`, `memory-recall`, `session-artifact-write`, `memory-policy`, and `prompt-compression` own framework runtime read/write/policy surfaces.
- `router-rs codex sync` owns repo host-entrypoint materialization.
- `router-rs codex host-integration ...` owns native install, bootstrap, skill install, memory automation, and related host integration flows.
- Rust memory policy persistence writes SQLite rows and, by default, appends deduped stable facts to `decisions.md`; `stable_journal: false` is the explicit opt-out.

## Current Status Ledger

### 当前真源

- Routing authority is Rust.
- Live execution and dry-run preview use Rust stdio.
- Runtime control plane publishes Rust-owned authority for `router`, `state`, `trace`, `memory`, and `background`.
- Framework snapshot, contract summary, memory recall, session artifact writing, and prompt/memory policy use direct `router-rs` surfaces.
- Memory policy extraction reports source/fact counts and can persist to both `memory.sqlite3` and the stable `decisions.md` journal without introducing an alternate writer.
- Host entrypoint sync and native integration are Rust-owned through `router-rs`; the supported host entrypoints are Codex CLI and Codex App.
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
- Codex host-private fields stay in `codex_profile.codex_host_payload`; they must not enter framework truth.
- Active contracts must describe current owners and outputs, not migration inventory.
- Any alternate runtime, routing, artifact, hook, or host-integration implementation is a regression unless explicitly approved as a host-private edge script.

## Codex Profile Invariants

- `codex_profile` is the only default profile artifact key.
- Codex host-private fields stay in `codex_profile.codex_host_payload`.
- Host-specific generated artifacts are not default runtime surfaces.
- Host entrypoint sync materializes `AGENTS.md` and `.codex/host_entrypoints_sync_manifest.json`.

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
