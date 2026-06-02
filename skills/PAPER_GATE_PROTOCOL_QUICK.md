# Paper Gate Protocol — 速查卡片

> 完整版：[`PAPER_GATE_PROTOCOL.md`](PAPER_GATE_PROTOCOL.md)。仅 L3（多轮磁盘状态）需要完整版。

## 何时使用

交互式审稿**不需要**本协议。仅在以下情况物化磁盘门控：
- 用户显式要求多轮跟踪、并行 sidecar、或协议工件
- 跨会话需要可追溯的 gate 文件

默认用户响应：verdict → blockers → evidence gaps → next honest move。

## Edit scope gate

| | `surgical`（默认） | `refactor`（需用户授权） |
|---|---|---|
| 允许 | 锚定 span 内改句式/衔接/术语 | 删并节、附录路由、throughline 重写 |
| 禁止 | 跨节改写、整篇回贴、静默全局替换 | 把「降 claim」当默认逃逸（须走阶梯） |
| 交付 | hunk/逐条 `change_id` 清单 | `sections_touched` + `claim_ledger_touch_statement` |

权威定义：[`paper-workbench/references/edit-scope-gate.md`](paper-workbench/references/edit-scope-gate.md)

## Claim–evidence 决策阶梯

降 claim 不得是沉默默认。顺序：补证据 → 强化论证 → 呈现优化 → 缩口径。
权威定义：[`paper-workbench/references/claim-evidence-ladder.md`](paper-workbench/references/claim-evidence-ladder.md)

## Gate 链（G0–G14）

| Gate | Slug | Kind | 输出 |
|---|---|---|---|
| G0 | target_contract | setup | pass/fail |
| G1 | fatal_eligibility | decision | ideal/hide/abandon |
| G2 | core_evidence | decision | ideal/hide/abandon |
| G3 | claim_ceiling | decision | ideal/hide/abandon |
| G4 | math_closure | decision | ideal/hide/abandon |
| G5 | reference_support | decision | ideal/hide/abandon |
| G6 | main_vs_appendix | decision | ideal/hide/abandon |
| G7 | narrative_spine | quality | ideal_only |
| G8 | front_door_text | quality | ideal_only |
| G9 | mirror_consistency | quality | ideal_only |
| G10 | notation_consistency | quality | ideal_only |
| G11 | figure_gate | quality | ideal_only |
| G12 | table_gate | quality | ideal_only |
| G13 | language_naturalness | quality | ideal_only |
| G14 | rendered_layout | quality | ideal_only |

## 核心规则

**Freeze/Backjump**：gate 通过即冻结；质量门发现上游回退时设 `backjump_gate_on_regression`；决策门是唯一可选 hide/abandon 的位置；sidecar 不可独立冻结 gate。

**Scope**：`full_chain`（仅用户显式要求全链）或 `single_gate`（用户点名某维度/G gate）。

**隔离**：每轮用 `fresh_isolated_subagent`；仅读 markdown packet，不继承前轮对话。

**并行 sidecar**：仅限有界批次，服务于当前活跃 gate。禁止：同时并行多个决策门、sidecar 直改 gate 文件。

## 磁盘布局

```
paper_ref/                          # 可复用的 target-journal 基准池
  TARGET_CONTRACT.md
  ref_pool_manifest_v<N>.md
  pdfs/001_<slug>.pdf … 020_<slug>.pdf

paper_review_v<N>/                  # 一轮审稿文件夹
  g00_target_contract_r1.md         # 仅追加，不覆盖
  g02_core_evidence_r1.md
  lanes/                            # 并行 sidecar 工作区
    g02_batch_a/lane_manifest.md
```

**Lane manifest 最小字段**：Main Gate、Batch Goal、Frozen Inputs、Lane Table（lane_id/kind/scope/owner/status/output）、Merge Back Rule、Stop Condition。

## 与 RFV 的区别

本协议通过 gate 文件 + freeze/backjump + lane scope 在**手稿**上强制深度。
RFV 通过 `verify_commands` + `EVIDENCE_INDEX` 在**代码**上强制深度。两者正交，不可混淆。

## 快速参考：`lane_kind`

`evidence_extract` / `citation_verify` / `figure_audit` / `table_audit` / `notation_audit` / `layout_audit` / `mirror_cleanup` / `prose_local` / `statistical_rigor` / `reproducibility_check`
