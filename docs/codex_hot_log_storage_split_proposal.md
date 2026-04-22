# Codex Hot Log Storage Split Proposal

## Summary

This proposal recommends separating Codex's durable state store from its
high-write hot log store instead of continuing to persist both classes of data
inside the same SQLite lifecycle.

The recommended steady-state architecture is:

- keep `state_*.sqlite` as the durable control-plane store
- move hot logs, event traces, and high-volume diagnostic payloads to a
  segmented append-only log sink
- keep optional background import or export paths for support bundles, but stop
  making SQLite the primary hot-path log sink

This is not a request to swap the whole local persistence layer to another
database. It is a request to stop using the same SQLite maintenance model for
two very different workloads.

## Why This Needs A Design Change

Current Codex behavior already distinguishes a SQLite-backed state surface from
other on-disk runtime artifacts, but the implementation still allows large
volumes of hot log traffic to accumulate in `logs_*.sqlite`. In practice, that
creates one failure pattern:

- high-frequency append traffic grows the log database quickly
- long-lived readers and multiple live Codex processes keep WAL files open
- retention-only fixes remove rows logically but do not reclaim space fast
  enough
- VACUUM and checkpoint work either lags behind or is skipped while the runtime
  is still active
- once the store becomes large, every write and maintenance operation gets more
  expensive than the control-plane state actually needs

The result is that a "debug log" workload can degrade the same local storage
stack that session state, thread metadata, resume pointers, and local control
surfaces depend on.

## Problem Statement

Codex currently uses SQLite for at least two distinct workload classes:

1. Durable state:
   thread metadata, session state, control-plane records, resumable pointers,
   and other small transactional objects.
2. Hot logs:
   high-frequency event streams, SSE/debug traces, transport diagnostics, and
   large append-heavy payloads.

These workloads have different storage needs:

- durable state wants transactions, indexes, and small-point reads
- hot logs want cheap append, bounded retention, cheap rollover, and minimal
  write amplification

SQLite is a good fit for the first class. It is a poor primary sink for the
second class once installs stay active for long periods or multiple Codex
processes are alive at the same time.

## Goals

- protect the state database from hot-log growth and write pressure
- make log retention bounded by design, not only by delayed cleanup
- reduce WAL growth and maintenance contention under concurrent Codex runtimes
- preserve local debugging and support-bundle workflows
- keep migration low-risk for existing installs

## Non-Goals

- replacing all local persistence with Postgres, RocksDB, or another new global
  backend
- changing the meaning of existing session/state records
- removing local logs entirely
- requiring a daemon or external service just to run Codex locally

## Recommended Architecture

### 1. Keep State In SQLite

Continue storing durable control-plane state in `state_*.sqlite`:

- thread/session metadata
- resumable state and durable cursors
- local approvals or UI state
- compact indexes needed for fast point lookups

This keeps the strongest part of the current design: a small embedded
transactional store with predictable semantics.

### 2. Move Hot Logs To A Segmented Append-Only Sink

Store hot logs in rotated segment files under a dedicated log root, for example:

- `~/.codex/logstore/YYYY/MM/DD/<process-id>-<segment>.jsonl`
- optionally compressed after rollover with `zstd`

Required properties:

- append-only while active
- size-based rotation
- age-based and total-size-based retention
- no global VACUUM requirement
- no shared B-tree growth on the write hot path

Recommended record shape:

- one JSONL object per event
- explicit `ts`, `level`, `target`, `thread_id`, `process_uuid`
- payload body stored inline only when small
- large blobs moved behind artifact refs or truncated summaries

### 3. Separate Reader Paths

Codex should stop treating hot-log queries and state queries as one storage
problem.

- state reads continue to hit SQLite
- log inspection reads recent segments directly
- historical support export can merge segments into a bundle on demand

This keeps the common case fast without forcing every diagnostic write through a
database page lifecycle.

### 4. Optional Cold Import Path

If maintainers still want SQL-style querying for support or analytics, make it a
background or explicit cold path:

- import rotated segments into a temporary SQLite/DuckDB bundle for analysis
- or ship structured logs directly through OTel / support export tooling

The key change is that SQLite should be a derived analysis surface, not the hot
log source of truth.

## Why Not "Just Use Another Database"

Replacing SQLite wholesale is the wrong granularity.

- Postgres solves concurrency, but adds a service dependency and too much local
  operational cost.
- RocksDB/redb/LMDB would require rewriting state semantics and query paths that
  already fit SQLite well.
- DuckDB is better for analysis than for hot operational logging.

The real mismatch is not "SQLite bad". The mismatch is "SQLite state store plus
SQLite hot log sink under one maintenance model".

## Design Details

### Log Segment Lifecycle

One process writes only to its current segment.

Rollover triggers:

- segment size exceeds threshold, for example `32-64 MiB`
- process lifetime ends
- idle flush plus rotation window completes

Retention policy applies on segment files, not rows:

- drop files older than `N` days
- enforce a hard total budget such as `512 MiB` or `1 GiB`
- prefer deleting oldest closed segments first

This makes retention deterministic and cheap.

### Crash Recovery

On startup:

- recover any unclosed segment by scanning to the last valid JSONL line
- mark the recovered segment immutable
- start a new active segment

No VACUUM or retroactive page reclamation is needed.

### Concurrency Model

The split design should assume multiple live Codex processes are normal.

- every process owns its active log segment
- no shared writer lock is needed for hot logs
- only state updates keep using SQLite transactions

This removes the "many processes, one hot log database, one WAL" failure mode.

### Support And Diagnostics

`codex debug` or export tooling can still provide one merged view by:

- scanning recent segments
- filtering by `thread_id` / `process_uuid`
- packaging selected ranges into a support archive

This preserves usability without keeping the primary write path inside SQLite.

## Migration Plan

### Phase 1: Dual Write

- introduce a new segmented log sink behind a feature flag
- keep existing SQLite log writes temporarily
- compare event counts, sizes, and support/debug parity

### Phase 2: Segmented Log Default

- make segmented logging the default for hot logs
- keep SQLite log ingestion only as an opt-in legacy mode
- keep state SQLite untouched

### Phase 3: Remove SQLite As Primary Log Sink

- stop writing hot logs to `logs_*.sqlite` by default
- keep only migration/readback tooling for old installs
- reserve SQLite for state and small indexed metadata only

## Proposed Config Surface

Example only:

```toml
[log_store]
backend = "segmented_jsonl"
root = "~/.codex/logstore"
segment_max_bytes = 67108864
retention_days = 7
max_total_bytes = 536870912
compression = "zstd"
legacy_sqlite_mirror = false
```

Key point: this is a log-store choice, not a global persistence-backend choice.

## Acceptance Criteria

- hot-log growth no longer increases `state_*.sqlite` or blocks state access
- a long-running install can bound log disk usage by file retention alone
- multiple concurrent Codex processes do not share one hot-log WAL
- support/debug flows can still retrieve recent logs by thread or process
- migration does not require deleting or rewriting existing state databases

## Risks And Mitigations

### Risk: support tooling currently expects SQL queries

Mitigation:

- add a log-reader library over segment files
- keep a temporary import-to-SQL tool for support bundles

### Risk: too many small files

Mitigation:

- use segment sizing and compression
- group by day/process

### Risk: schema drift in JSONL logs

Mitigation:

- version the log envelope explicitly
- keep the same canonical event vocabulary already used by observability

## Why This Is Better Than Retention + VACUUM Alone

Retention + VACUUM improves symptoms, but it does not change the workload
coupling:

- the log hot path still pays SQLite write costs
- the runtime still depends on maintenance windows to stay healthy
- concurrent processes can still pin the same WAL lifecycle

Splitting state and hot logs removes the coupling instead of only managing its
side effects.

## Upstream Ask

The upstream change request is:

1. keep SQLite as the embedded durable state store
2. stop using SQLite as the primary hot log sink
3. introduce a segmented append-only local log store with bounded retention
4. keep support/export/query tooling as a derived layer, not the hot-path write
   layer

That is the smallest architectural change that addresses the real failure mode
without forcing a full local database rewrite.
