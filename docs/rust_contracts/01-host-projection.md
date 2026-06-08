---
last_verified: "2026-06-02"
depends_on:
  - ../host_adapter_contract.md
  - ../framework_profile_contract.md
---

# 宿主投影不变量 (Host Projection Invariants)

[返回索引](index.md)

## Host Projection Invariants

- The shared framework core is the profile authority; host projections are closed-set and explicit.
- Supported hosts are **exactly** the ids enumerated under **`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`**. `host_projections` is a narrower generated payload/projection set and must not be read as a second closed-set host registry.
- **Profile bundle vs host registry:** `build_profile_bundle` (`core/router-rs/src/framework_profile.rs`) derives `host_payloads` from `RUNTIME_REGISTRY.host_projections` while preserving legacy `codex_profile` / `full_codex_profile` artifacts for Codex consumers. Retired ids（`codex-cli`、`codex-app`、`claude-desktop`、`antigravity-cli`）不得再出现在 `host_targets.supported`。
- `codex_profile` is the Codex projection artifact and may carry Codex-private payload fields.
- Generated host projections are disposable install targets and must remain thin bootstrap pointers to the Rust core.
- `framework host-integration remove` removes only framework-owned projection files and manifest-recorded settings keys; user-authored files and unrelated settings are preserved.
- `framework host-integration compatibility-aliases` is the machine-readable inventory for retained aliases such as `install-skills`, `codex host-integration`, and `--repo-root`; each entry must include owner, reason, primary command, kept policy, removal condition, and `independent_behavior: false`.
- `configs/framework/host_projection_narrative.json` (schema `framework-host-projection-narrative-v2`, per-host `lifecycle_by_host`; JSON field `gsd_lifecycle_by_host` is a **serde alias** only) is the checked-in source for My lifecycle (`/discussx` → `/planx` → `/implementx` → `/verifyx`) and review findings-only paragraphs embedded in generated host framework entrypoints; `host_integration` must load it at install/render time instead of duplicating prose in Rust `const` strings.
- `configs/framework/RUNTIME_REGISTRY.json` is read at runtime for `review_gate` and related registry-backed hook policy through `runtime_registry` (`core/router-rs/src/runtime_registry/mod.rs`; `` is a re-export shim); steady-state must not rely on compile-time `include_str!` of the registry for those hot paths.
- `configs/framework/GENERATED_ARTIFACTS.json` declares checked-in generated artifacts with schema `framework-generated-artifacts-manifest-v1`. `framework host-integration generated-artifacts-status` supports two modes: **metadata-only** (`--skip-generator-run` or `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS=1`; used by `framework doctor` by default) checks existence, forbidden markers, undeclared paths, and per-artifact `clean` without running generators; **drift-gate** (default for maint `update-one-shot`) regenerates declared artifacts in an isolated temporary root, byte- or normalized-text-compares outputs, and reports drift. Slow generators (for example `host-integration install`) honor `ROUTER_RS_GENERATOR_TIMEOUT_SECONDS` (default 300s per generator invocation).
