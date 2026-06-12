---
module: host-projection::host_integration::projection
lines: ~3700
layer: B2
last_verified: "2026-06-13"
---

# projection 子模块详解

宿主投影的核心逻辑：install/status/remove 操作、bootstrap 生成、manifest 管理。

## 文件结构

| 文件 | 行数 | 功能 |
|------|------|------|
| `mod.rs` | 1,590 | 核心投影逻辑：`HostProjectionAdapter` trait、四宿主适配器常量、MCP payload 生成、narrative 渲染 |
| `projection_bootstrap.rs` | 702 | 默认 bootstrap 生成与验证（`build_default_bootstrap_payload`） |
| `projection_host_ops.rs` | 659 | 各宿主 install/status/remove 实现 |
| `projection_manifest.rs` | 747 | 投影 manifest CRUD、entrypoint 渲染、Cursor MCP 服务器管理 |

## 核心 trait

```rust
pub struct HostProjectionAdapter {
    pub install: fn(...) -> Result<...>,
    pub status: fn(...) -> Result<...>,
    pub remove: fn(...) -> Result<...>,
    pub home_root: fn(...) -> Option<PathBuf>,
    pub explicit_home: fn(...) -> Option<String>,
}
```

四个适配器常量: `CURSOR_ADAPTER`、`CLAUDE_ADAPTER`、`OPENCODE_ADAPTER`、`CODEX_ADAPTER`。

## 关键 pub 函数

| 函数 | 文件 | 作用 |
|------|------|------|
| `install_native_integration` | mod.rs | 主安装入口 |
| `projection_install_command` | mod.rs | CLI install 子命令 |
| `projection_status_command` | mod.rs | CLI status 子命令 |
| `canonical_tool_name` | projection_manifest.rs | 工具名标准化（含 alias 匹配） |
| `render_claude_framework_entrypoint` | mod.rs | Claude entrypoint 渲染 |
| `render_codex_framework_entrypoint` | projection_manifest.rs | Codex entrypoint 渲染 |
| `render_cursor_framework_entrypoint` | projection_manifest.rs | Cursor entrypoint 渲染 |
| `install_cursor_mcp_server` | projection_manifest.rs | Cursor MCP 服务器安装 |

## 已知技术债

- `projection_manifest.rs` 命名为 "manifest" 但包含大量 entrypoint 渲染逻辑
- 管理 MCP 键列表 `["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"]` 在 7 处重复
- `skills_runtime_rel_path` 计算逻辑在 3 处重复
- `framework_entrypoint_common_footer` 仅 Claude 使用，Codex/Cursor 内联了相同内容
