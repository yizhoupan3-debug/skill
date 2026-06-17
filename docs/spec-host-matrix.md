---
parent: docs/spec.md
version: unified-v7
---

## 6. 跨宿主统一矩阵

> 权威真源：`configs/framework/RUNTIME_REGISTRY.json`

### 6.1 宿主闭集

权威真源：`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`（五 id 闭集）。

| 宿主 ID | install_tool | 运输模式 |
|---------|-------------|---------|
| `claude-code` | `claude` | `anthropic-claude-code` |
| `cursor` | `cursor` | `cursor-agent` |
| `codex` | `codex` | `native-codex` |
| `opencode` | `opencode` | `native-opencode` |
| `mimo` | `mimo` | `native-mimo` |

> **退役 id** 不在闭集内；迁移指引见 [`MIGRATION.md`](../MIGRATION.md)。

### 6.2 Hook 事件矩阵

| 事件 | claude-code | cursor | codex | opencode | mimo |
|------|:-----------:|:------:|:-----:|:--------:|:----:|
| PreToolUse | ✅ core | — | ✅ | ✅ ⁴ | ✅ ⁵ |
| UserPromptSubmit | ✅ core | ✅ ¹ | ✅ | ✅ ⁴ | ✅ ⁵ |
| PostToolUse | ✅ core | ✅ | ✅ | ✅ ⁴ | ✅ ⁵ |
| Stop | ✅ core | ✅ | ✅ | ✅ ⁴ | ✅ ⁵ |
| SessionStart | optional | ✅ | ✅ | ✅ ⁴ | ✅ ⁵ |
| SubagentStart | optional | ✅ | ✅ ² | ✅ ⁴ | ✅ ⁵ |
| SubagentStop | optional | ✅ | ✅ ² | ✅ ⁴ | ✅ ⁵ |

¹ `beforeSubmitPrompt` 映射 · ² v0.133.0+ · ⁴ 通过 `router-rs opencode agent` Rust 统一后端（JS 插件 bridge → `tool.execute.before/after` / `session.idle`） · ⁵ MiMo hook 统一后端

### 6.3 MCP 配置差异

| 宿主 | 顶层 key | transport | 配置文件 |
|------|----------|-----------|----------|
| claude-code | `mcpServers` | `stdio` | `~/.claude/mcp.json` |
| cursor | `mcpServers` | `stdio` | `~/.cursor/mcp.json` |
| opencode | `mcp` | `local` | `~/.config/opencode/opencode.json` |
| codex | `mcp_servers` (TOML) | `stdio` | `~/.codex/config.toml` |
| mimo | `mcpServers` | `stdio` | `~/.mimo/mcp.json` |

**§14.7 Schema Drift 三道闸**：写盘前 validate → 写盘后 readback → manifest 路径存在性

### 6.4 三档宿主能力 (v7)

| 档位 | 宿主 | capabilities | session_supervisor | harness_capabilities |
|------|------|-------------|-------------------|---------------------|
| **S 档** | codex | 11 | codex_driver | 4 项 |
| **A 档** | claude-code, cursor | 6 | mcp_bridge / unsupported | 4 项 |
| **B 档** | opencode, mimo | 5 | unsupported | 4 项 |

### 6.5 编译嵌入矩阵

| 宿主 | 嵌入内容 | 机制 |
|------|----------|------|
| claude-code | settings.json 模板 | `host_integration/projection` |
| cursor | hooks.json + .mdc | `host_integration/projection` |
| codex | AGENTS.md + AGENTS_CODEX.md | `policy_embed.rs` |
| opencode | opencode.json 投影 | `host_integration/projection` |
| mimo | mimo.json 投影 | `host_integration/projection` |

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

