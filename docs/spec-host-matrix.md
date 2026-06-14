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
| `claude-code` | `claude` | `anthropic-claude-code` |
| `cursor` | `cursor` | `cursor-agent` |
| `codex` | `codex` | `native-codex` |
| `opencode` | `opencode` | `opencode-plugin` |

> **退役 id** 不在闭集内；迁移指引见 [`MIGRATION.md`](../MIGRATION.md)。

### 6.2 Hook 事件矩阵

| 事件 | claude-code | cursor | codex | opencode |
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
| claude-code | `mcpServers` | `stdio` | `~/.claude/mcp.json` |
| cursor | `mcpServers` | `stdio` | `~/.cursor/mcp.json` |
| opencode | `mcp` | `local` | `~/.config/opencode/opencode.json` |
| codex | `mcp_servers` (TOML) | `stdio` | `~/.codex/config.toml` |

**§14.7 Schema Drift 三道闸**：写盘前 validate → 写盘后 readback → manifest 路径存在性

### 6.4 三档宿主能力 (v7)

| 档位 | 宿主 | capabilities | session_supervisor | harness_capabilities |
|------|------|-------------|-------------------|---------------------|
| **S 档** | codex | 11 | codex_driver | 4 项 |
| **A 档** | claude-code, cursor | 6 | mcp_bridge / unsupported | 4 项 |
| **B 档** | opencode | 5 | unsupported | 4 项 |

### 6.5 编译嵌入矩阵

| 宿主 | 嵌入内容 | 机制 |
|------|----------|------|
| claude-code | settings.json 模板 | `host_integration/projection` |
| cursor | hooks.json + .mdc | `host_integration/projection` |
| codex | AGENTS.md + AGENTS_CODEX.md | `policy_embed.rs` |
| opencode | opencode.json 投影 | `host_integration/projection` |

---

## 7. 宿主接入契约

### 7.1 目标路径（3 文件）

| # | 文件 | 操作 |
|---|------|------|
| 1 | `configs/framework/RUNTIME_REGISTRY.json` | 注册宿主 id + 元数据 |
| 2 | `core/host-projection/src/hosts/<host>_hooks.rs` | 宿主 hook 实现 |
| 3 | `core/host-projection/src/hosts/<host>_hooks/` | 事件 handler 目录 |

### 7.2 HostHook trait

```rust
pub trait HostHook {
    fn host_id(&self) -> &str;
    fn canonical_event(&self, raw: &str) -> Option<String>;
    fn critical_events(&self) -> &[&str];
    fn handle_pre_tool_use(&self, ctx: &HookContext) -> HookResult;
    fn handle_post_tool_use(&self, ctx: &HookContext) -> HookResult;
    fn handle_stop(&self, ctx: &HookContext) -> HookResult;
    fn handle_user_prompt_submit(&self, ctx: &HookContext) -> HookResult;
    fn handle_custom_event(&self, event: &str, ctx: &HookContext) -> Option<HookResult> { None }
}
```

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
| `mcp_common/host.rs` | hard_closeout 列表 | → registry 数据驱动 |

---

