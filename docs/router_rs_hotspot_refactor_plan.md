# router-rs Hotspot Refactor Plan

Date: 2026-04-25

## Scope

This document records the current `router-rs` hotspot diagnosis and the refactor direction for the large Rust entry files, especially `scripts/router-rs/src/main.rs` and the extracted route engine module.

The immediate goal is to reduce entrypoint sprawl, make lock/thread ownership visible, and keep route/search logic out of the CLI/runtime control plane.

## Diagnosis

`main.rs` had accumulated several unrelated ownership domains:

- CLI parser and command dispatch.
- Legacy top-level JSON flags that bypassed canonical subcommands.
- Stdio request routing and bounded worker admission.
- Route/search/scoring/eval code.
- Runtime, trace, background, sandbox, checkpoint, and framework control paths.

This created two practical problems.

- The route engine had no module boundary. Search, scoring, inline record loading, cache state, route policy generation, route diff reporting, and eval fixtures all lived inside `main.rs`, so every runtime or CLI change risked touching the hottest file.
- There were too many entrypoints. Old top-level flags such as route JSON, framework snapshot/refresh/profile, browser MCP, runtime storage, route report, host integration, and codex-hook compatibility modes kept extra branches alive even though canonical subcommands already existed.

The lock/thread risks are mostly ownership risks rather than one obvious deadlock.

- There are nested schedulers: external host/browser process pools, Rust stdio bounded workers, and Rayon parallel scans in the route engine.
- The route records cache is process-local and mutex-protected. That is fine inside one process, but it is not a cross-process cache and it should not be owned from the CLI entry file.
- Background/session state and trace/runtime append paths still need explicit single-writer, transaction, or file-lock ownership. They are separate from route scoring, but they are currently too close in `main.rs`.
- Stdio request handling, route execution, and runtime storage all sharing the same giant dispatch file makes contention and cancellation behavior hard to audit.

## Completed In This Refactor

- Added `scripts/router-rs/src/route.rs`.
- Moved route/search/scoring/cache/eval data types and functions out of `main.rs` into `route.rs`.
- Moved route schema/authority constants and route-only scoring thresholds into `route.rs`.
- Added `scripts/router-rs/src/stdio_transport.rs`.
- Moved stdio JSON transport, bounded worker admission, ordered response handling, stdio request/response envelopes, and stdio concurrency defaults out of `main.rs` into `stdio_transport.rs`.
- Deleted retired top-level legacy CLI fields and branches for old browser MCP, route JSON, route policy, framework snapshot/refresh/memory/profile, route report, runtime storage, codex hook/sync/check, and host integration modes.
- Removed the implicit top-level `--query` fallback. Callers now need canonical subcommands such as `search` or `route`.
- Reduced `main.rs` from about 14k lines to about 10.4k lines while keeping existing router tests green.

## Direct Deletes And Simplifications

The following old surfaces were removed instead of preserved behind compatibility branches:

- Top-level browser MCP stdio and artifact-resolution flags.
- Top-level route JSON and route policy JSON flags.
- Top-level framework runtime snapshot, contract summary, memory recall/policy, prompt compression, refresh, session artifact write, alias, and profile flags.
- Top-level route report compatibility branches and retired route JSON branches.
- Top-level runtime storage and codex hook projection/sync/check branches.
- Top-level host integration mode.
- The fallback path that treated a bare top-level query as a search request.

## Still Should Be Deleted Or Split Next

- Move the remaining top-level JSON modes into canonical subcommands or stdio-only execution paths.
- Continue narrowing stdio operation dispatch now that stdio transport/admission/ordered response handling has a module boundary.
- Split runtime/session/background/trace storage control out of `main.rs` so file writes and state transitions have one owner.
- Split `route.rs` again after this extraction stabilizes: `records`, `scoring`, `policy`, and `eval` are natural seams.
- Replace or narrow the process-local route cache mutex if profiling shows contention; it should remain clearly process-local unless a real cross-process cache is introduced.
- Add explicit CAS, transaction, or file-lock boundaries around background/session state and trace append paths.
- Reconcile stale docs or runbooks that still mention retired top-level compatibility flags.

## Target Shape

`main.rs` should eventually become only:

- argument parsing;
- canonical subcommand dispatch;
- wiring between modules;
- process exit and error formatting.

The long-lived modules should own their domains:

- `route`: skill records, search, scoring, route policy, route snapshots, and eval.
- `stdio`: transport framing, worker admission, request ordering, and stdio op registry.
- `runtime`: session execution, sandbox, checkpoint, and background lifecycle.
- `trace`: trace event persistence and replay.
- `framework`: framework contract/profile/statusline surfaces.

## Validation

The refactor is expected to pass:

```bash
cargo fmt --manifest-path scripts/router-rs/Cargo.toml
cargo check --manifest-path scripts/router-rs/Cargo.toml
cargo test --manifest-path scripts/router-rs/Cargo.toml
```

The policy surface should also keep passing the canonical-help contract test:

```bash
cargo test --test policy_contracts router_rs_top_level_help_exposes_only_canonical_subcommands
```
