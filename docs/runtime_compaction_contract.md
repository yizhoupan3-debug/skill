# Runtime Compaction Contract

## Purpose

This document freezes the first-pass compaction contract for the runtime
Workstream F in `aionrs_fusion_docs/codex_dual_entry_next_phase_checklist.md`.

It is the contract source of truth for:

- snapshot + delta splitting
- generation rollover
- artifact reference strategy
- replay and recovery invariants

It must remain compatible with existing runtime artifacts and must not silently
redefine the meaning of trace, journal, or durable state records.

## Contract Rules

- Contract changes must be explicit and versioned.
- Compaction may change storage shape, not logical meaning.
- Large payloads should move behind artifact refs instead of being duplicated
  in main history.
- Every generation must remain traceable to the previous stable snapshot.
- Replay must be possible from compacted artifacts without scanning the full
  historical JSONL stream.

## Contract 1: Snapshot Schema

Purpose:

- represent the stable checkpoint for one generation
- provide the minimum state required to resume replay

Compatibility targets:

- runtime trace metadata
- durable state stores
- future compaction manifest and recovery tooling

Required fields:

- `schema_version: string`
- `generation: integer`
- `snapshot_id: string`
- `parent_generation: integer | null`
- `parent_snapshot_id: string | null`
- `session_id: string`
- `job_id: string | null`
- `created_at: timestamp`
- `watermark_event_id: string | null`
- `state_digest: string`
- `artifact_index_ref: artifact ref | null`
- `state_ref: artifact ref | null`
- `delta_cursor: string | null`
- `summary: object`

Invariants:

- `schema_version` must be explicit and stable.
- `generation` must identify the compaction epoch the snapshot belongs to.
- `snapshot_id` must uniquely identify the checkpoint payload.
- `parent_snapshot_id` must point to the most recent stable snapshot when one
  exists.
- `state_digest` must match the serialized snapshot payload.
- `summary` may be compact, but it must still encode the state needed to
  resume execution safely.

## Contract 2: Delta Replay Contract

Purpose:

- represent append-only change records between snapshots
- make replay deterministic from the latest snapshot plus its deltas

Compatibility targets:

- runtime journal events
- trace replay and recovery logic
- future Rust replay engine

Required fields:

- `schema_version: string`
- `generation: integer`
- `delta_id: string`
- `parent_snapshot_id: string`
- `seq: integer`
- `ts: timestamp`
- `kind: string`
- `payload: object`
- `artifact_refs: artifact ref[]`
- `applies_to: object`

Replay rules:

- deltas within one generation must replay in ascending `seq` order.
- delta application must be idempotent for a given `delta_id`.
- replay must start from the latest stable snapshot and apply only the
  generation-local deltas after it.
- replay must not require scanning the full historical stream once the
  snapshot boundary is known.
- delta payloads may reference artifacts, but large blobs must not be copied
  into the main history.

Invariants:

- a delta may only describe changes for one generation.
- `parent_snapshot_id` must identify the snapshot that the delta chain extends.
- replay from `snapshot + deltas` must reconstruct the same logical state as
  the live runtime.
- missing or unreadable artifact refs must fail closed during replay.

## Contract 3: Generation Rollover Policy

Purpose:

- bound history growth by cutting a new generation when compaction triggers

Rollover triggers:

- history length exceeds the configured threshold
- replay cost exceeds the acceptable recovery budget
- artifact fanout or payload size crosses the compaction budget
- the runtime requests a manual continue-as-new boundary

Required rollover outputs:

- a finalized snapshot for the old generation
- a new generation number
- a successor snapshot seed for the new generation
- an audit trail linking the new generation to the previous stable snapshot

Inheritance rules:

- the new generation inherits only the minimal necessary state.
- the new generation must keep session identity, job identity, and other
  durable control metadata required to continue safely.
- the new generation must not inherit full historical payload duplication.
- the old generation must remain readable for audit and recovery, but it must
  be treated as frozen once the rollover completes.
- every generation must have a single predecessor snapshot except the root
  generation.

Invariants:

- one rollover produces exactly one successor generation.
- generation numbers must be monotonic.
- the active execution path must always have a recoverable parent snapshot.
- no generation may orphan its predecessor chain.

## Contract 4: Artifact Ref Strategy

Purpose:

- keep large memory, compression, and tool-output objects out of the main
  history
- make recovery use indexed references instead of repeated inline copies

Required artifact ref fields:

- `artifact_id: string`
- `kind: string`
- `uri: string`
- `digest: string`
- `size_bytes: integer`
- `schema_version: string`
- `created_at: timestamp`
- `producer: string`

Strategy rules:

- artifact refs are the canonical pointer shape for large payloads.
- refs must be immutable once published.
- the snapshot should store artifact refs or an artifact index, not raw large
  payload blobs.
- artifact resolution must be deterministic and content-addressable or
  manifest-backed.
- the compaction index must let recovery find the latest resolvable artifact
  without scanning the entire event history.

Invariants:

- artifact refs must be sufficient to locate the payload later.
- refs may be compacted, but the resolved payload identity must not change.
- unresolved artifact refs must be treated as recovery failures.

## Contract 5: Consistency Invariants

Purpose:

- keep compaction safe under recovery, rollback, and replay

Required invariants:

- snapshot data, delta data, and durable state must agree on the active
  generation.
- replay must be deterministic given the same snapshot and same ordered deltas.
- the latest stable snapshot must be sufficient as the replay root.
- artifact refs must not introduce cross-generation mutable aliasing.
- compaction may reduce history size, but it must not remove information needed
  to reconstruct active execution state.
- every reachable generation must retain a chain back to the previous stable
  snapshot.
- if a field is not represented in the snapshot or delta contract, it must be
  treated as non-recoverable runtime-only state.

## Completion Criteria

- snapshot schema is explicit and versioned.
- delta replay is deterministic and generation-local.
- rollover preserves the minimum state needed to continue execution.
- large payloads are moved to artifact refs instead of duplicated in history.
- compacted recovery can be validated without a full historical JSONL scan.
