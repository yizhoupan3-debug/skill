---
last_verified: "2026-06-02"
depends_on:
  - harness_architecture.md
  - harness_policy_map.md
---

# Operator profiles（运维开关组合）

本页只提供**可复制**的 `ROUTER_RS_*` 组合，**不**定义第二套默认值。逐变量语义、默认值与脚注以 [`harness_architecture/03-hook-and-switches.md` §5 开关面](harness_architecture/03-hook-and-switches.md#5-开关面) 为唯一裁判（含「`_CHARS` 实为 UTF-8 字节」等说明）。叙事分层见 [`harness_policy_map.md`](harness_policy_map.md)。

## 默认（profile 之外）

- **2026-05 减法默认（`router-rs` unset）**：PostTool 证据 **关**；`GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` hook 注入与 Stop checkpoint **已拔除**（相关 env 无操作）；需显式 `=1` 仅对 PostTool 等仍生效项开启（见 harness §5）。
- **CI closeout**：仍由 `GITHUB_ACTIONS` / `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 控制硬门禁（solo 本地默认软）。
- 下列 profile 仅在你在 shell 中**显式 `source`/export** 后生效；关能力不等于关闭硬门控短码（见 harness §4 / §5 各节）。

## Solo / low-noise（单人、**与代码默认一致** + 可选再关注入）

目标：与 unset 默认一致；可选再关 operator nudge。若要恢复 PostTool 自动记账，显式开启下表 `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1`（**不能**恢复 hook `GOAL_CONTINUE` / Stop checkpoint）。

**cwd**：下文 `source` 与相对路径 `configs/...` 均以 **Git 仓库根** 为当前目录；请先 `cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"`（或手动 `cd` 到 clone 根）再执行，**从子目录 `source` 会找不到文件**。若不在 Git 工作副本中或 `git rev-parse` 失败，回退的 `pwd` **不保证**为仓库根，请自行确认当前目录下存在 `configs/framework/`。

建议在**新 shell** 中逐条核对后复制（或 `source configs/framework/OPERATOR_PROFILE_SOLO.env.example`）：

| 变量 | profile 建议 | harness §5 语义摘要 |
|------|----------------|---------------------|
| `ROUTER_RS_OPERATOR_INJECT` | **Solo：默认不要关总闸**（保持 unset）；若你显式设为 `0`，SessionStart 轻量 advisory 等会按 harness §2.1 / §5 关闭 | 总闸；**不含**已拔除的 digest / hook 续跑 |
| `ROUTER_RS_HARNESS_OPERATOR_NUDGES` | 可设 `0` | 仅关 operator nudge 文案（stdio / 契约，非 hook `GOAL_CONTINUE`） |
| `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` | 默认 **关**；设 `1` 开启 | PostTool 自动追加 `EVIDENCE_INDEX` |
| `ROUTER_RS_CURSOR_HOOK_SILENT` | 可设 `1` | 出站 policy 后剥 advisory；硬短码保留（harness §4.2） |
| `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE` | 可设 `0` | 关 Stop 软 `SESSION_CLOSE_STYLE` |

**明确不在 solo 默认里关闭的项（除非你自己接受 trade-off）**：

- `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`：应急关 Cursor review 门控，**语义与常态不同**（harness §5.0）；默认应保持 unset。
- `ROUTER_RS_TASK_LEDGER_FLOCK`：仅在网络 FS 不稳时考虑 `0`；否则多进程写账本可能竞态（harness §3.1）。

示例块见仓库 [`configs/framework/OPERATOR_PROFILE_SOLO.env.example`](../configs/framework/OPERATOR_PROFILE_SOLO.env.example)。

## 与 `ROUTER_RS_OPERATOR_PROFILE` 的关系

- 本仓库**当前不**实现单一环境变量「profile 一键切换」；若未来增加，须以 harness §5 为真源做映射，并配契约测试。
