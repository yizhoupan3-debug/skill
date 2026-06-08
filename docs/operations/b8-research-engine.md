---
last_verified: "2026-06-09"
plate: B8
---

# B8 — research-engine

## 职责

外研与 RFV（Review → Fix → Verify）闭环：`framework_rfv_loop` stdio、`autoresearch-rs` CLI、外研 harness 契约与 lane 模板。

## 启动 / 配置

```bash
# RFV loop（stdio，经 router-rs MCP harness 或直连 JSON payload）
# 见 skills 与 docs/rfv_loop_harness.md

# autoresearch-rs（独立 crate）
cargo run --manifest-path core/autoresearch-rs/Cargo.toml -- --help
```

状态磁盘：`artifacts/current/<task_id>/RFV_LOOP_STATE.json`；轮次追加经 `framework_rfv_loop` `append_round`。

深度外研契约：[`../references/rfv-loop/external-research-harness.md`](../references/rfv-loop/external-research-harness.md)。

## 排障

| 现象 | 处理 |
|------|------|
| `framework_rfv_loop requires repo_root` | payload 补 `repo_root` 或激活 `active_task.json` |
| 外研未标「深度完成」 | 缺 `retrieval_trace` / Contradiction sweep；见 reasoning-depth-contract |
| RFV 轮次未进 evolution | B11：`rfv_round` journal 事件；`evolution-rs analyze` 分桶 |
| autoresearch 网络失败 | 检查代理 / TLS；crate 使用 `reqwest` blocking |

## 相关路径

- `core/router-rs/src/rfv_loop.rs`
- `core/autoresearch-rs/`
- `docs/rfv_loop_harness.md`
- `docs/references/rfv-loop/`
- `skills/` 中 planx / verifyx / deepinterview 路由提示
