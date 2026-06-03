---
last_verified: "2026-06-02"
depends_on:
  - architecture/overview.md
  - architecture/components.md
  - architecture/data-flow.md
  - architecture/security.md
  - architecture/host-integration.md
  - harness_architecture/index.md
  - rust_contracts.md
  - README.md
redirect_to: architecture/INDEX.md
---

# Architecture

> **本文已拆分**。完整内容见 [`architecture/INDEX.md`](architecture/INDEX.md)。
> 本文件保留以兼容旧链接。

## 拆分导航

| 主题 | 文件 |
|------|------|
| 架构总览（仓库定位、生命周期、源码地图） | [architecture/overview.md](architecture/overview.md) |
| 组件详解（skill 体系、router-rs、antigravity、configs） | [architecture/components.md](architecture/components.md) |
| 数据流（用户请求、skill 路由、goal drive、证据流） | [architecture/data-flow.md](architecture/data-flow.md) |
| 安全模型（测试、CI、schema drift、生成物 drift） | [architecture/security.md](architecture/security.md) |
| 宿主集成（宿主列表、hook 差异、shell launcher、跨仓库接入） | [architecture/host-integration.md](architecture/host-integration.md) |
| 索引 | [architecture/INDEX.md](architecture/INDEX.md) |

## 关联已拆分文档

| 文档 | 说明 |
|------|------|
| [`harness_architecture/`](harness_architecture/index.md) | 五层模型详细设计 |
| [`rust_contracts/`](rust_contracts/index.md) | Rust 实现契约 |
| [`host_adapter_contract.md`](host_adapter_contract.md) | 多宿主适配契约 |
| [`framework_operator_primer.md`](framework_operator_primer.md) | 使用者一页纸 |
