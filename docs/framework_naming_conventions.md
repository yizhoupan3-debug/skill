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

### Known Env Vars

#### Claude/Cursor Shared

| Env Var | Default | Description |
|---------|---------|-------------|
| `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` | 1 | Append PostTool evidence to EVIDENCE_INDEX |
| `ROUTER_RS_CONTINUITY_STOP_CHECKPOINT` | 1 | Write checkpoint on Stop |
| `ROUTER_RS_CLOSEOUT_ENFORCEMENT` | soft | Hard/soft closeout enforcement |
| `ROUTER_RS_DEPTH_SCORE_MODE` | off | Depth scoring mode |

#### Cursor-specific

| Env Var | Default | Description |
|---------|---------|-------------|
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` | false | Disable review gate |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED` | false | Enable pre-goal autopilot |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES` | 8 | Max autopilot nudges |
| `ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS` | 5 | Max concurrent subagents |
| `ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS` | 300 | Stale subagent threshold |
| `ROUTER_RS_CURSOR_KILL_STALE_TERMINALS` | true | Kill stale terminals on session end |
| `ROUTER_RS_CURSOR_TERMINAL_KILL_MODE` | default | Terminal kill mode |
| `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE` | - | Session close style |
| `ROUTER_RS_CURSOR_HOOK_STATE_FULL_SWEEP_ENABLE` | false | Full state sweep |

#### Claude-specific

| Env Var | Default | Description |
|---------|---------|-------------|
| `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` | false | Disable review gate |
| `ROUTER_RS_CLAUDE_SESSION_NAMESPACE` | - | Session namespace |

#### Codex-specific

| Env Var | Default | Description |
|---------|---------|-------------|
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` | 640 | Max SessionStart context (256-8192) |

---

## Artifact Path Conventions

### Framework Configs

Framework configuration files are located in `configs/framework/`:

```
configs/framework/
├── CLOSEOUT_RECORD_SCHEMA.json      # Closeout record schema
├── FRAMEWORK_SURFACE_POLICY.json     # Framework surface policy
├── GENERATED_ARTIFACTS.json          # Generated artifact registry
├── HARNESS_*.json                    # Harness configuration
├── RUNTIME_REGISTRY.json             # Runtime registry
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

`configs/framework/GENERATED_ARTIFACTS.json` tracks all generated artifacts with their generator commands.

**Generator sources:**
- `scripts/router-rs/Cargo.toml` — Rust router runtime (`framework skills validate|refresh`)

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

The following artifacts are now tracked as deprecated in `GENERATED_ARTIFACTS.json`:

- `skills/SKILL_ROUTING_REGISTRY.md`
- `skills/SKILL_ROUTING_INDEX.md`
- `skills/SKILL_MANIFEST.json`
- `skills/SKILL_ROUTING_RUNTIME.json`
- `skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`
- `skills/SKILL_PLUGIN_CATALOG.json`
- `skills/SKILL_ROUTING_METADATA.json`
- `skills/SKILL_HEALTH_MANIFEST.json`
- `skills/SKILL_APPROVAL_POLICY.json`
- `configs/framework/FRAMEWORK_SURFACE_POLICY.json`

Active artifacts (5 entries) now only reference `scripts/router-rs/Cargo.toml`.
