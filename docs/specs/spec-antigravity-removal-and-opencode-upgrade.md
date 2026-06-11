# Spec: Antigravity 移除 + 四宿主一致性 + 注册表驱动

> **日期**: 2026-06-12
> **版本**: v2 (深度审计后)
> **范围**: host-projection / runtime-core / framework-kernel / configs / docs / policy / skills

---

## 0. 背景与动机

### 0.1 问题

当前闭集宿主 5 个（`codex`、`claude-code`、`cursor`、`opencode`、`antigravity`），其中：

- **Antigravity** 产品线已停用，残留 30+ 文件引用，增加维护负担
- **OpenCode** 被错误标记为"anemic host"，实际拥有完整插件 hook 系统
- **硬编码泛滥**：添加/删除宿主需改 ~15 个文件，未走注册表驱动

### 0.2 OpenCode 官方 Hook 系统确认

**来源**: [opencode.ai/docs/plugins](https://opencode.ai/docs/plugins)

| OpenCode Hook | 等价于 | 能力 |
|---|---|---|
| `tool.execute.before` | PreToolUse | 拦截工具调用，可 throw 阻断 |
| `tool.execute.after` | PostToolUse | 工具执行后处理 |
| `session.idle` | Stop | 会话空闲时触发 |
| `permission.asked` | Permission hooks | 权限拦截 |
| `shell.env` | 环境注入 | 注入 shell 环境变量 |

OpenCode **不是** anemic host。

### 0.3 三大目标

1. **彻底移除 Antigravity**：删除所有代码、配置、文档、投影
2. **OpenCode 升级为 rich host**：对齐四宿主核心能力
3. **注册表驱动**：消除硬编码宿主列表，新增/删除宿主只改 `RUNTIME_REGISTRY.json`

---

## 1. Antigravity 完整移除清单

### 1.1 Rust 源码（core/）— 22 个文件

| 文件 | 行号 | 操作 |
|---|---|---|
| `host-projection/src/hosts/antigravity_provider.rs` | 全文 | **删除文件** |
| `host-projection/src/hosts/mod.rs` | 9 | 删除 `pub mod antigravity_provider;` |
| `host-projection/src/hosts/host_provider.rs` | 321-325,388-397,483-493,509-510,531-533 | 删除 antigravity 测试断言 |
| `host-projection/src/hosts/mcp_stdio_harness.rs` | 1,39 | 删除 "Antigravity" 注释和 display label 分支 |
| `host-projection/src/host_integration/roots.rs` | — | 删除 `ANTIGRAVITY_HOME` / `ANTIGRAVITY_CLI_HOME` |
| `host-projection/src/host_integration/artifacts.rs` | 280 | 删除 `.gemini/antigravity/` 条目 |
| `host-projection/src/host_integration/projection.rs` | 305-356 | 删除 antigravity adapter；删除 ~180 行 install/status/remove 函数 |
| `host-projection/build.rs` | 43 | 删除 `("antigravity", ...)` 映射 |
| `runtime-core/src/cli/args.rs` | 19-27,280-288,314-319 | 删除 `RouterCommand::Antigravity` / `AntigravityApp` / `AntigravitySubcommand` |
| `runtime-core/src/cli/dispatch.rs` | 68-69,505-515 | 删除 `dispatch_antigravity_command()` 及 match arm |
| `runtime-core/src/framework_runtime/router_command_dispatch.rs` | 437-439 | 删除 antigravity 分发 |
| `runtime-core/src/framework_runtime/pre_tool_use_guard.rs` | 603-618 | 删除 antigravity 测试 |
| `runtime-core/src/framework_runtime/framework_doctor.rs` | 130-131,574-585 | 从 hint 和 cleanup list 删除 antigravity / `.gemini` |
| `runtime-core/src/framework_host_targets.rs` | 测试 | 删除 antigravity 测试引用 |
| `runtime-core/src/types.rs` | 88 | 删除 doc comment 中的 Antigravity |
| `framework-kernel/src/framework_host_targets.rs` | 测试 | 删除 antigravity 测试引用 |
| `framework-kernel/src/runtime_registry.rs` | 127-128 | 删除 `antigravity-app` / `antigravity-cli` alias |
| `core-policy/src/hook_policy.rs` | 25,31-32 | 删除 `AGENTS_ANTIGRAVITY.md` 和 `.antigravitycli/hooks.json` |
| `core-policy/src/registry_review_gate.rs` | 238 | 删除 antigravity MCP bullet |
| `core-state/src/state_manager/goal_ops.rs` | 64 | 删除 antigravity 注释 |
| `core-state/src/rfv_loop.rs` | 1 | 删除 antigravity 注释 |
| `router-rs/Cargo.toml` | feature | 删除 `host-antigravity` feature |
| `router-rs/src/bin/router-rs-cli.rs` | 14-15 | 删除 `antigravity-app` CLI alias |
| `router-rs/tests/hook_contract/mcp.rs` | 1,11 | 更新 doc comment 和 `MCP_HOSTS` 常量 |

### 1.2 配置文件

| 文件 | 操作 |
|---|---|
| `configs/framework/RUNTIME_REGISTRY.json` | 从 `supported` 删除 `"antigravity"`；删除 `metadata.antigravity`、`host_providers.antigravity`、`host_projections.antigravity` 整段 |

### 1.3 文档 / Agent Policy — 删除 4 文件

| 文件 | 操作 |
|---|---|
| `AGENTS.md:7` | 从闭集宿主列表删除 `antigravity` |
| `AGENTS_ANTIGRAVITY.md` | **删除文件** |
| `docs/hosts/antigravity.md` | **删除文件** |
| `docs/hosts/antigravity-app.md` | **删除文件** |
| `docs/hosts/antigravity-cli.md` | **删除文件** |

### 1.4 文档 — 更新 13 文件（antigravity 引用清理）

| 优先级 | 文件 | 引用 |
|---|---|---|
| **高** | `docs/spec.md` | 9 处：5宿主列表、能力矩阵、host entrypoint 表 |
| **高** | `docs/ONBOARDING.md:108-109` | 宿主表格含 Antigravity 行 |
| **高** | `docs/MCP_TOOL_CATEGORIES.md:8,18,246` | 5宿主列表、host-integration 提及 |
| **高** | `docs/host_adapter_contract.md:81,109,125` | 跨宿主差异表、闭集列表、接入 checklist |
| **高** | `docs/architecture/host-integration.md:19,24,36` | 宿主表格、"闭集五宿主" |
| **高** | `docs/references/AGENTS_OPERATOR_SURFACE.md:68,71` | `ROUTER_RS_ANTIGRAVITY_CLI_*` 环境变量 |
| **高** | `docs/references/EXECUTION_LADDER.md:14` | Antigravity MCP 强拦截描述 |
| **中** | `docs/architecture/overview.md:40,42,75` | 目录树含 `antigravity/` |
| **中** | `docs/framework_naming_conventions.md:48,52` | 配置路径表含 antigravity |
| **低** | `docs/maintenance/host-projection-schema-validity.md` | 历史 bug 诊断（可保留为历史记录） |
| **低** | `docs/rust_contracts/01-host-projection.md:16` | 退役列表（退役列表本身正确） |

### 1.5 Skills 目录 — antigravity platform 清理

| 文件 | 引用数 | 操作 |
|---|---|---|
| `skills/SKILL_PLUGIN_CATALOG.json` | ~32 处 | 从所有 `platforms` 数组删除 `"antigravity"` |
| `skills/SKILL_MANIFEST.json` | ~34 处 | 同上 |
| `skills/SKILL_ROUTING_RUNTIME.json` | 若干 | 同上 |
| `skills/my-lifecycle-common/references/routing-decision-table.md:39-40` | 1 处 | 删除 antigravity 行 |

### 1.6 投影 / 配置目录

| 文件/目录 | 操作 |
|---|---|
| `.gemini/.framework-projection-antigravity.json` | **删除** |
| `.gemini/antigravity/` | **删除整个目录** |
| `Justfile:13` | 删除 `core/antigravity/Cargo.toml` 引用 |

### 1.7 过时 Workflow 清理（.claude/workflows/）

| 文件 | 问题 | 操作 |
|---|---|---|
| `.claude/workflows/claude-code-cli-audit.js` | 5 处引用不存在的 `cli/antigravity-cli/` | 清理死引用 |
| `.claude/workflows/batch1-p0-fixes.js` | 引用不存在的 `core/antigravity/` | 清理死引用 |
| `.claude/workflows/full-closeout-audit.js` | 6 处引用含历史分支名 | 清理 |
| `.claude/workflows/hook-route-deep-audit.js` | 提及退役文档 | 清理 |

---

## 2. 注册表驱动：消除硬编码宿主列表

### 2.1 问题审计

深度扫描发现 **37 处硬编码宿主逻辑**，增减宿主需改 ~15 个文件：

| 类别 | 发现数 | 代表问题 |
|---|---|---|
| CLI 枚举/分发 | 10 | `HostCommand` 逐宿主 enum variant、`dispatch_*_command` 函数 |
| 配置投影 | 8 | `HOST_PROJECTION_ADAPTERS` 静态数组、`ResolvedProjectionRoots` 逐宿主字段 |
| 医生诊断 | 7 | hint 字符串硬编码宿主列表、cleanup list 手动维护 |
| MCP harness | 5 | `mcp_host_display_label` match、`run_antigravity_mcp_loop` 硬编码 |
| CLI 入口 | 5 | `router-rs-cli.rs` 硬编码 5 个宿主名、`--xxx-home` 参数 |
| 测试断言 | 2 | `anemic_hosts` 测试、`MCP_HOSTS` 常量 |

### 2.2 注册表驱动方案

**原则**：新增/删除宿主**只改** `RUNTIME_REGISTRY.json`，Rust 代码从注册表动态派生。

#### 2.2.1 CLI 入口（`router-rs-cli.rs`）

```rust
// 当前（硬编码 5 个宿主名）
if cmd == "codex" || cmd == "claude" || cmd == "cursor"
    || cmd == "antigravity-app" || cmd == "opencode" {
    args.insert(1, OsString::from("host"));
}

// 目标（注册表驱动）
let supported = load_supported_host_ids(); // 从 RUNTIME_REGISTRY.json
let cmd_str = args.get(1).and_then(|a| a.to_str()).unwrap_or("");
if supported.iter().any(|h| h == cmd_str) || host_aliases().any(|a| a == cmd_str) {
    args.insert(1, OsString::from("host"));
}
```

#### 2.2.2 CLI 分发（`args.rs` + `dispatch.rs`）

```rust
// 当前：6 个 enum variant × 6 个 dispatch 函数
enum HostCommand {
    Codex(CodexSubcommand),
    Cursor(CursorSubcommand),
    Claude(ClaudeSubcommand),
    Antigravity(AntigravitySubcommand),
    AntigravityAppHost(AntigravitySubcommand),
    Opencode(OpenCodeSubcommand),
}

// 目标：通用 Host variant + 注册表路由
enum HostCommand {
    // 特殊宿主保留复杂子命令（Codex 有 HookProjection/InstallHooks 等）
    Codex(CodexSubcommand),
    // 通用 MCP agent 宿主（OpenCode 和未来宿主）
    McpAgent { host_id: String },
    // 通用 hook 宿主（Cursor、Claude）
    Hook { host_id: String },
}
// dispatch_host_command 从 HostProvider trait 获取 dispatch_fn
```

#### 2.2.3 投影适配器（`projection.rs`）

```rust
// 当前：HOST_PROJECTION_ADAPTERS 静态数组 5 个 entry
static HOST_PROJECTION_ADAPTERS: &[HostProjectionAdapter] = &[
    HostProjectionAdapter { tool: "cursor", host_id: "cursor", install: install_cursor_projection, ... },
    HostProjectionAdapter { tool: "antigravity", host_id: "antigravity", install: install_antigravity_projection, ... },
    // ... 5 个
];

// 目标：从注册表动态构建 + HostProvider trait 方法
// HostProvider trait 新增：
fn projection_adapter(&self) -> Option<HostProjectionAdapter>;
// RUNTIME_REGISTRY.json 新增：
// host_projections.<id>.config_format: "json" | "toml" | "mdc"
// host_projections.<id>.config_path: ".opencode/opencode.json"
```

#### 2.2.4 主目录解析（`roots.rs`）

```rust
// 当前：8 个命名字段
struct ResolvedProjectionRoots {
    codex_home_root: PathBuf,
    cursor_home_root: PathBuf,
    claude_home_root: PathBuf,
    antigravity_home_root: PathBuf,      // ← 要删
    antigravity_cli_home_root: PathBuf,  // ← 要删
    opencode_home_root: PathBuf,
    // ...
}

// 目标：HashMap 动态派生
struct ResolvedProjectionRoots {
    host_homes: HashMap<String, PathBuf>,  // host_id → home_root
    // 通用字段保留
}
// 从 RUNTIME_REGISTRY.host_targets.supported 遍历，调用 resolve_host_home(host_id)
```

#### 2.2.5 Display label（`mcp_stdio_harness.rs`）

```rust
// 当前：硬编码 match
fn mcp_host_display_label(host_id: &str) -> &'static str {
    match host_id {
        "antigravity" | "antigravity-app" => "Antigravity",
        "opencode" => "Opencode",
        _ => "MCP Host",
    }
}

// 目标：HostProvider trait 新增 display_name()，或 RUNTIME_REGISTRY.metadata.<id>.display_name
fn mcp_host_display_label(host_id: &str) -> String {
    host_provider_for_id(host_id)
        .map(|p| p.display_name().to_string())
        .unwrap_or_else(|| format!("MCP Host ({host_id})"))
}
```

#### 2.2.6 Managed MCP server IDs（`projection.rs` 多处）

```rust
// 当前：多处硬编码
["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"]

// 目标：RUNTIME_REGISTRY.json 新增顶层
"managed_mcp_servers": ["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"]
// 代码中从注册表读取
```

#### 2.2.7 Doctor hint / cleanup list（`framework_doctor.rs`）

```rust
// 当前：hint 硬编码 "claude-code|antigravity|opencode"
// cleanup list 硬编码 [".antigravitycli", ".claude", ".codex", ".cursor", ".gemini", ".opencode"]

// 目标：
// hint: 从 RUNTIME_REGISTRY.host_targets.supported + metadata.installable 过滤
// cleanup: 保留手动列表（intentionally non-registry，需覆盖已移除宿主的残留目录）
//   但添加注释说明何时更新
```

### 2.3 注册表 schema 扩展（`RUNTIME_REGISTRY.json`）

```jsonc
{
  "host_targets": {
    "supported": ["cursor", "claude-code", "opencode", "codex"],
    "metadata": {
      "opencode": {
        "install_tool": "opencode",
        "display_name": "OpenCode",           // ← 新增
        "transport_type": "opencode-plugin",   // ← 新增
        "config_format": "json",               // ← 新增
        "cli_aliases": ["opencode"],           // ← 新增
        "installable": true,
        "home_env_var": "OPENCODE_HOME",       // ← 新增
        "default_home_dir": ".opencode",       // ← 新增
        "host_entrypoints": ".opencode/opencode.json"
      }
      // ... 其他宿主类似
    }
  },
  "managed_mcp_servers": [                     // ← 新增
    "router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"
  ],
  "retired_hosts": {                           // ← 新增
    "claude-desktop": "2026-06: use claude-code",
    "antigravity-app": "2026-06: merged into antigravity",
    "antigravity-cli": "2026-06: replaced by antigravity MCP",
    "antigravity": "2026-06: product line discontinued",
    "codex-cli": "2026-06: merged into codex"
  }
}
```

---

## 3. OpenCode 升级：anemic → rich host

### 3.1 Provider 升级

```rust
// opencode_provider.rs — 目标状态
impl HostLifecycle for OpencodeHostProvider {
    fn profile_id(&self) -> &'static str { "opencode_profile" }
    fn session_supervisor_driver(&self) -> &'static str { "unsupported" }
    fn harness_capabilities(&self) -> &'static [&'static str] { HARNESS_CAPABILITIES_FULL }
    fn context_file(&self) -> &'static str { "AGENTS_OPENCODE.md" }
    fn hooks_manifest_path(&self) -> Option<&'static str> { Some(".opencode/plugins/") }
    fn registered_hook_events(&self) -> &'static [&'static str] {
        &["tool.execute.before", "tool.execute.after", "session.idle", "permission.asked"]
    }
}
impl HostToolExecutor for OpencodeHostProvider {
    fn has_hard_gate_hooks(&self) -> bool { false }
    fn closeout_evidence_hooks_supported(&self) -> bool { true }        // ← 升级
    fn requires_strict_pre_tool_fallback_default(&self) -> bool { false } // ← 升级
}
impl HostTelemetry for OpencodeHostProvider {
    fn review_gate_router_observable(&self) -> bool { true }             // ← 升级
    fn hook_telemetry_surface(&self) -> &'static str { "opencode-plugin" }
    fn observation_host_id(&self) -> Option<&'static str> { Some("opencode") }
}
impl HostProvider for OpencodeHostProvider {
    fn host_id(&self) -> &'static str { "opencode" }
    fn install_tool(&self) -> &'static str { "opencode" }
    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            has_native_hook: true,        // ← 升级
            supports_subagent: true,      // ← 升级
            supports_worktree: false,
            mcp_config_key: "mcp",
            transport_type: "opencode-plugin",  // ← 改名
            config_path: ".opencode/opencode.json",
            ..Default::default()
        }
    }
}
```

### 3.2 新增 `opencode_hooks.rs`

```rust
//! OpenCode plugin hook event constants.
//! OpenCode uses JS/TS plugins (not shell hooks).
//! Plugins: ~/.config/opencode/plugins/ + .opencode/plugins/

pub const OPENCODE_HOOKS_PATH: &str = ".opencode/plugins/";
pub const OPENCODE_HOOKS_REGISTERED_EVENTS: &[&str] = &[
    "tool.execute.before", "tool.execute.after",
    "session.idle", "session.created", "session.deleted",
    "permission.asked", "permission.replied",
    "file.edited", "shell.env",
];
```

### 3.3 测试更新

- `host_provider.rs`: 删除 `anemic_hosts_skip_native_hook_glue_defaults` 整个测试（不再有 anemic host）
- `host_provider.rs`: 新增 `opencode_capabilities_declare_plugin_hooks` 断言全量升级
- `pre_tool_use_guard.rs`: 删除 antigravity 测试；更新 opencode 测试（不再 requires strict）
- `host_provider.rs:16`: `HARNESS_CAPABILITIES_MINIMAL` doc 改为 `#[deprecated]`

### 3.4 `HARNESS_CAPABILITIES_MINIMAL` 废弃

```rust
#[deprecated(note = "All closed-set hosts use HARNESS_CAPABILITIES_FULL. Will be removed in v7.")]
pub const HARNESS_CAPABILITIES_MINIMAL: &[&str] = &[
    "hot_runtime_routing", "l2_continuity_contract",
];
```

---

## 4. 四宿主一致性矩阵

### 4.1 核心一致点（全部 ✅）

| 维度 | Claude Code | Codex | Cursor | OpenCode |
|---|---|---|---|---|
| `has_native_hook` | ✅ | ✅ | ✅ | ✅ |
| `supports_subagent` | ✅ | ✅ | ✅ | ✅ |
| `HARNESS_CAPABILITIES` | FULL | FULL | FULL | FULL |
| `closeout_evidence_hooks` | ✅ | ✅ | ✅ | ✅ |
| `review_gate_router_observable` | ✅ | ✅ | ✅ | ✅ |
| `requires_strict_pre_tool_fallback` | ❌ | ❌ | ❌ | ❌ |

### 4.2 有意差异（产品决定，不强制统一）

| 差异 | Claude Code | Codex | Cursor | OpenCode |
|---|---|---|---|---|
| `supports_worktree` | ✅ | ✅ | ✅ | ❌ |
| `session_supervisor` | ✅ | ✅ | ❌ | ❌ |
| `has_hard_gate_hooks` | ✅ | ❌ | ❌ | ❌ |
| Hook 机制 | Shell | Shell | Shell | JS Plugin |
| MCP config key | (empty) | `mcp_servers` | `mcpServers` | `mcp` |

---

## 5. 文档全面更新

### 5.1 需更新文件清单（按优先级）

**P0 — 宿主列表直接错误**:
1. `AGENTS.md:7` — 闭集 5→4
2. `docs/hosts/opencode.md` — 全面重写，反映插件 hook
3. `docs/host_adapter_contract.md` — 移除 antigravity、更新 OpenCode 分类
4. `docs/spec.md` — 9 处 antigravity 引用
5. `docs/ONBOARDING.md:108-109` — 移除 antigravity 行
6. `docs/MCP_TOOL_CATEGORIES.md` — 5→4 宿主列表
7. `docs/architecture/host-integration.md` — 移除 antigravity 行
8. `docs/references/AGENTS_OPERATOR_SURFACE.md` — 移除 antigravity CLI 变量
9. `docs/references/EXECUTION_LADDER.md:14` — 移除 antigravity 引用
10. `AGENTS_OPENCODE.md` — 更新 transport 要点反映插件 hook

**P1 — Skills 路由数据**:
11. `skills/SKILL_PLUGIN_CATALOG.json` — ~32 处 antigravity platform 清理
12. `skills/SKILL_MANIFEST.json` — ~34 处同上
13. `skills/SKILL_ROUTING_RUNTIME.json` — antigravity platform 清理
14. `skills/my-lifecycle-common/references/routing-decision-table.md` — 移除 antigravity 行

**P2 — 架构文档**:
15. `docs/architecture/overview.md` — 更新目录树
16. `docs/framework_naming_conventions.md` — 移除 antigravity 路径

---

## 6. 代码缩减估算

### 6.1 直接删除（antigravity 移除）

| 区域 | 预估行数 |
|---|---|
| `antigravity_provider.rs` 删除 | ~78 行 |
| `projection.rs` antigravity install/status/remove | ~180 行 |
| `args.rs` Antigravity CLI 类型 | ~30 行 |
| `dispatch.rs` antigravity dispatch | ~15 行 |
| `roots.rs` antigravity home 解析 | ~20 行 |
| `framework_maint.rs` antigravity 验证 | ~90 行 |
| `AGENTS_ANTIGRAVITY.md` + docs | ~15 行 |
| 测试代码 | ~40 行 |
| **小计** | **~468 行** |

### 6.2 间接删除（消除硬编码 → 注册表驱动）

| 区域 | 预估行数 |
|---|---|
| CLI alias 硬编码 → 注册表循环 | ~10 行 |
| `mcp_host_display_label` match → trait 方法 | ~5 行 |
| `auto_clean_broken_symlinks` 注释清理 | ~3 行 |
| `lifecycle_paragraph_for_host` alias match → 注册表 | ~10 行 |
| `managed_mcp_servers` 多处硬编码 → 注册表读取 | ~15 行 |
| `allowed_dot_generated_artifact` 路径列表 → 注册表派生 | ~15 行 |
| **小计** | **~58 行** |

### 6.3 净代码变化估算

| | 行数 |
|---|---|
| 删除（antigravity + 硬编码） | -526 行 |
| 新增（opencode_hooks.rs + 注册表扩展 + 测试） | +120 行 |
| **净减少** | **~406 行** |

---

## 7. 执行波次

### Wave 1: Antigravity 移除（最大变更面，~468 行删除）
1. 删除 `antigravity_provider.rs` + `AGENTS_ANTIGRAVITY.md`
2. 清理 `RUNTIME_REGISTRY.json` 四段
3. 清理 `build.rs` + `Cargo.toml` features
4. 清理 `args.rs` / `dispatch.rs` CLI 类型和分发
5. 清理 `projection.rs` adapter + install/status/remove 函数
6. 清理 `roots.rs` / `artifacts.rs` / `mod.rs` 投影相关
7. 清理 `framework_doctor.rs` hint + cleanup
8. 清理 `mcp_stdio_harness.rs` display label + 注释
9. 清理 `hook_policy.rs` / `registry_review_gate.rs` / `runtime_registry.rs`
10. 清理所有测试引用
11. 删除 `.gemini/` 投影目录
12. `cargo check` 验证编译

### Wave 2: OpenCode 升级（~80 行新增/修改）
1. 新增 `opencode_hooks.rs` + `mod.rs` 注册
2. 升级 `opencode_provider.rs` 全部 trait 实现
3. 更新 `mcp_stdio_harness.rs` 注释
4. 更新 `host_provider.rs` 测试
5. 更新 `pre_tool_use_guard.rs` 测试
6. `HARNESS_CAPABILITIES_MINIMAL` 标记 `#[deprecated]`
7. `cargo test` 验证

### Wave 3: 注册表驱动（~40 行新增，~58 行删除）
1. `RUNTIME_REGISTRY.json` 新增 `managed_mcp_servers` / `retired_hosts` / `display_name`
2. `router-rs-cli.rs` CLI alias → 注册表循环
3. `mcp_stdio_harness.rs` display label → trait 方法
4. `framework_doctor.rs` hint → 注册表派生
5. `artifacts.rs` 允许列表 → 注册表派生
6. `projection.rs` managed MCP server IDs → 注册表读取
7. `cargo test` 验证

### Wave 4: 文档全量同步（P0 + P1 + P2 全部完成）
1. 更新 `AGENTS.md` / `AGENTS_OPENCODE.md`
2. 重写 `docs/hosts/opencode.md`
3. 更新 `docs/host_adapter_contract.md` / `docs/spec.md` / `docs/ONBOARDING.md`
4. 更新 `docs/MCP_TOOL_CATEGORIES.md` / `docs/architecture/host-integration.md`
5. 更新 `docs/references/AGENTS_OPERATOR_SURFACE.md` / `docs/references/EXECUTION_LADDER.md`
6. 批量清理 skills JSON 中的 antigravity platform
7. 更新 `docs/architecture/overview.md` / `docs/framework_naming_conventions.md`
8. 清理 `.claude/workflows/` 过时 antigravity 引用

### Wave 5: 最终验证
1. `cargo build` 全 feature 组合
2. `cargo test` 全量通过
3. `grep -ri antigravity --include='*.rs' --include='*.json' --include='*.md' --include='*.toml' .` → 零结果
4. `grep -ri "anemic\|MINIMAL"` → 仅 deprecated 注解
5. 确认 `REGISTRY_SUPPORTED_HOST_IDS` 长度为 4
6. 确认四宿主能力矩阵一致

---

## 8. 风险与缓解

| 风险 | 概率 | 缓解 |
|---|---|---|
| antigravity 删除导致编译失败 | 中 | 逐文件删除，每步 `cargo check` |
| skills JSON 批量清理引入格式错误 | 中 | JSON 验证脚本 |
| 注册表驱动改造破坏现有 CLI 行为 | 低 | 现有集成测试覆盖 |
| OpenCode 升级后 MCP harness 行为变化 | 低 | harness 代码不变，只改 provider metadata |
| `HARNESS_CAPABILITIES_MINIMAL` deprecated 后编译警告 | 低 | 确认无活跃使用后 suppress |

---

## 9. 验收标准

- [ ] `grep -ri antigravity` （*.rs *.json *.md *.toml）→ 零结果
- [ ] `cargo build` 全 feature 组合通过
- [ ] `cargo test` 全量通过
- [ ] 四宿主 `has_native_hook == true`
- [ ] 四宿主 `HARNESS_CAPABILITIES == FULL`
- [ ] 四宿主 `closeout_evidence_hooks_supported() == true`
- [ ] 四宿主 `review_gate_router_observable() == true`
- [ ] 四宿主 `requires_strict_pre_tool_fallback_default() == false`
- [ ] `REGISTRY_SUPPORTED_HOST_IDS` 长度为 4
- [ ] CLI alias 从注册表派生（无硬编码宿主名列表）
- [ ] display label 从 HostProvider trait 获取
- [ ] managed MCP server IDs 从注册表读取
- [ ] P0 + P1 + P2 文档全部更新
- [ ] 净代码减少 > 300 行
