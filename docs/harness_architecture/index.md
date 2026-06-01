---
last_verified: "2026-06-02"
depends_on:
  - ../host_adapter_contract.md
  - ../rust_contracts/index.md
  - ../hook_lock_order.md
  - ../AGENTS.md
---

# Continuity harness architecture

本文件是 harness 的唯一长解释面，负责说明：

- 五层结构与数据流
- 热路径应该读什么、不该读什么
- hook 可见提示如何投影
- 哪些环境变量仍然有效
- 哪些兼容层被刻意删除

跨宿主执行协议、语言与收口原则见仓库根 [`AGENTS.md`](../../AGENTS.md)。宿主接入见 [`host_adapter_contract.md`](../host_adapter_contract.md)。Rust 运行时契约见 [`rust_contracts/index.md`](../rust_contracts/index.md)。Hook **flock 锁序**（L1/L2/L3）见 [`hook_lock_order.md`](../hook_lock_order.md)。

## 拆分导航

本文件已拆分为以下聚焦子文档：

| 主题 | 文件 |
|------|------|
| 五层模型 + 扩展规则 + 文件映射（本文件） | [index.md](index.md)（当前） |
| 热路径真源 + 证据流 + 续跑门控 | [data-flows.md](data-flows.md) |
| Hook 文案策略 + 环境变量表 | [hook-and-switches.md](hook-and-switches.md) |
| Closeout + 深度调研对齐 | [closeout-and-depth.md](closeout-and-depth.md) |
