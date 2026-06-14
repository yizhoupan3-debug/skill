---
last_verified: "2026-06-14"
depends_on:
  - ../spec.md
---

# Performance Guide

## Hotspot Summary

| Hotspot | File | Issue | Severity |
|---------|------|-------|----------|
| Snapshot clone | `framework_profile.rs` | 65 clone/1692 lines | P0 |
| Error alloc | `execution_contract.rs` | 166 alloc/1121 lines | P0 |
| Regex compile | `hook_common.rs` | ~13 static Regex | P0 (✅ resolved) |
| Clone density | `status.rs` | 40 clone/265 lines = 15% | P1 |
| Alloc density | `routing.rs` | 122 alloc/1113 lines | P1 |
| Serde calls | `stdio_dispatch.rs` | 21 calls | P1 |

## Regex Caching Pattern

All static regex patterns MUST use `OnceLock<Regex>`:

```rust
static RE: OnceLock<Regex> = OnceLock::new();
RE.get_or_init(|| Regex::new(r"...").expect("..."))
```

## Clone Reduction Strategy

1. Prefer `&str` over `String` in function signatures.
2. Use `Arc<T>` for shared immutable data.
3. Use `Cow<'_, str>` for conditionally-owned strings.
4. Avoid `.clone()` chains — restructure to pass references.

## Build Profile

- Development: `cargo build` (debug, fast iteration)
- Release: `cargo build --release` (optimized, for deployment)
- Target dir: `/tmp/skill-cargo-target/` (via `.cargo/config.toml`)
