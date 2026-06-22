---
module: browser-mcp
lines: ~6100
layer: B2
last_verified: "2026-06-22"
---

# browser-mcp（B2 层）

浏览器自动化 MCP crate，提供 CDP 协议的 stdio MCP 服务。

## 职责

通过 Chrome DevTools Protocol (CDP) 提供浏览器自动化能力：页面导航、截图、元素交互、session supervisor、background jobs。

## 架构

该 crate 使用 `include!()` 将多个 frag 文件合并为单一模块（非标准 Rust 模块系统）。

```rust
// lib.rs
include!("frag_01_through_types.rs");
include!("frag_impl_browser_runtime.rs");
include!("frag_impl_browser_runtime_attach.rs");
include!("frag_impl_cdp.rs");
include!("frag_rest.rs");
include!("tests.rs");
```

## 文件结构

| 文件 | 行数 | 功能 |
|------|------|------|
| `lib.rs` | 50 | 入口：`dispatch_browser_command`、`register_browser_dispatch` |
| `frag_01_through_types.rs` | 780 | MCP 常量、stdio transport、`BrowserRuntime`/`BrowserAttachConfig`/`CdpClient` 类型定义 |
| `frag_impl_browser_runtime.rs` | 1,910 | `impl BrowserRuntime`（页面管理、截图、导航、元素交互） |
| `frag_impl_browser_runtime_attach.rs` | 320 | `impl BrowserRuntime` attach 相关方法（runtime descriptor 解析、artifact 发现） |
| `frag_impl_cdp.rs` | 167 | `impl CdpClient`（CDP 协议底层通信） |
| `frag_rest.rs` | 1,119 | CDP HTTP 助手、Attach 候选与 skill 路由、工具 JSON schema |
| `tests.rs` | 1,688 | 测试 |

## 关键 pub 接口

| 函数 | 作用 | 调用方 |
|------|------|--------|
| `dispatch_browser_command` | CLI 分发入口 | `runtime-core::browser_dispatch_hook` |
| `register_browser_dispatch` | 向 runtime-core 注册分发函数 | `router-rs` 启动时 |

## MCP 工具

browser-mcp 暴露的 MCP 工具（通过 stdio）：
- `browser_open/close/tabs` — 标签页管理
- `browser_screenshot/get_text/get_state/get_elements/get_network` — 页面交互与状态
- `browser_click/fill/press/wait_for` — 元素交互
- `browser_save_session/restore_session` — 会话持久化
- `browser_diagnostics` — 运行时诊断

## 依赖关系

- **依赖**: `serde_json`、`tokio`（async）、`reqwest`（HTTP）
- **被依赖**: `runtime-core`（通过 `browser_dispatch_hook` 函数指针注册）

## 近期变更

- v6.5: `frag_impl_browser_runtime_attach.rs` 从 `frag_impl_browser_runtime.rs` 提取（控制文件大小 ≤2000 行）

## 已知技术债

- `include!()` 架构非标准，代码导航困难
- `lib.rs` 文档表格未反映 attach 方法已提取到新文件
