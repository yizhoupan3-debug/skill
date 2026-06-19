---
parent: docs/spec.md
version: unified-v7
---

## 6. 跨宿主统一矩阵

> 权威真源：`configs/framework/RUNTIME_REGISTRY.json`

### 6.1 宿主闭集

权威真源：`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`（四 id 闭集）。

| 宿主 ID | install_tool | 运输模式 |
|---------|-------------|---------|
| `claude` | `claude` | `anthropic-claude` |
| `cursor` | `cursor` | `cursor-agent` |
| `codex` | `codex` | `native-codex` |
| `opencode` | `opencode` | `native-opencode` |

> **退役 id** 不在闭集内；迁移指引见 [`MIGRATION.md`](../../MIGRATION.md)。

### 6.2 Hook 事件矩阵

| 事件 | claude | cursor | codex | opencode |
|------|:-----------:|:------:|:-----:|:--------:|
| PreToolUse | ✅ core | — | ✅ | ✅ ⁴ |
| UserPromptSubmit | ✅ core | ✅ ¹ | ✅ | ✅ ⁴ |
| PostToolUse | ✅ core | ✅ | ✅ | ✅ ⁴ |
| Stop | ✅ core | ✅ | ✅ | ✅ ⁴ |
| SessionStart | optional | ✅ | ✅ | ✅ ⁴ |
| SubagentStart | optional | ✅ | ✅ ² | ✅ ⁴ |
| SubagentStop | optional | ✅ | ✅ ² | ✅ ⁴ |

¹ `beforeSubmitPrompt` 映射 · ² v0.133.0+ · ⁴ 通过 `router-rs opencode agent` Rust 统一后端（JS 插件 bridge → `tool.execute.before/after` / `session.idle`）

### 6.3 MCP 配置差异

| 宿主 | 顶层 key | transport | 配置文件 |
|------|----------|-----------|----------|
| claude | `mcpServers` | `stdio` | `~/.claude/mcp.json` |
| cursor | `mcpServers` | `stdio` | `~/.cursor/mcp.json` |
| opencode | `mcp` | `local` | `~/.config/opencode/opencode.json` |
| codex | `mcp_servers` (TOML) | `stdio` | `~/.codex/config.toml` |

**§14.7 Schema Drift 三道闸**：写盘前 validate → 写盘后 readback → manifest 路径存在性

### 6.4 三档宿主能力 (v7)

| 档位 | 宿主 | capabilities | session_supervisor | harness_capabilities |
|------|------|-------------|-------------------|---------------------|
| **S 档** | codex | 11 | codex_driver | 4 项 |
| **A 档** | claude, cursor | 6 | mcp_bridge / unsupported | 4 项 |
| **B 档** | opencode | 5 | unsupported | 4 项 |

### 6.5 编译嵌入矩阵

| 宿主 | 嵌入内容 | 机制 |
|------|----------|------|
| claude | settings.json 模板 | `host_integration/projection` |
| cursor | hooks.json + .mdc | `host_integration/projection` |
| codex | AGENTS.md + AGENTS_CODEX.md | `policy_embed.rs` |
| opencode | opencode.json 投影 | `host_integration/projection` |

---

### 6.6 跨宿主去重架构（合并自 `docs/architecture/cross-host-dedup.md`）

> **Intent K4**: Eliminate duplicated code across host implementations.

#### 共享层

跨四宿主共享的代码在专用模块中：

| Layer | Module | Responsibility |
|-------|--------|---------------|
| Hook dispatch | `host-projection/hook_dispatch.rs` | Event normalization, prompt/tool extraction, subagent detection |
| State lock | `host-projection/file_state_lock.rs` | Atomic file-based state with flock |
| Review gate | `core-policy/review_gate_engine.rs` | Review gate logic (facts, independent reviewer detection) |
| Hook review state | `core-policy/hook_review_disk_core.rs` | Shared `HookReviewDiskCore` struct (cross-host compatible) |
| Crypto | `core-policy/crypto_util.rs` | `short_hash_for_session()`, `hex_lower()` |
| Session key | `core-policy/session_key.rs` | `extract_session_key()` |
| Hook common | `core-policy/hook_common.rs` | `normalize_tool_name()`, `saw_reject_reason()`, `has_override()` |

#### 去重决策

某模式出现在 2+ 宿主实现中时的三选一规则：

1. **Extracted to core-policy** — pure logic（无 IO、无宿主特定状态）
2. **Extracted to host-projection shared** — 需要 IO 或宿主上下文
3. **Left in host module** — 确属宿主特有行为

#### 子代理工具识别

共享层提供 `is_subagent_tool()`、`recognize_subagent_type()`、`subagent_lane_bits()`。宿主可通过覆盖（如 Codex 的 `saw_subagent_codex()`）扩展，但优先委托共享函数。

#### Review Gate 共享状态

所有宿主使用同一 `HookReviewDiskCore` 结构体：

```json
{
  "review_required": false,
  "review_override": false,
  "reject_reason_seen": false,
  "independent_reviewer_seen": false
}
```

宿主特定扩展（如 `subagent_start_count`、`review_phase`）通过 `#[serde(flatten)]` 在各宿主的 state struct 中添加。

### 6.7 宿主注册表规范 (v2)（合并自 `docs/architecture/host-registry.md`）

> **权威真源**: `configs/framework/RUNTIME_REGISTRY.json`
> **Schema version**: `framework-runtime-registry-v2`

#### Schema v2 结构

```jsonc
{
  "schema_version": "framework-runtime-registry-v2",
  "framework_core": {
    "authority": "rust",
    "host_policy": "closed-set-explicit-projections"
  },
  "host_targets": {
    "supported": ["cursor", "claude", "opencode", "codex"],
    "metadata": {
      "<host_id>": {
        "install_tool": "string",
        "projection_status": "implemented | experimental",
        "installable": true,
        "default_framework_command": "implementx",
        "host_entrypoints": "string | string[]",
        // v2 新增
        "display_name": "Human-readable name",
        "transport_type": "hook | native-opencode",
        "config_format": "json | toml | mdc",
        "config_path": ".<host>/settings.json",
        "home_env_var": "HOST_HOME",
        "default_home_dir": ".<host>"
      }
    },
    "host_providers": { /* Rust module bindings */ }
  },
  "host_projections": { /* Runtime configuration per host */ }
}
```

#### 新建宿主（非 registry 部分）

注册表条目之外还需：

1. **Rust provider**: `core/host-projection/src/hosts/<host>_provider.rs` — 实现 `HostLifecycle`、`HostTelemetry`、`HostProvider` traits
2. **Hook dispatcher**: `core/host-projection/src/hosts/<host>_hooks.rs` — 实现 `HostHookDispatcher` trait（7 event handlers）
3. **Hook launcher**: `configs/framework/<host>-router-rs-hook.sh`
4. **模块注册**: `core/host-projection/src/hosts/mod.rs`
5. **CLI 子命令**: `core/router-rs/src/`

不需要修改共享基础设施（`hook_dispatch.rs`、`core-policy`、`mcp_stdio_harness`）。

#### 关键不变量

- `host_targets.supported` 长度 == metadata 条目数 == host_providers 条目数
- 每个宿主必须有 `has_native_hook: true`（由 `host_provider.rs` 测试强制执行）
- `transport_type` 必须在 metadata 与 `host_projections` 中一致
- Schema version 在加载时严格校验（版本不匹配则 hard error）

---

## 7. 宿主接入契约

### 7.1 目标路径（3 文件）

| # | 文件 | 操作 |
|---|------|------|
| 1 | `configs/framework/RUNTIME_REGISTRY.json` | 注册宿主 id + 元数据 |
| 2 | `core/host-projection/src/hosts/<host>_hooks.rs` | 宿主 hook 实现 |
| 3 | `core/host-projection/src/hosts/<host>_hooks/` | 事件 handler 目录 |

### 7.2 Hook Trait 体系

真源：`core/host-projection/src/hosts/hook_dispatch.rs`

```rust
/// 宿主配置参数（host_id、state_dir、session namespace 等）
pub trait HostHookConfig: Send + Sync {
    fn host_id(&self) -> &'static str;
    fn state_dir_leaf(&self) -> &'static str;
    fn hook_state_unreadable_tag(&self) -> &'static str;
    fn session_namespace_env(&self) -> &'static str;
    fn log_label(&self) -> &'static str;
    fn additional_context_max_bytes(&self) -> usize { 640 }
    fn supports_session_start(&self) -> bool { false }
    fn supports_subagent_start(&self) -> bool { false }
    fn supports_subagent_stop(&self) -> bool { false }
}

/// 核心 trait：统一 hook 分发（ABC 三类方法）
pub trait HostHookDispatcher: HostHookConfig {
    // (C) Must implement:
    fn handle_pre_tool_use(&self, event: &HookEvent) -> Option<HookOutput>;
    fn handle_user_prompt_submit(&self, event: &HookEvent) -> Option<HookOutput>;
    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput>;
    // (B) Shared + extension（有默认实现，宿主可覆盖）:
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> { ... }
    fn handle_session_start(&self, event: &HookEvent) -> Option<HookOutput> { ... }
    fn handle_subagent_start/stop(...) { ... }
    // (A) Pure shared（无需覆盖）:
    fn canonical_event(&self, raw: &str) -> Option<String> { ... }
    fn critical_events(&self) -> &[&str] { ... }
}
```

另有 `HostProvider` 复合 trait（`HostLifecycle + HostToolExecutor + HostTelemetry`），位于 `host_provider.rs`。

### 7.3 接入 Checklist

- [ ] `RUNTIME_REGISTRY.json` — host_targets.supported + metadata
- [ ] `framework_host_targets.rs` — 只读注册表，fail-closed（位于 `core/framework-kernel/`）
- [ ] `hosts/<host>_hooks/` — 事件 handler
- [ ] `cli/dispatch.rs` — 子命令分发
- [ ] `host_integration/projection/` — install/status/remove + 三道闸
- [ ] `host_entrypoint_sync.rs` — provider trait（真源在 host-projection，runtime-core 重导出）
- [ ] 测试 + Schema 校验

### 7.4 硬编码耦合盘点

| 位置 | 内容 | 目标 |
|------|------|------|
| `framework_maint/mod.rs` | refresh_host_projections 遍历 | → registry 驱动 |
| `session_supervisor/mod.rs` | Codex driver only | → registry 标记 |
| `cursor_hooks/handlers/stop_closeout.rs` | hard_closeout 列表 | → registry 数据驱动 |

---

