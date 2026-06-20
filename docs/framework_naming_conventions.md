---
last_verified: "2026-06-19"
depends_on:
  - spec.md
---

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
| `FEATURE` | Feature or subsystem name | `REVIEW_GATE`, `CONTINUITY`, `PRE_GOAL`, `RFV_LOOP` |
| `ACTION` | Action modifier (optional) | `DISABLE`, `ENABLE`, `MAX`, `MODE` |

### Host Identifiers

| Host | Env Var Prefix | Notes |
|------|---------------|-------|
| Claude | `ROUTER_RS_CLAUDE_*` | Shell hook integration |
| Cursor | `ROUTER_RS_CURSOR_*` | Cursor IDE integration |
| Codex | `ROUTER_RS_CODEX_*` | OpenAI Codex |
| OpenCode | `ROUTER_RS_OPENCODE_*` | OpenCode MCP stdio |

**默认值与语义**：`ROUTER_RS_*` 的完整表见 [`spec.md`](spec.md)（唯一裁判）。本文件只定义命名模式，不维护第二份 env 默认值表。

### MCP Key Convention（**闭集，禁从一个 host 抄到另一个**）

> 与 `ROUTER_RS_*` env var 命名**正交**：env var 前缀按 host 走；MCP 顶层 key 按该 host **官方文档/源码**实测值走。**两套不能混。**
>
> MCP Key 矩阵与 host projection 校验见 [`spec.md`](spec.md) §7。本表只做"投影写盘 key 名"速查。

| host_id | MCP 配置文件 | **顶层 key（字面量）** | transport 字面量 | `managed_key_paths` 字面量 |
|---------|--------------|------------------------|------------------|----------------------------|
| `cursor` | `~/.cursor/mcp.json` | `mcp_servers`（**snake**，**不是** `mcpServers`） | `"stdio"` | `mcp_servers.router-rs-framework` · `mcp_servers.browser-mcp` · `mcp_servers.mcp-codegraph` · `mcp_servers.paperplain` |
| `claude` | 项目 `.mcp.json` | `mcpServers`（**camel**） | `"stdio"` | `mcpServers.router-rs-framework` · `mcpServers.browser-mcp` · `mcpServers.mcp-codegraph` · `mcpServers.paperplain` |
| `codex` | 项目 `.codex/config.toml` | `mcp_servers`（**snake**，**TOML 表**） | 无 `type`（`command` 隐式） | `mcp_servers.router-rs-framework` · `mcp_servers.browser-mcp` · `mcp_servers.mcp-codegraph` · `mcp_servers.paperplain` |
| `opencode` | `~/.config/opencode/opencode.json` | `mcpServers`（**camel**） | `"local"` | `mcpServers.router-rs-framework` · `mcpServers.browser-mcp` · `mcpServers.mcp-codegraph` · `mcpServers.paperplain` |

**三个最常踩的坑**：

1. **snake_case vs camelCase**：TOML (codex) 和 Cursor JSON 用 `mcp_servers`；JSON (claude / opencode) 用 `mcpServers`。
2. **Cursor 是 snake_case JSON**：Cursor 用 `mcp_servers` 而非 `mcpServers`，与其他 JSON 宿主不同。
3. **transport 字段**：opencode 用 `"local"`；其余 host 用 `"stdio"`（或隐式）。

**改动任何 host projection 前必读** [`spec.md`](spec.md) §7 与宿主手册 [`hosts/_common.md`](hosts/_common.md) + [`hosts/hook-hosts.md`](hosts/hook-hosts.md) / [`hosts/opencode.md`](hosts/opencode.md)。

closeout 分层见 [`spec.md`](spec.md) §12。

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
├── RUNTIME_REGISTRY.json             # Runtime registry (disk-loaded by runtime_registry/mod.rs)
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
├── SKILL_MANIFEST.json
├── SKILL_TIERS.json
├── SKILL_SOURCE_MANIFEST.json
└── SKILL_*.md                        # Skill documentation
```

### Generated Artifact Tracking

`configs/framework/GENERATED_ARTIFACTS.json` tracks all checked-in generated artifacts with their generator commands.

**Inspection modes** (`framework host-integration generated-artifacts-status`):

- **metadata-only** — `--skip-generator-run` or `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS=1`; default for `framework doctor`
- **drift-gate** — full regeneration in a temp root; required for `framework maint update-one-shot`

See [`spec.md`](spec.md) §13.5.

**Generator sources:**
- `core/router-rs/Cargo.toml` — Rust router runtime (`framework skills validate|refresh`, `host-integration install`, `sync-entrypoints`)

---

## Backward Compatibility

### Legacy Env Var Aliases

When renaming env vars, maintain legacy aliases with deprecation warnings:

```rust
// Current shared implementation (hook_dispatch.rs)
pub fn is_review_gate_suppressed(host_id: &str, repo_root: Option<&Path>, prompt: &str) -> bool {
    core_policy::env_flags::router_rs_review_gate_disabled_for_host(host_id)
        || core_policy::hook_common::review_gate_hard_block_disabled(repo_root, prompt)
}
// Per-host env vars (ROUTER_RS_CURSOR_*, ROUTER_RS_CLAUDE_*, etc.) are
// still honored by router_rs_review_gate_disabled_for_host() for backward compat.
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

## Shared Code Naming Convention（公用代码命名规范）

### 核心原则

**公用模块中的函数名、常量名、类型名禁止包含宿主名称**（`cursor`/`codex`/`opencode`/`claude`）。

### 规则

| 场景 | 规则 | 示例 |
|------|------|------|
| 公用函数 | 通用语义命名 | `extract_prompt_text` ✅，`cursor_prompt_text` ❌ |
| 公用常量 | 通用语义命名 | `HOOK_SIGNAL_ASSISTANT_TAIL_CHARS` ✅，`CURSOR_HOOK_SIGNAL_*` ❌ |
| 公用类型 | 通用语义命名 | `LockConfig::long_timeout()` ✅，`LockConfig::cursor()` ❌ |
| 环境变量 | 保留宿主前缀（运维合约） | `ROUTER_RS_CURSOR_HOOK_SILENT` ✅（已发布，不可改） |
| 宿主适配层 | 允许宿主前缀 | `recognized_subagent_kind` ✅（宿主适配层内，无宿主前缀） |

### 中立配置目录

| 目录 | 内容 | 宿主目录 symlink 指向 |
|---|---|---|
| `.commands/` | 共享 slash 命令定义 | `.cursor/commands/` → `.commands/`，`.opencode/commands/` → `.commands/` |
| `.rules/` | 共享规则文件（`.mdc`） | `.cursor/rules/` → `.rules/` |

### 公用模块清单

| 层 | 路径 |
|---|---|
| 策略层 | `core/core-policy/src/hook_common.rs` |
| 策略层 | `core/core-policy/src/env_flags.rs` |
| 分发层 | `core/host-projection/src/hosts/hook_dispatch.rs` |
| 状态层 | `core/host-projection/src/hosts/hook_state_common.rs` |
| 锁层 | `core/host-projection/src/hosts/file_state_lock.rs` |
| Provider 层 | `core/host-projection/src/hosts/host_provider.rs` |
| 启动器 | `configs/framework/hook.sh` |

---

## Resolved Issues

### skill-compiler-rs Deletion (Phase 1)

**Status**: RESOLVED - `GENERATED_ARTIFACTS.json` updated to remove 10 entries referencing deleted `scripts/skill-compiler-rs/Cargo.toml`. Deprecated entries moved to `_deprecated_entries` array for audit trail.

`skill-compiler-rs` 删除后，**手维护热路由**为 `skills/SKILL_ROUTING_RUNTIME.json` 与 `skills/SKILL_MANIFEST.json`（见 [`SKILL_MAINTENANCE_GUIDE.md`](../skills/SKILL_MAINTENANCE_GUIDE.md)）。2026-06 已清理的空壳 companion（`SKILL_ROUTING_RUNTIME_EXPLAIN.json`、`SKILL_ROUTING_METADATA.json`、`SKILL_PLUGIN_CATALOG.json`、`SKILL_HEALTH_MANIFEST.json` 等）**勿**再手改或恢复；路由 metadata 真源为 `SKILL_ROUTING_RUNTIME.json` 顶层 `default_host_platforms` 与各 skill 行。其余生成物见 [`GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json)（如 `FRAMEWORK_SURFACE_POLICY.json`）。

**重要说明**：上述空壳 companion 文件是存根文件，仅用于向后兼容，不应被修改或恢复。所有实际的路由配置和元数据都应存储在 `SKILL_ROUTING_RUNTIME.json` 中。
