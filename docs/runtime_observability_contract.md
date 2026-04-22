# Runtime Observability Contract

## Purpose

This document freezes the OpenTelemetry-ready observability vocabulary for the
runtime migration workstream.

It is the contract source of truth for:

- JSONL event sinks
- OTel exporters
- runtime dashboards
- trace-to-metric aggregation rules

## Contract Rules

- Contract changes must be explicit and versioned.
- JSONL and OTel exports must share the same canonical runtime vocabulary.
- Resource attributes are process-scoped and must be stable for the lifetime of
  one runtime instance.
- Derived dashboard rates may be computed from raw counters and histograms, but
  the raw metric names must remain stable.
- No signal may depend on unbounded high-cardinality labels beyond the shared
  runtime dimensions defined below.

## Producer / Exporter Ownership

The runtime observability producer and exporter are Rust contract-owned.

- Python remains a thin projection for bridge delivery and persistence
  plumbing; it does not own the canonical observability vocabulary.
- The ownership boundary is expressed with
  `ownership_lane = "rust-contract-lane"`,
  `producer_owner = "rust-control-plane"`, and
  `exporter_owner = "rust-control-plane"`.
- `producer_authority` and `exporter_authority` remain rooted in
  `rust-runtime-control-plane`.
- This ownership shift does not change the JSONL vocabulary, the OTel
  vocabulary, the replay seam, or the compaction seam.

## Runtime Resource Attributes

All spans, metrics, and logs emitted by the runtime carry the same resource
envelope.

| name | type | required | meaning |
| --- | --- | --- | --- |
| `service.name` | string | yes | Canonical service name for the runtime process. |
| `service.version` | string | yes | Build or release identifier for the runtime process. |
| `runtime.instance.id` | string | yes | Unique identifier for one runtime process instance. |
| `route_engine_mode` | string | yes | Current route engine mode used by the runtime. |

Invariants:

- `service.name`, `service.version`, `runtime.instance.id`, and
  `route_engine_mode` must be present on every exported signal.
- `runtime.instance.id` must stay stable for the full process lifetime.
- `route_engine_mode` must be copied into all spans, metrics, and logs without
  renaming.
- Resource attributes must never be derived from mutable per-event payload data.

## Shared Span / Metric / Log Fields

The runtime signal envelope uses the same shared dimensions across spans,
metrics, and logs.

| field | applies to | required | notes |
| --- | --- | --- | --- |
| `runtime.job_id` | span / metric / log | yes | Correlates all telemetry for one job. |
| `runtime.session_id` | span / metric / log | yes | Correlates telemetry across retries and reroutes. |
| `runtime.attempt` | span / metric / log | yes | Monotonic attempt number for the job or session. |
| `runtime.worker_id` | span / metric / log | yes | Worker identity for distributed execution. |
| `runtime.generation` | span / metric / log | yes | Generation or rollout cohort identifier. |
| `runtime.schema_version` | span / metric / log | yes | Version of the signal envelope itself. |
| `runtime.event_id` | span / log | yes | Unique event identity for reconstructable history. |
| `runtime.stage` | span / metric / log | yes | Pipeline stage or lifecycle phase. |
| `runtime.kind` | span / log | yes | Canonical event vocabulary shared with JSONL. |
| `runtime.status` | span / metric / log | yes | Terminal or intermediate state of the emitted event. |
| `trace_id` | span / log | when available | Preserves cross-signal correlation for one trace. |
| `span_id` | span / log | when available | Preserves parent/child linkage inside a trace. |

Invariants:

- Every signal must include the shared runtime dimensions listed above.
- Metrics may omit `trace_id` and `span_id` when the OTel representation does
  not support them, but the runtime dimensions remain mandatory.
- `runtime.kind` must reuse the same vocabulary as the JSONL sink.
- `runtime.schema_version` must change only when the contract changes.

## JSONL <-> OTel Vocabulary Map

The JSONL sink and the OTel exporter must use a single canonical vocabulary.

| JSONL token | OTel target | signal types | rule |
| --- | --- | --- | --- |
| `ts` | `time_unix_nano` | span / metric / log | Timestamp must be serialized as UTC nanoseconds. |
| `event_id` | `runtime.event.id` | span / log | Unique event identifier, never reused. |
| `seq` | `runtime.event.seq` | span / log | Monotonic per-generation replay order. |
| `cursor` | `runtime.resume.cursor` | span / log | Stable resume pointer for replay windows and stream resubscribe. |
| `kind` | `runtime.kind` | span / log | Same vocabulary across sinks, no renaming. |
| `stage` | `runtime.stage` | span / metric / log | Stable pipeline stage label. |
| `status` | `runtime.status` | span / metric / log | Preserve terminal and intermediate states. |
| `payload` | `attributes` | span / log | JSON object must be flattened into attributes, not stringified. |
| `service_name` | `service.name` | span / metric / log | Resource attribute alias only. |
| `service_version` | `service.version` | span / metric / log | Resource attribute alias only. |
| `runtime_instance_id` | `runtime.instance.id` | span / metric / log | Resource attribute alias only. |
| `route_engine_mode` | `route_engine_mode` | span / metric / log | Resource attribute alias only. |
| `job_id` | `runtime.job_id` | span / metric / log | Shared correlation dimension. |
| `session_id` | `runtime.session_id` | span / metric / log | Shared correlation dimension. |
| `attempt` | `runtime.attempt` | span / metric / log | Shared correlation dimension. |
| `worker_id` | `runtime.worker_id` | span / metric / log | Shared correlation dimension. |
| `generation` | `runtime.generation` | span / metric / log | Shared correlation dimension. |
| `schema_version` | `runtime.schema_version` | span / metric / log | Envelope version marker. |

Vocabulary invariants:

- Every JSONL token in this table must map to exactly one canonical OTel target.
- The canonical OTel targets above must not be repurposed for unrelated fields.
- The JSONL sink may add new tokens only as additive extensions that keep the
  existing table stable.
- `seq` and `cursor` together define the first resumable replay seam even when
  the runtime has not yet exposed a live SSE bridge.
- The current runtime does expose an in-memory bridge for local
  producer/consumer delivery; the missing piece is an external SSE-grade host
  transport, not the existence of any bridge at all.
- The current runtime also exposes a handoff descriptor plus persisted binding
  artifact for replay-capable attach; this is a recovery/control-plane seam,
  not a claim that external SSE delivery already exists.

## Runtime Metrics Catalog

Core metrics are exported as raw counters or histograms so that dashboards can
derive rates and percentiles without changing the underlying contract.

| intent | metric name | type | unit | base dimensions | dashboard derivation |
| --- | --- | --- | --- | --- | --- |
| route mismatch rate | `runtime.route_mismatch_total` | counter | `1` | `service.name`, `service.version`, `runtime.instance.id`, `route_engine_mode`, `runtime.job_id`, `runtime.session_id`, `runtime.attempt`, `runtime.worker_id`, `runtime.generation` | `rate(route_mismatch_total) / rate(route_evaluation_total)` |
| replay resume success rate | `runtime.replay_resume_success_total` | counter | `1` | `service.name`, `service.version`, `runtime.instance.id`, `route_engine_mode`, `runtime.job_id`, `runtime.session_id`, `runtime.attempt`, `runtime.worker_id`, `runtime.generation` | `rate(replay_resume_success_total) / rate(replay_resume_attempt_total)` |
| lease takeover latency | `runtime.lease_takeover_latency_ms` | histogram | `ms` | `service.name`, `service.version`, `runtime.instance.id`, `route_engine_mode`, `runtime.job_id`, `runtime.session_id`, `runtime.attempt`, `runtime.worker_id`, `runtime.generation` | `p50 / p95 / p99` |
| interrupt completion latency | `runtime.interrupt_completion_latency_ms` | histogram | `ms` | `service.name`, `service.version`, `runtime.instance.id`, `route_engine_mode`, `runtime.job_id`, `runtime.session_id`, `runtime.attempt`, `runtime.worker_id`, `runtime.generation` | `p50 / p95 / p99` |
| compression offload rate | `runtime.compression_offload_total` | counter | `1` | `service.name`, `service.version`, `runtime.instance.id`, `route_engine_mode`, `runtime.job_id`, `runtime.session_id`, `runtime.attempt`, `runtime.worker_id`, `runtime.generation` | `rate(compression_offload_total) / rate(compression_candidate_total)` |
| sandbox timeout rate | `runtime.sandbox_timeout_total` | counter | `1` | `service.name`, `service.version`, `runtime.instance.id`, `route_engine_mode`, `runtime.job_id`, `runtime.session_id`, `runtime.attempt`, `runtime.worker_id`, `runtime.generation` | `rate(sandbox_timeout_total) / rate(sandbox_execution_total)` |

Metric invariants:

- Every metric in this catalog must carry the base dimensions listed in the
  table.
- Rate metrics are derived from counters and must not be stored as a separate
  mutable state machine.
- Latency metrics must be recorded as histograms with millisecond units.

## Concrete Exporter Path

The runtime now exposes a concrete exporter and metric-record helper surface at
`codex_agno_runtime.observability`.

- Public helper calls should delegate to `router-rs` whenever the repo-local
  Rust lane is available; Python stays as a thin projection and compatibility
  fallback only.
- `build_runtime_observability_exporter_descriptor()` freezes the Rust-owned
  exporter lane and ties it to the JSONL sink plus replay/handoff schema
  versions used by the trace lane.
- `runtime_observability_metric_catalog()` returns the machine-readable metric
  catalog frozen by this contract, including each metric name, type, unit,
  dimensions, and dashboard derivation.
- `build_runtime_metric_record()` emits one versioned metric payload using only
  cataloged metric names and the shared base dimensions above.
- `runtime_observability_dashboard_schema()` returns the machine-readable
  dashboard payload that mirrors the JSON block below.
- `build_runtime_observability_health_snapshot()` exposes the same Rust-owned
  contract through the runtime health surface, and
  `CodexAgnoRuntime.health()["trace"]["observability"]` is the steady-state
  projection.

Concrete path invariants:

- exporter helpers must stay inside the frozen JSONL / OTel vocabulary
  contract.
- metric helpers may emit only cataloged metric names.
- unknown metric names must fail closed with an explicit error instead of
  silently emitting ad hoc payloads.
- required resource and shared runtime dimensions must reject empty strings, and
  metric values must reject `NaN` / infinity payloads.
- `runtime.attempt` must stay a non-negative integer, not a float, bool, or
  negative retry marker.
- helper payloads must not introduce host-private or high-cardinality
  dimensions outside the base dimensions table.
- the machine-readable metric catalog must stay additive-only and must not
  redefine existing metric names, units, or dimension sets.

## Dashboard Schema

The dashboard schema is intentionally explicit so the same JSON can back a
Grafana-like board, a managed cloud dashboard, or a local visualization layer.

```json
{
  "schema_version": "runtime-observability-dashboard-v1",
  "title": "Runtime Observability",
  "resource_dimensions": [
    "service.name",
    "service.version",
    "runtime.instance.id",
    "route_engine_mode",
    "runtime.job_id",
    "runtime.session_id",
    "runtime.attempt",
    "runtime.worker_id",
    "runtime.generation"
  ],
  "panels": [
    {
      "name": "Route mismatch rate",
      "metric": "runtime.route_mismatch_total",
      "visualization": "timeseries",
      "group_by": [
        "service.name",
        "service.version",
        "route_engine_mode"
      ]
    },
    {
      "name": "Replay resume success rate",
      "metric": "runtime.replay_resume_success_total",
      "visualization": "timeseries",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.session_id"
      ]
    },
    {
      "name": "Lease takeover latency",
      "metric": "runtime.lease_takeover_latency_ms",
      "visualization": "histogram",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.worker_id"
      ]
    },
    {
      "name": "Interrupt completion latency",
      "metric": "runtime.interrupt_completion_latency_ms",
      "visualization": "histogram",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.session_id"
      ]
    },
    {
      "name": "Compression offload rate",
      "metric": "runtime.compression_offload_total",
      "visualization": "timeseries",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.generation"
      ]
    },
    {
      "name": "Sandbox timeout rate",
      "metric": "runtime.sandbox_timeout_total",
      "visualization": "timeseries",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.worker_id"
      ]
    }
  ],
  "alerts": [
    {
      "name": "route-mismatch-burst",
      "metric": "runtime.route_mismatch_total",
      "severity": "warning"
    },
    {
      "name": "lease-takeover-latency-regression",
      "metric": "runtime.lease_takeover_latency_ms",
      "severity": "critical"
    },
    {
      "name": "sandbox-timeout-spike",
      "metric": "runtime.sandbox_timeout_total",
      "severity": "warning"
    }
  ]
}
```

Dashboard invariants:

- Every panel must reference one of the cataloged metrics above.
- Dashboard group-by dimensions must come from the shared runtime dimensions or
  the resource dimensions.
- Alerts must only reference cataloged metrics so the dashboard cannot drift to
  ad hoc names.
- `schema_version` must stay stable until the observable contract changes.

Current implementation note:

- an in-memory bridge exists for `subscribe` / `resume` / `heartbeat` / `cleanup`
- external SSE bridge not yet exposed
