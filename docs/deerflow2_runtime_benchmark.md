# DeerFlow 2.0 Runtime Benchmark

## Purpose

This note records what the current DeerFlow 2.0 runtime is doing that is worth
borrowing, adapting, or explicitly avoiding in the Codex runtime.

Date of benchmark: 2026-04-16 UTC.

Primary sources:

- https://github.com/bytedance/deer-flow
- https://github.com/bytedance/deer-flow/blob/main/README.md
- https://github.com/bytedance/deer-flow/blob/main/backend/README.md
- https://github.com/bytedance/deer-flow/blob/main/backend/CLAUDE.md
- https://github.com/bytedance/deer-flow/blob/main/backend/packages/harness/deerflow/runtime/runs/manager.py

## Verified Baseline

- DeerFlow 2.0 lives on the `main` branch of `bytedance/deer-flow`; the old
  line stays on `main-1.x`.
- DeerFlow 2.0 describes itself as a ground-up rewrite and a super-agent
  harness rather than a narrow deep-research framework.
- It already supports two runtime shapes:
  - standard mode: Gateway + LangGraph server
  - gateway mode: embedded runtime inside Gateway
- The current runtime is still Python/LangGraph-based, but it has clearer
  seams than our local runtime for:
  - run lifecycle management
  - stream bridging
  - sandbox abstraction
  - store/checkpointer backend selection
  - harness/app separation

## Borrow / Adapt / Avoid

### Borrow

- Harness/app split:
  - keep runtime core isolated from HTTP/UI/channel adapters
  - make the dependency direction one-way
- Run-manager semantics:
  - explicit run status
  - explicit conflict policy (`reject`, `interrupt`, `rollback`)
  - explicit cancellation path
- Embedded-runtime migration:
  - keep external API shape stable while moving execution into owned runtime
- Replayable stream bridge:
  - durable event model
  - resumable streaming instead of ad-hoc polling only
- Unified persistence seam:
  - store and checkpointer should choose the same backend family

### Adapt

- Sandbox abstraction:
  - useful model, but our runtime should align it to
    `runtime_sandbox_contract.md` rather than copying DeerFlow's provider
    layout directly
- Middleware chain:
  - useful for decomposition and observability
  - our local runtime already borrowed this idea, but still lacks a richer
    control-plane around it
- Per-thread workspace model:
  - useful for deterministic file semantics
  - should be tied to our artifact and continuation contracts

### Avoid

- Reflection-heavy runtime wiring as the long-term center of gravity
- Process-global config assumptions
- Treating in-memory run registries as the end-state for shared or
  multi-instance runtime deployment
- Copying LangGraph-specific runtime shapes into Rust without first freezing the
  Codex-owned state machine

## Codex Mapping

### DeerFlow pattern -> Codex target

- harness/app split -> `framework_runtime` core vs future HTTP/CLI/channel adapters
- `RunManager` -> our background job state machine and future run kernel
- `StreamBridge` -> future resumable trace/stream transport for long runs
- unified store/checkpointer -> first land a narrow runtime checkpointer seam,
  then swap backend families underneath it
- gateway mode -> future owned runtime mode that no longer depends on a thin
  external orchestrator process

### Current Codex gaps

- only an in-memory producer/consumer stream bridge exists so far; there is no
  host-bound SSE/distributed bridge yet
- the compaction/checkpoint backend family now exists as an abstraction, but
  only the filesystem backend is concrete so far
- no real sandbox control plane yet
- no Rust live in-process kernel yet; live execution is now Rust-first through
  `router-rs`, while the old Python fallback survives only as a retired
  explicit-request surface that returns rejection metadata
- Python and Rust still duplicate some contract/compiler logic

## Immediate Next Wave

The next runtime wave should stay incremental and avoid big-bang kernel
rewrites.

Priority order:

1. keep collapsing the residual route metadata/canonicalization lane behind the
   Rust-owned route policy/compiler authority
2. keep the compatibility fallback retirement artifact as historical evidence,
   but do not reopen any request shim, settings/env exposure, or Python live
   path
3. decide which middleware transforms stay as Python host callbacks vs move
   behind the execution-kernel seam
4. keep extending resumable persistence and backend-family seams without
   re-opening the kernel boundary again

The first concrete runtime control-plane improvement already landed in this
round:

- background runs now support explicit `multitask_strategy`
  (`reject` or `interrupt`) instead of only implicit duplicate-session failure
- runtime traces now expose replayable `seq` / `cursor` metadata and resume
  windows, which gives the repo its first resumable stream seam without
  pretending that live SSE bridge work is done
- runtime now exposes an in-memory `RuntimeEventBridge` with
  `subscribe/resume/heartbeat/cleanup`, so the stream seam is no longer
  trace-only metadata
- runtime now exposes a versioned poll-transport descriptor for that bridge,
  which gives host adapters a discoverable transport handshake without
  pretending that SSE or remote transport is already done
- runtime now also exposes a handoff descriptor plus a persisted JSON binding
  artifact, so another host or bridge sharing the backend can re-attach using
  replay/checkpoint anchors without inventing a second stream protocol
- that descriptor now explicitly advertises a host-facing/remote-capable seam
  and documents that cleanup clears only bridge cache while replay can reseed
  resumability from persisted events
- the old `scripts/route.py` Python shim is retired; route/search CLI calls go
  directly to `router-rs`, so the CLI surface no longer carries a parallel
  Python scoring authority
- runtime route mode / rollback / primary-authority policy now also comes from
  `router-rs`, so Python no longer hardcodes which route engine is primary for
  health or execution handoff
- runtime now routes trace paths, resume manifest writes, and background-state
  path discovery through a shared filesystem `RuntimeCheckpointer` seam
- the runtime checkpointer and durable background-state store now share the
  same backend-family abstraction, which is still filesystem-only today but is
  the staging point for compaction / snapshot-delta work
- interrupt-style background replacements now reserve a pending takeover before
  preemption completes, which closes the old release-then-requeue ownership gap
- pending takeover reservations are now persisted, so restart recovery keeps
  the replacement intent instead of dropping it in memory
- background admission now uses an explicit admitted-job count rather than
  reading the raw semaphore permit value
- runtime execution now flows through a single execution-kernel adapter seam
  instead of inlining dry/live execution branches in `runtime.py`
- live execution now enters a Rust-owned `router-rs --execute-json` contract,
  so Python no longer owns the actual live model call or run-response
  normalization on the primary path

What this changes in practice:

- DeerFlow's stream-bridge idea is now reflected in the local runtime as a
  replay/resume seam plus a real in-memory producer/consumer bridge
- the next runtime slice should extend that handoff seam beyond filesystem-only
  binding artifacts into stronger host/remote transport backends instead of
  redoing the same bridge in another local form
- the next Rust slice should delete the remaining Python fallback and continue
  collapsing residual Python-side prompt-preview/result shaping behind the
  Rust-owned execution contract, but the current repo must not describe the
  retired request shim as a runnable fallback path

## Decision

Use DeerFlow 2.0 as a benchmark for runtime decomposition and migration
strategy, not as a drop-in implementation template.
