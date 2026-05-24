# Framework Naming Conventions

## Env Var Naming Convention

All `ROUTER_RS_*` environment variables follow the pattern:

```
ROUTER_RS_{HOST}_{FEATURE}_{ACTION}
```

### Components

| Component | Description | Examples |
|-----------|-------------|----------|
| `HOST` | Target host identifier | `CLAUDE`, `CURSOR`, `CODEX` |
| `FEATURE` | Feature or subsystem name | `REVIEW_GATE`, `CONTINUITY`, `AUTOPILOT`, `RFV_LOOP` |
| `ACTION` | Action modifier (optional) | `DISABLE`, `ENABLE`, `MAX`, `MODE` |

### Host Identifiers

| Host | Env Var Prefix | Notes |
|------|---------------|-------|
| Claude Code | `ROUTER_RS_CLAUDE_*` | Main CLI agent |
| Claude Desktop | `ROUTER_RS_CLAUDE_*` | MCP-based desktop agent |
| Cursor | `ROUTER_RS_CURSOR_*` | Cursor IDE integration |
| Codex | `ROUTER_RS_CODEX_*` | OpenAI Codex |

**默认值与语义**：`ROUTER_RS_*` 的完整表见 [`harness_architecture.md`](harness_architecture.md) **§5 开关面**（唯一裁判）。本文件只定义命名模式，不维护第二份 env 默认值表。

场景子集与 closeout 分层见 [`references/AGENTS_OPERATOR_SURFACE.md`](references/AGENTS_OPERATOR_SURFACE.md)；可复制 profile 见 [`operator_profiles.md`](operator_profiles.md)。

---

## Artifact Path Conventions

### Framework Configs

Framework configuration files are located in `configs/framework/`:

```
configs/framework/
├── CLOSEOUT_RECORD_SCHEMA.json      # Closeout record schema
├── FRAMEWORK_SURFACE_POLICY.json     # Framework surface policy
├── GENERATED_ARTIFACTS.json          # Generated artifact registry
├── host_projection_narrative.json    # My lifecycle + review findings-only install copy
├── HARNESS_*.json                    # Harness configuration
├── RUNTIME_REGISTRY.json             # Runtime registry (disk-loaded by registry_loader)
├── RUNTIME_PROVIDER_REGISTRY.json    # Provider registry
├── NL_ROUTE_ADJUSTMENTS.json         # Natural language route adjustments
├── ROUTER_RS_HOOK_OBSERVATION_RULES.json
├── ROUTING_SIGNAL_MARKERS.json
└── *.schema.json                     # JSON schemas
```

### Skill Artifacts

Skill-related files are located in `skills/`:

```
skills/
├── SKILL_ROUTING_RUNTIME.json        # Hot routing entry point
├── SKILL_ROUTING_RUNTIME_EXPLAIN.json
├── SKILL_ROUTING_METADATA.json
├── SKILL_ROUTING_INDEX.md
├── SKILL_ROUTING_REGISTRY.md
├── SKILL_MANIFEST.json
├── SKILL_PLUGIN_CATALOG.json
├── SKILL_APPROVAL_POLICY.json
├── SKILL_HEALTH_MANIFEST.json
├── SKILL_SOURCE_MANIFEST.json
└── SKILL_*.md                        # Skill documentation
```

### Generated Artifact Tracking

`configs/framework/GENERATED_ARTIFACTS.json` tracks all checked-in generated artifacts with their generator commands.

**Inspection modes** (`framework host-integration generated-artifacts-status`):

- **metadata-only** — `--skip-generator-run` or `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS=1`; default for `framework doctor`
- **drift-gate** — full regeneration in a temp root; required for `framework maint update-one-shot`

See [`harness_architecture.md`](harness_architecture.md) §2.3.

**Generator sources:**
- `scripts/router-rs/Cargo.toml` — Rust router runtime (`framework skills validate|refresh`, `host-integration install`, `sync-entrypoints`)

---

## Backward Compatibility

### Legacy Env Var Aliases

When renaming env vars, maintain legacy aliases with deprecation warnings:

```rust
pub fn router_rs_claude_review_gate_disabled() -> bool {
    // New name takes precedence
    router_rs_env_enabled_default_false("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE")
        || router_rs_env_enabled_default_false("ROUTER_RS_REVIEW_GATE_DISABLE") // legacy
}
```

### Deprecation Warning Pattern

```rust
fn check_legacy_env_vars() {
    if std::env::var("ROUTER_RS_OLD_NAME").is_ok() {
        eprintln!("[router-rs] DEPRECATED: use ROUTER_RS_NEW_NAME instead");
    }
}
```

---

## Resolved Issues

### skill-compiler-rs Deletion (Phase 1)

**Status**: RESOLVED - `GENERATED_ARTIFACTS.json` updated to remove 10 entries referencing deleted `scripts/skill-compiler-rs/Cargo.toml`. Deprecated entries moved to `_deprecated_entries` array for audit trail.

`skill-compiler-rs` 删除后，下列路径仍由 [`GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json) **登记为 active 生成物**（`framework skills refresh` / `host-integration install` / `sync-entrypoints`）；**勿 hand-edit**，须用 generator 刷新：

- `skills/SKILL_ROUTING_*`、`SKILL_MANIFEST.json`、`SKILL_PLUGIN_CATALOG.json` 等（见 manifest 全文）
- `configs/framework/FRAMEWORK_SURFACE_POLICY.json`

`SKILL_ROUTING_METADATA.json` 在路由加载时由 `merge_sidecar_route_metadata_from_runtime` 合并进记录（非每 prompt 全量扫描，但影响 route 记录）。
