---
last_verified: "2026-06-02"
depends_on:
  - harness_architecture/index.md
  - host_adapter_contract.md
  - README.md
redirect_to: rust_contracts/index.md
---

# Runtime Rust Contracts

> **本文已拆分**。完整内容见 [`rust_contracts/index.md`](rust_contracts/index.md)。
> 本文件保留以兼容旧链接。运行时只读视图：`router-rs framework snapshot`；宿主同步：`codex sync --repo-root`；stdio `execute` operation 见 index 契约节。

## 拆分导航

| 主题 | 文件 |
|------|------|
| 概述 + 契约规则 + 当前边界 | [rust_contracts/index.md](rust_contracts/index.md) |
| 宿主投影不变量 | [rust_contracts/01-host-projection.md](rust_contracts/01-host-projection.md) |
| 路由契约 + 插件 ABI | [rust_contracts/02-routing-and-plugin.md](rust_contracts/02-routing-and-plugin.md) |
| 状态账本 + 可移植性 + 外部基准 | [rust_contracts/03-status-and-portability.md](rust_contracts/03-status-and-portability.md) |
