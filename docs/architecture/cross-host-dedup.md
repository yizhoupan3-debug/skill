# Cross-Host Deduplication Architecture

> **Intent K4**: Eliminate duplicated code across host implementations.

## Shared Layers

Code shared across all four hosts lives in dedicated crates:

| Layer | Crate | Responsibility |
|-------|-------|---------------|
| Hook dispatch | `host-projection/hook_dispatch.rs` | Event normalization, prompt/tool extraction, subagent detection |
| State lock | `host-projection/file_state_lock.rs` | Atomic file-based state with flock |
| Review gate | `core-policy/review_gate_engine.rs` | Review gate logic (facts, independent reviewer detection) |
| Hook review state | `core-policy/hook_review_disk_core.rs` | Shared `HookReviewDiskCore` struct (cross-host compatible) |
| Crypto | `core-policy/crypto_util.rs` | `short_hash_for_session()`, `hex_lower()` |
| Session key | `core-policy/session_key.rs` | `extract_session_key()` |
| Hook common | `core-policy/hook_common.rs` | `normalize_tool_name()`, `saw_reject_reason()`, `has_override()` |

## Dedup Decision Record

When a pattern appears in 2+ host implementations, it should be:

1. **Extracted to core-policy** if it's pure logic (no IO, no host-specific state)
2. **Extracted to host-projection shared** if it needs IO or host context
3. **Left in host module** if it's genuinely host-specific behavior

## Subagent Tool Recognition

Each host has different tool names for subagents. The shared layer provides:

- `is_subagent_tool(normalized_name)` — checks against a shared registry
- `recognize_subagent_type(tool_input)` — extracts subagent type from tool input
- `subagent_lane_bits(kind)` — returns (review_lane, parallel_lane) booleans

Host-specific overrides (e.g., Codex's `saw_subagent_codex()`) are kept in host modules
but delegate to the shared functions where possible.

## Review Gate State

All hosts use the same `HookReviewDiskCore` struct for on-disk state:

```json
{
  "review_required": false,
  "review_override": false,
  "reject_reason_seen": false,
  "independent_reviewer_seen": false
}
```

Host-specific extensions (e.g., `subagent_start_count`, `review_phase`) are added
via `#[serde(flatten)]` in each host's state struct.
