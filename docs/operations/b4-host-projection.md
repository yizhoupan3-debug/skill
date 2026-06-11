---
last_verified: "2026-06-09"
plate: B4
---

# B4 — host-projection

## 职责

将框架统一协议**投影**到各宿主：hook 绑定、MCP 注册、`host_projection_narrative` 文案、生成物 drift 探测。实现：`core/host-projection/src/host_integration/`、各 `hosts/*_hooks`。

**宿主 id 闭集**：仅以 [`configs/framework/RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json) → `host_targets.supported` 为准。

## 启动 / 配置

```bash
# 安装 / 刷新投影（<host_id> 取自 RUNTIME_REGISTRY）
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to <host_id> --repo-root "$PWD" [--scope user|project]

# 状态
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration status

# 生成物快探针（metadata-only，doctor 默认）
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration generated-artifacts-status --skip-generator-run
```

叙事文案真源：`configs/framework/host_projection_narrative.json`（安装时渲染，勿在 `host_integration/mod.rs` 硬编码段落）。

新宿主接入工程清单：[`../host_adapter_contract.md`](../host_adapter_contract.md) §3.1。

## 排障

| 现象 | 处理 |
|------|------|
| 投影 drift | `framework maint update-one-shot`（全量 drift-gate）或按 [`02-data-flows.md`](../harness_architecture/02-data-flows.md) §2.3 逐项修复 |
| hook 未触发 | 核对宿主手册中 hook 事件闭集；`framework maint verify-cursor-hooks`（Cursor） |
| 重复 hook 执行 | `framework doctor` 统计多份 hooks.json 并 WARN；保留一份入口 |
| MCP server 陈旧 | 重跑 `host-integration install`（含 browser-mcp / mcp-codegraph 重写） |

**宿主专项 Stop / REVIEW_GATE / hook-state**：见 [`docs/hosts/`](../hosts/) 对应手册，本页不列 per-host 表。

## 相关路径

- `core/host-projection/src/host_integration/`
- `configs/framework/host_projection_narrative.json`
- `configs/framework/GENERATED_ARTIFACTS.json`（若存在）/ harness §2.3
- `docs/hosts/`（各宿主操作手册）
- `docs/host_adapter_contract.md`
