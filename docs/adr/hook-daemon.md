---
last_verified: "2026-06-02"
depends_on: []
---

# ADR: Warm hook daemon (deferred)

| Status | **Rejected / not implemented** (2026-05-20) |
|--------|---------------------------------------------|
| Context | Each Cursor hook spawns a new `router-rs` process (~5–80ms cold start). |
| Decision | Keep **one event → one process** until p95 targets are met without daemon complexity. |
| Consequences | No long-lived RPC server to deploy, patch, or crash-recover. |
| Revisit when | p50 still > 80ms after W1–W3 optimizations **and** product accepts new deployment surface. |
| Rollback | N/A — not shipped. |

See `docs/hook_lock_order.md` and task `hook-perf-deadlock`.
