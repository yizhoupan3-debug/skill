# Harness 减法与第一性原理 — 全面自检清单

**目的**：按问题级条目自查连续性 harness（skill 热路由、L3 `router-rs`、L4 hook 投影、L2 工件、文档真源），标记冗余、token 税、补丁气质与抽象边界。**不重复**长叙事：五层模型与数据流以 [harness_architecture.md](../harness_architecture.md) 为准；跨宿主执行协议见根目录 [AGENTS.md](../../AGENTS.md)；新宿主接入见 [host_adapter_contract.md](../host_adapter_contract.md)。

**何时用**：季度减法评审；大改 harness / review gate / SessionStart 注入前；接入或升级某宿主后复盘。

**与 PR 短清单关系**：合并 harness 相关改动时，**先**过合并门槛与十行表 [EXECUTION_harness_pr_review_checklist.md](EXECUTION_harness_pr_review_checklist.md)；解绑 / 沉降验收见 [EXECUTION_harness_decouple_sink_checklist.md](EXECUTION_harness_decouple_sink_checklist.md)；本页用于**深入、全盘**审计与 issue 登记，不替代该 PR 门槛。

**Finding 极简模板**（粘贴到 issue 或文末）：`条目 id | 是/否说不清 | 证据（路径/命令/截图）| P0/P1/P2`。

---

## A. 真源与漂移（单一事实会不会坏掉）

### A1. `AGENTS.md` 与各宿主文案

- [ ] **AGENTS.md 与 Cursor `alwaysApply` rules**：是否存在大段复述（语言、Execution Ladder、Review gate、Git 禁令等），且两处未同步更新的风险可被指出具体段落？
- [ ] **Codex：`AGENTS.md` 嵌入快照**：`codex sync` 是否与磁盘 `AGENTS.md` 的流程写清楚，避免出现「Codex 里仍是旧策略」的操作疏忽？
- [ ] **`AGENTS.md` 与 `docs/harness_architecture.md`**：`AGENTS.md` 是否只做跨宿主不变量，长叙事仍收敛到 harness 文档？

### A2. 路由与 Skill 索引

- [ ] **`skills/SKILL_ROUTING_RUNTIME.json` 是否为唯一热路径**：宿主、规则、skills 正文是否任何地方仍鼓励「按 slug 猜路径」或扫全 `skills/`？（参见 PR 清单 #2）
- [ ] **热 runtime 与冷面**：plugin catalog / routing metadata / runtime explain 是否被误塞进热加载路径或会话注入？
- [ ] **`SKILL_MANIFEST.json` fallback**：fallback 触发条件、`full_skill_count` / `hot_skill_count` 与 `scope.policy` 是否与实际路由行为一致？
- [ ] **`skill-compiler-rs` 产出**：编译后 JSON 与 `SKILL.md` 源是否可能不同步？CI 或流程是否防 drift？

### A3. 宿主注册与安装真源

- [ ] **`configs/framework/RUNTIME_REGISTRY.json`**：`host_targets.supported`、metadata、install_tool、host_entrypoints 是否为闭集宿主唯一入口，仍存在硬编码宿主列表分叉？
- [ ] **安装投影 vs 手写 hooks**：仓库内 `.cursor/hooks.json` 等与 `framework install/sync` 生成物是否会长期双边维护？（参见 PR 清单 #8）

---

## B. 上下文 / Token（每轮看不见的税）

### B1. 固定注入（Rules / System）

- [ ] **alwaysApply rules 数量与体积**：每一条是否仍「必须全程注入」，或可改为 globs / 按需规则而不损安全？
- [ ] **重复的 Plan 契约**：`skills/plan-mode` 与 `.cursor/rules/cursor-plan-output.mdc` 是否可收敛为单一真源 + 另一侧仅 pointer？
- [ ] **`agent_skills` / 用户 skills 清单与仓库 runtime**：是否在规则或文档里写死优先级，避免两套「可选用 skill」并排？

### B2. SessionStart / 开场注入

- [ ] **与各宿主契约对齐**：Codex SessionStart 是否遵守「只动态活信息」、禁 repo onboarding / Quick Reference 等（harness_architecture §2.1）？（参见 PR 清单 #9）
- [ ] **Cursor / 其它通道**：是否存在「非 SessionStart 通道」但仍每轮附带静态 Quick Reference 或等价大块？若有，是否与文档声明一致？
- [ ] **`ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX`（或等价变量）**：默认值与 clamp 是否合理，是否与真实 prompt 构造路径一致？

### B3. Hook 出站：`followup_message` / `additional_context`

- [ ] **多事件重复注入**：是否在多个生命周期事件上对同一语义（Goal/RFV/Review）重复投影？（意图：合并、短码优先；参见 PR 清单 #3）
- [ ] **ADVISORY 累积**：AUTOPILOT drive、RFV continue、paper adversarial、pre-goal、`operator nudges` 等是否在「全开」下明显膨胀？
- [ ] **`ROUTER_RS_OPERATOR_INJECT` 聚合闸**：是否所有 advisory 路径都尊重该闸？是否有漏网支线？
- [ ] **Codex `additional_context` 去重与截断**：去重与字符上限截断是否在「信息量」与「可诊断性」上可接受？截断会否静默丢关键门禁句？

---

## C. L3（`router-rs`）设计与负债

### C1. 模块边界与重复逻辑

- [ ] **Codex hooks vs Cursor hooks**：review / delegation / subagent / override / reject_reason 等行为是否在两边存在可辨识的平行实现？差异是否仅用「宿主 JSON 形状」解释？
- [ ] **`hook_common` 覆盖度**：门控启发式是否在 L4 bash 仍存在复制（[host_adapter_contract.md](../host_adapter_contract.md) 反模式）？
- [ ] **CLI 分发面**：`dispatch` / `dispatch_body.txt` 是否每次新子命令都要求多处同步更新？有无遗漏测试？

### C2. 防御性补丁（是否仍必要或可缩小）

- [ ] **Cursor stdin 误入 Claude**：顶层字段静默策略是否仍能覆盖已知误接场景，且无过度误杀？（harness_architecture §4.1）
- [ ] **伪造宿主 followup 行（如 RG_FOLLOWUP）**：`scrub` 是否在最小够用集合上？是否文档化「防的是哪类攻击面」？
- [ ] **`prompt_compression`**：是否与真实大包 prompt 源头治理重复？谁在什么路径依赖它？

### C3. 复杂度与管理成本

- [ ] **`cursor_hooks` / `browser_mcp` 分片**：`include!` / fragment 拆分是否改善了可读性，还是仅为控制单文件体积？
- [ ] **`route/` 细分**（policy、scoring、signals…）：是否与「路由输出契约」对齐？是否存在仅为结构而拆的中间层？
- [ ] **Trace vs Evidence vs Step ledger**：三者写入方、读取方、非目标是否与文档一致且无用途 creep？（参见 PR 清单 #4）

---

## D. Skill 层（L5）与宿主规则（L4）

### D1. Skill 正文 vs 宿主

- [ ] **`host_support.platforms` / catalog**：是否与 `RUNTIME_REGISTRY` 闭集一致？
- [ ] **Framework command skills**（如 `/autopilot`）与普通 skill：在 runtime 表里 `kind`、门控、`session_start` 等字段是否一致且无例外魔法？

### D2. 规则与 skill 的职责

- [ ] **Cursor-only 规则**：是否严格只保留「宿主独有硬约束」，没有复制 `AGENTS` 长篇？
- [ ] **Review-only / Execution subagent gates**：是否与 `review_gate`、`hook-state`、Stop 短码链路一致且无「规则说一套、hook 另一套」？

---

## E. 状态与磁盘副作用（一致性 / 异常路径）

### E1. 多宿主状态路径

- [ ] **`.cursor/hook-state`**、**`.codex/hook-state`**、**Claude `.claude/hook_state_*`**：清理、迁移、并行工作区是否会遗留僵尸状态？
- [ ] **`flock` / 持久化失败**：各宿主在 lock 失败、写盘失败时的降级文案与行为是否一致、可观测？（参见 PR 清单 #3 硬门禁 vs advisory）

### E2. L2 连续性

- [ ] **`TASK_STATE` 与 `STEP_LEDGER`**：是否保证 ledger 全文不会误入模型上下文，仅摘要投影？
- [ ] **`GOAL_STATE` / `RFV_LOOP_STATE`**：谁在写、谁在合并进 Stop / hydration，路径是否单一？
- [ ] **`EVIDENCE_INDEX`**：追加形状、closeout / digest 消费者是否仅此一处真源？
- [ ] **`TRACE_EVENTS`**：是否与 `EVIDENCE_INDEX` 发生「抢着当真相」的用法？

---

## F. Closeout / Depth / Failure taxonomy

- [ ] **`ROUTER_RS_CLOSEOUT_ENFORCEMENT`**：本地软 / CI 硬是否与团队预期一致？
- [ ] **Depth：`ROUTER_RS_DEPTH_SCORE_MODE`（legacy vs strict）**：是否与 eval、task_state rollup、operator 文案一致？（参见 PR 清单 #6）
- [ ] **`HARNESS_FAILURE_TAXONOMY` / behavioral eval**：是否仅用于分类与评估而不变成第二套路由？（参见 PR 清单 #7、`HARNESS_BEHAVIORAL_EVAL_CASES.json`）

---

## G. 环境与开关矩阵

- [ ] **开关是否仍可合并**：多个 `ROUTER_RS_*` 细分开关是否在文档中有「优先级 / 聚合闸」图示？（参见 PR 清单 #6、harness_architecture §5）
- [ ] **默认开/关是否合理**：paper adversarial、pre-goal、review gate disable 等对「新克隆仓库开箱体验」是否符合预期？
- [ ] **`ROUTER_RS_HARNESS_OPERATOR_NUDGES` 与各子开关**：关系是否可查、可测？

---

## G2. Observability（可观测性与排障）

- [ ] **`router-rs framework snapshot` / `contract-summary`（或等价）**：新成员能否仅靠命令还原「当前 task / digest / continuity」？
- [ ] **Hook 出站 JSON**：关键门禁失败时，`reason` / `decision` 是否足以区分「宿主 bug / 状态损坏 / 用户触发条件」？
- [ ] **日志与环境**：是否在敏感路径刷屏或输出过大上下文片段？

---

## H. Host integration / bootstrap

- [ ] **跨仓库接入（根 README checklist）**：与 `RUNTIME_REGISTRY`、install 路径是否同步？
- [ ] **`GENERATED_ARTIFACTS` / 必填生成物列表**：新增生成物是否必登记，避免静默漏装？（参见 PR 清单 #8）
- [ ] **`host_entrypoints_sync_manifest`**：Codex sync 后与真实入口是否一致？

---

## I. 文档与仓库卫生（不参与运行但仍拖累 harness）

- [ ] **`docs/plans`、`docs/history` 暴增**：哪些是 SSOT，哪些仅为考古？新人从哪一页进 harness？
- [ ] **「唯一长解释」是否仍是 `harness_architecture`**：其它文档是否在重复五层叙事？
- [ ] **PR checklist 与路线图**：是否与 [EXECUTION_harness_pr_review_checklist.md](EXECUTION_harness_pr_review_checklist.md)、[harness_improvement_backlog.md](harness_improvement_backlog.md) 分工清楚？

---

## J. First principles / Subtraction gates（逐项「该不该存在」）

- [ ] **该能力是否产生可验证 L1/L2 证据？**若无，是否必须从 hook 默认值移除或改为 opt-in？
- [ ] **该文案是否可被指针替换为单行「见 AGENTS §x / harness §x」？**
- [ ] **该 ENV 是否真的改变边界行为？**若只改文案措辞，是否应删掉？
- [ ] **该状态是否可由更少文件表达？**
- [ ] **该抽象是否在 3 个宿主上出现第 4 份拷贝？**若是，是否需要薄共享层而非继续粘贴？

---

## K. Git / 协作与 Harness 交集

- [ ] **AGENTS / Git 禁令**（不设分支/worktree）：与 autoflow（`/gitx`）叙事是否冲突或需显式豁免说明？
- [ ] **dirty 工作区 closeout**：是否遵守「如实报告不顺手清理」且不假装干净？（参见 PR 清单 #5）

---

## L. CI / Tests

- [ ] **`cargo test`（router-rs + 根包 policy）**：与 `RUNTIME_REGISTRY` / hosts / skills 契约是否绑死？
- [ ] **`tests/policy_contracts.rs`（若适用）**：registry 缩水夹具是否与真 registry 漂移？
- [ ] **Host integration tests**：新增宿主是否真的跑过 install dry-run？
- [ ] **防回归**：SessionStart token 上限、inject 闸门是否有回归测？（参见 PR 清单 #7、#10）

---

## M. Supply chain / 外部依赖（如涉及）

- [ ] **`router-rs` 依赖**：是否有过重依赖仅为小功能可考虑内联或删？
- [ ] **MCP / Browser**：可选路径是否在关闭时完全不增加 cold start（含规则/钩子）？

---

## 使用说明（建议节奏）

1. **一轮全盘**：自 **§A** 顺序扫到 **§M**；**「否」或「说不清」**即记为一项 finding。
2. **高收益减法子集**：仅 **§J + §B + §A2**（政策重复、会话注入税、路由真源）。
3. **宿主专项**：新开 Cursor/Codex/Claude 能力时重做 **§C、§E、§H**。

---

## 附录：合并 PR 时十字引用

| PR 清单 # | 主对应节 |
|-----------|----------|
| 1 | A、I |
| 2 | A2 |
| 3 | B3、E1 |
| 4 | C3、E2 |
| 5 | K、F |
| 6 | F、G |
| 7 | F、L |
| 8 | A3、H |
| 9 | B2 |
| 10 | L、rollback 心智 |
