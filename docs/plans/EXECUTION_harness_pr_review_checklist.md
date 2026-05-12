# Harness 面 PR 审查清单

**相关**：深入、全盘自检（减法 / 第一性原理问题清单，与本条合并门槛互补）见 [harness_subtraction_first_principles_audit_checklist.md](harness_subtraction_first_principles_audit_checklist.md)。

长解释与分层模型见仓库根 [`docs/harness_architecture.md`](../harness_architecture.md)。本文件只列 **harness 相关改动** 的合并门槛与可勾选表，不重复 `AGENTS.md` 全文。

## 合并门槛（五条）

1. **失败形态**：能一句话说清防止或修复的坏情况（可选附 issue/复盘链接）。
2. **单 owner**：主改动落在唯一真源（单文件或单一目录）；需双处同步时写明 lead。
3. **验证**：有 `cargo test` 过滤子串或一条可复现命令；纯文档须声明无行为变更风险。
4. **热路径**：若动 SessionStart / hook 注入，须更短或更固定；增长须写明预算策略。
5. **第二真源**：不新增与 `SKILL_ROUTING_RUNTIME.json`、L2 证据链或现有 schema 平行的叙事/状态。

## 检查表（十行）

| # | 检查项 | Done when |
|---|--------|-----------|
| 1 | 与 `harness_architecture.md` L1–L5 与数据流一致 | 矛盾已改文档或改代码 |
| 2 | 热路由未向 `SKILL_ROUTING_RUNTIME.json` 塞冷数据 | diff 无 explain/plugin 膨胀 |
| 3 | 硬门控走约定字段；advisory 不冒充硬门禁 | 对照 harness_architecture 投影节 |
| 4 | `TRACE_EVENTS` / `STEP_LEDGER` / `EVIDENCE_INDEX` 职责未混用 | 边界清晰 |
| 5 | closeout 仍由证据驱动 | 无「叙述即完成」弱化 |
| 6 | 新 `ROUTER_RS_*` 真改变行为边界且默认安全 | 对照 harness_architecture 开关表 |
| 7 | 新分支或失败类有测试或 eval case | 可指认测试名或 `HARNESS_BEHAVIORAL_EVAL_CASES` id |
| 8 | 工具/契约变更已同步 `configs/framework` 或宿主 `hooks.json` | 已同步或写明不适用 |
| 9 | 模型可见注入未带回长静态说明 | 对照 SessionStart 禁令 |
| 10 | 回滚本 commit 后仓库仍自洽 | 无隐藏应用顺序依赖 |

## 行为 eval 索引

机器可读用例表：[`configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json`](../../configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json)。改 harness 行为时优先对照其中 `verify` 命令是否仍通过。
