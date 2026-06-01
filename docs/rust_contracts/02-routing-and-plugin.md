---
last_verified: "2026-06-02"
depends_on:
  - ../harness_architecture/index.md
  - ../host_adapter_contract.md
---

# 路由契约与插件 ABI (Route & Plugin Contract)

[返回索引](index.md)

## Route Contract

Required route result fields:

- `task`
- `session_id`
- `selected_skill`
- `overlay_skill`
- `layer`
- `score`
- `reasons`
- `prompt_preview`
- `route_engine`
- `diagnostic_route_mode`
- `route_diagnostic_report`

Invariants:

- exactly one primary owner
- at most one overlay
- `route_engine` and primary authority stay Rust
- unknown selected skills fail closed in consumers
- fallback selection may choose a safe owner from `SKILL_MANIFEST.json`, but must not introduce a second route authority
- generated framework command aliases must name an existing manifest owner as `canonical_owner`; deleted historical owners must not appear in steady-state registry or routing

## Plugin and Routing Contract

This section freezes the skill runtime plugin ABI (V1): Rust keeps control-plane authority while skills, storage backends, and routing policies evolve as declarative records.

### Core rules

- **Control-plane authority**: The Rust runtime remains the control-plane authority.
- **Declarative records**: Plugin records are declarative data structures, not dynamically loaded executable binaries.
- **Fail-closed capability classes**: Unknown capability classes must fail closed.
- **Hot/cold split**: `SKILL_ROUTING_RUNTIME.json` stays a minimal hot index; full metadata and explain data live in cold catalogs.

### Plugin ABI (V1)

Each plugin declaration record must provide:

- `slug`: stable unique identifier
- `kind`: e.g. `skill`, `framework_command`
- `skill_path`: repo-relative skill path
- `entrypoint`: runtime entrypoint class
- `capabilities`: declared routing, tool, artifact, network, and gate boundaries
- `risk`: priority, review policy, and destructive-risk projection

Capability validation uses a closed-set mapping:

- `routing_layer` → `routing`
- `routing_owner` → `routing_owner`
- `routing_gate` → `routing_gate`
- `allowed_tools` → `tool`
- `approval_required_tools` → `high_risk`
- `artifact_outputs` → `artifact`
- `network_access` → `networked`

Unknown mappings or capability declarations fail closed during static self-check.

### Routing metadata and catalogs

Routing metadata is declarative and includes:

- `intent_tags`: normalized semantic tags
- `positive_triggers` / `negative_triggers`: confidence boost/suppress signals
- `gate_policy` / `overlay_policy` / `fallback_policy`: scheduling gates and fallback rules

Catalog locations:

- **Hot routing index**: `skills/SKILL_ROUTING_RUNTIME.json` (schema id + minimal skill index only; no explain prose)
- **Cold plugin catalog**: `skills/SKILL_PLUGIN_CATALOG.json`
- **Cold routing companion metadata**: `skills/SKILL_ROUTING_METADATA.json`
- **Routing explain**: `skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`
- **Health manifest**: `skills/SKILL_HEALTH_MANIFEST.json`
- **Provider registry**: `configs/framework/RUNTIME_PROVIDER_REGISTRY.json`

Invariants:

- The Rust runtime remains the control-plane authority.
- Unknown capability classes must fail closed.
- `SKILL_ROUTING_RUNTIME.json` stays a minimal hot index.

## Runtime Control Contracts

Runtime control-plane payloads must keep these owner markers stable:

- `rust-route-core`
- `rust-route-compiler`
- `rust-runtime-control-plane`
- `rust-runtime-storage`
- `rust-runtime-trace-io`
- `rust-framework-runtime-read-model`
- `rust-framework-session-artifact-writer`
- `rust-framework-prompt-policy`
