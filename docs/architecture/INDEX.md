---
last_verified: "2026-06-02"
depends_on:
  - overview.md
  - components.md
  - data-flow.md
  - security.md
  - host-integration.md
---

# Architecture 文档索引

本文档是 `docs/ARCHITECTURE.md` 拆分后的索引。原文件已重定向至此。

## 子文档

| 主题 | 文档 | 说明 |
|------|------|------|
| 架构总览 | [overview.md](overview.md) | 仓库定位、默认生命周期、源码地图 |
| 组件详解 | [components.md](components.md) | skill 体系、router-rs、antigravity、configs |
| 数据流 | [data-flow.md](data-flow.md) | 用户请求全链路、skill 路由、goal drive、证据流 |
| 安全模型 | [security.md](security.md) | 测试层次、CI 流水线、schema drift、生成物 drift |
| 宿主集成 | [host-integration.md](host-integration.md) | 宿主列表、hook 差异、shell launcher、跨仓库接入 |

## 推荐阅读顺序

1. [overview.md](overview.md) — 先理解仓库定位和源码地图
2. [components.md](components.md) — 深入各组件职责和文件结构
3. [data-flow.md](data-flow.md) — 理解数据如何在组件间流动
4. [host-integration.md](host-integration.md) — 宿主适配层细节
5. [security.md](security.md) — 测试和质量保障体系

## 关联文档

- `docs/harness_architecture/` — 五层模型详细设计（已拆分为 4 个文件）
- `docs/rust_contracts/` — Rust 实现契约（已拆分为 3 个文件）
- `docs/host_adapter_contract.md` — 多宿主适配契约
- `docs/framework_operator_primer.md` — 使用者一页纸
- `docs/README.md` — 文档总索引
