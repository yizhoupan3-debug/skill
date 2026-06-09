---
last_verified: "2026-06-09"
plate: B0
---

# B0 — framework-kernel

## 职责

框架内核：共享类型、hook 策略、数学验证、registry 读盘与 **TelemetryWriter** trait。物理 crate：

| 子 crate | 路径 | 说明 |
|----------|------|------|
| core-state | `core/core-state/`（package 名 `core-state`） | 任务状态、ledger 锁、观察载荷 |
| core-policy | `core/core-policy/` | hook_policy、review_gate registry、MCP 安全 |
| core-math | `core/core-math/` | 形式化 / 数学验证（feature `math-verify`） |
| framework-kernel | `core/framework-kernel/` | 轻量共享 trait / 类型 |

## 启动 / 配置

- 无独立二进制；随 `router-rs` 链接编译。
- **Registry 真源**：`configs/framework/RUNTIME_REGISTRY.json`（`review_gate.reviewer_lanes`、`host_targets` 等）。
- **Telemetry**：journal 写入经 MPSC；`ROUTER_RS_HOOK_TIMING=1` 可打 hook 耗时（见 B11）。
- **Task ledger 锁**：默认 `artifacts/current/.router-rs.task-ledger.lock`（`ROUTER_RS_TASK_LEDGER_FLOCK=0` 可关，见 B3）。

Feature gates（`core/router-rs/Cargo.toml`）：`math-verify`、`dev-exempt` 等。

## 排障

| 现象 | 处理 |
|------|------|
| review lane 全部 fail-closed | `RUNTIME_REGISTRY.json` 读盘失败；`framework doctor` 看 `review_gate snapshot` WARN |
| MCP 危险工具被拦 | `hook_policy` / `mcp_safety`；跑 `cargo test -p router-rs smoke_p0_hook_policy` |
| 账本 JSON 撕裂 | 确认 `ROUTER_RS_TASK_LEDGER_FLOCK` 未关；网络 FS 上考虑关闭并行写 |
| Eval cases 路径越权 | `core-policy` eval 路径保护（SEC-002） |

## 相关路径

- `configs/framework/RUNTIME_REGISTRY.json`
- `configs/framework/CLOSEOUT_RECORD_SCHEMA.json`
- `core/core-policy/src/`
- `docs/harness_architecture/02-data-flows.md` §2.3（registry / 生成物）
- `docs/references/AGENTS_OPERATOR_SURFACE.md`（env 详表，本页不重复）
