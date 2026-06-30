---
name: next-evolution-2026-06
description: 2026-06-30 全框架深度核查：已审计覆盖 vs 缺口分析 + 下一轮演化方向
metadata:
  type: reference
---

# 全框架深度核查：下一轮演化方向

> 核查日期: 2026-06-30
> 核查范围: core/ (27 crate, ~133K 行 Rust) + tests/ + configs/ + docs/ + skills/ (43 个)
> 核对面: 已完成的 13 轮审计 vs 代码库实际增量

---

## 一、已完成审计覆盖（13 轮）

| # | 审计 | 范围 | 状态 |
|---|------|------|------|
| 1 | 路由系统审计 | routing-engine, routing-core, routing-core、路由表 | ✅ 6 项架构债务记录 |
| 2 | Runtime 系统审计 | runtime-storage, trace-runtime, runtime-infra 等 | ✅ 132 项，7 修复 |
| 3 | Cross-crate dedup | 跨 crate 类型/函数重复 | ✅ 14 项，P0/P1 已修复 |
| 4 | Skill 路由对抗审计 | SKILL_ROUTING_RUNTIME.json + route 管道 | ✅ 87 项闭环修复 |
| 5 | v10 对抗审计 | v10 整体架构 | ✅ 16 项，含 AGENTS.md v9 残留 |
| 6 | Wiring 接线审计 | 全框架 hook 注册表与调用链 | ✅ 全部正常，3 项架构债务 |
| 7 | web_fetch dispatch | 工具路由 dispatch 断裂 | ✅ 已修复 |
| 8 | Task/Goal/Gates 对抗 | core-state, goal-engine, quality-gate | ✅ 33 项(P0:2)，全部修复 |
| 9 | Goal Engine Runner | goal-engine 内部 | ✅ 42 项，零 P0 |
| 10 | Skill Layer 对抗 | skill-layer crate | ✅ 30 项，P0/P1 已修复 |
| 11 | Task/Goal/Gates 二轮 | 全量 warning 清零 | ✅ 46 项闭环 |
| 12 | Good Skill 集成 | good-story/good-question 深集成 | ✅ 路由注册+重叠解决 |
| 13 | Goal 模型 DevExempt | goal 从未触发问题 | ✅ 已修复 |

**另有**：文档审计 (2026-06-26, 12 项全部修复)、goalx 注册对抗审核 (11 项修复)。

---

## 二、已覆盖 vs 未覆盖图谱

```
已覆盖区域（13 轮）                      未覆盖区域（9 个）
──────────────────────────────          ──────────────────────────────
✅ routing-engine                      ❌ framework-core（13K 行核心+策略巨块）
✅ routing-core                        ❌ framework-runtime（8.5K 行执行基础设施）
✅ skill-layer                         ❌ framework-extra（5.2K 行，含 34K 编排控制器）
✅ goal-engine (Task/Goal)              ❌ host-projection（15K 行宿主层）
✅ quality-gate (QG Route)              ❌ session-supervisor（4K 行 worker 生命周期）
✅ runtime-storage                      ❌ tool-routing-engine + mcp-tool-registry
✅ trace-runtime                        ❌ 测试质量/覆盖率分析
✅ runtime-core (部分)                  ❌ 端到端集成测试
✅ runtime-infra                        ❌ 性能基准与瓶颈扫描
✅ core-state + core-state-utils
✅ research-harness（501 tests）
✅ core-policy → 已合入 framework-core
✅ framework-kernel → 已合入 framework-core
✅ 文档体系（12 项已修复）
```

### 2.1 已覆盖但需跟进

| 区域 | 上次审计 | 跟进理由 |
|------|---------|---------|
| routing-engine | ~2026-06-25 | 路由表从 43→42，需要验证是否清洁 |
| research-harness | 审计外围但非核心 | 最大单 crate (25K 行)，501 tests 但 GateChecker 适配层未独立审计 |
| docs/ | 2026-06-26 | 一月后需刷新 last_verified 和架构引用 |

---

## 三、下一轮演化方向：9 个未覆盖领域

### 🥇 P0（最优先）

#### [P0-1] framework-core 重构：kernel+policy 巨块分解

**现状**：13K 行，49 个模块文件，从 core-policy + framework-kernel 于 2026-06-29 合并而来。

**风险文件**：
- `hook_policy.rs` — 1502 行，67 个函数，`HookPolicyEvaluateRequest`/`Response`
- `cli_args.rs` — 1693 行，43 个函数，所有 CLI 参数定义
- `registry_review_gate.rs` — 475 行，review gate 注册表
- `review_gate_engine.rs` — 405 行，review gate 状态机

**问题**：一次合并导致一个大 monolith。`hook_policy` 和 `cli_args` 单文件超 1500 行，边界模糊（review gate / CLI / policy 混合在同一 crate）。Review gate 引擎在 framework-core 但 review gate 的 MCP 工具在 runtime-core → 概念分层混淆。

**建议方向**：
1. `cli_args` 抽出独立 `framework-cli-args` crate（或拆入 router-rs）
2. `hook_policy` 拆入 `framework-policy`（或保持但文件切分）
3. `review_gate_*` 相关模块应评估是否归入 runtime-core（与 QG Route 同层）
4. `goal_auto_detect` → 评估是否移入 goal-engine
5. 目标：framework-core 减至 6K-8K 行，文件最大不超过 800 行

#### [P0-2] framework-runtime 审计：v9 执行基础设施残留

**现状**：8.5K 行，2026-06-29 从 fr-utils + fr-contracts + fr-exec 合并而来。**未经任何独立审计**。

**风险模块**：
- `live_execute.rs` — 1247 行（43K 函数），实时执行引擎
- `runtime_view.rs` — 1244 行，运行时视图/TUI
- `trace_attach.rs` — 1008 行，trace 附加
- `trace_stream_io.rs` — 1192 行，trace 流 IO
- `trace_transport.rs` — 872 行，trace 传输层
- `execution_contract.rs` — 1084 行，执行合约
- `pre_tool_use_guard.rs` — 778 行，工具使用前守卫

**风险等级**：**高**。这些是 v9 `runtime_execution_layer` 的直接残留，与 v10 的 goal-engine + runtime-core 架构可能存在大量功能重叠或矛盾。trace 系统（4 个模块，~4300 行）和 `live_execute` 尤其可疑——它们可能不再被实际使用，或者被 runtime-core 中的新实现取代。

**建议方向**：
1. **全面追溯引用**：对 `live_execute`、`runtime_view` 和各 `trace_*` 模块进行 100% 引用审计
2. 确认不用的模块 mark `#[deprecated]` + 安排退场
3. 验证 execution_contract 与 goal-engine 的 QGEntry 防欺诈门是否冲突
4. 目标：framework-runtime 至少减至 4K-5K 行

#### [P0-3] framework-extra 审计：编排控制器与 closeout 边界

**现状**：5.2K 行，30 个单元测试。未经审计。

**风险模块**：
- `orchestration_controller.rs` — 975 行，背景工作编排（batch_plan / enqueue / interrupt / claim / complete / retry / session_release）
- `alias.rs` — 769 行，高级别名解析
- `framework_doctor.rs` — 829 行，健康诊断
- `evidence.rs` — 624 行，证据写入
- `contract_summary.rs` — 380 行，合约摘要

**问题**：`orchestration_controller` 处理背景 worker 编排，但 session-supervisor 也处理 worker 生命周期 → 边界模糊。`framework_doctor` (829 行) 可能应当移到 router-rs CLI 层。

**建议方向**：
1. 编排控制器 → 移入 session-supervisor 或评估是否仍须存在
2. framework_doctor → 移入 router-rs CLI 命令
3. closeout + evidence (833 行) 保留，但应增强测试覆盖
4. 目标：framework-extra 减至 3K 行

---

### 🥈 P1（高优先）

#### [P1-1] 测试质量深度审计

**现状**：CI 有覆盖率门限（≥60%），但未对测试质量本身进行审计。

**发现**：
- 仅 `core-state` 使用 proptest 属性测试（1 个文件，~170 行）
- 其他 26 个 crate 零属性测试
- `quality-gate` (508 行) 仅 7 个测试，但 10 个 GateChecker 实现无独立测试
- `framework-extra` (5.2K 行) 仅有 30 个测试
- `skill-layer` (2.7K 行) 仅有 16 个测试
- **无任何端到端集成测试**练习完整的"skill route → execute → evidence → closeout → QG verdict"管道
- Fuzz 仅覆盖 stdio 解析和 JSONL 维护，未覆盖 goal-engine、routing-engine、QG Route

**建议方向**：
1. 为 quality-gate 全面补测：为每个 `GateChecker` 实现编属性测试
2. 为 goal-engine 增加 proptest：状态转移、并发锁、kill switch、drift
3. 增加 1-3 个端到端集成测试：`tests/e2e/` 目录，mock stdio 执行完整 goal 生命周期
4. 新增 2-3 个 fuzz target：routing_decision 已有但可以扩展，QG evaluation 为候选
5. 为 framework-runtime trace 模块增加数据流测试

#### [P1-2] 宿主层审计：host-projection + session-supervisor

**现状**：host-projection (15K 行, 115 tests) + session-supervisor (4K 行, 49 tests) 均未独立审计。

**风险点**：
- host-projection 的 hooks.rs (799 行) 持有 `RuntimeHooks` struct (~30 fn 指针字段) — 这是跨层通信的心脏
- host_entrypoint_sync.rs (557 行) — 主机入口同步逻辑
- host-projection 的 build.rs 代码生成逻辑
- session-supervisor 的 team_manager.rs (558 行) — 团队管理器
- process.rs (355 行) + worker.rs (404 行) + driver.rs (150 行) — 进程生命周期

**建议方向**：
1. Host layer 架构审查：是否 4 种宿主 (claude/cursor/codex/opencode) 的抽象仍有效
2. hooks.rs 与 runtime-core init_hooks() 的双向引用验证
3. session-supervisor 的测试覆盖率增强（test.rs 42K 行但有待确认覆盖广度）
4. MCP stdio bridge 路径审计

#### [P1-3] 工具层审计：tool-routing-engine + mcp-tool-registry

**现状**：tool-routing-engine (1.8K 行, 43 tests) + mcp-tool-registry (1.4K 行, 29 tests)。**路由层审计覆盖了 skill route 但未覆盖 tool route**。

**建议方向**：
1. 8 步 tool 评分管道审计：与 16 步 skill 评分管道的一致性
2. MCP_TOOL_REGISTRY.json (40 工具) 完整性验证：每个工具在代码中确实有 handler
3. 工具 dispatch 路径：route_tool() → dispatch 的完整路径验证
4. MCP 服务器二进制 (rust_tools/ 中的 citation/financial/pdf/pptx/ooxml/browser) 的安全审计

---

### 🥉 P2（中等优先）

#### [P2-1] 性能基准与瓶颈扫描

**现状**：CI 有 Criterion benchmark（6 个），回归门限 5%。但**从未进行过系统性性能分析**。

**建议方向**：
1. 执行一次 `perf` / `flamegraph` 扫描，识别热点
2. 分析 cold start 时间分布（哪些 crate 编译最慢、启动最慢）
3. 评估 runtime-storage 的 SQLite vs FS 存储性能
4. 大规模 routing 表（43 skills × 4 hosts）的 route 延迟分析

#### [P2-2] 安全专项审计

**现状**：代码中有 `web_fetch_guard.rs` (SSRF 防护)、`tool_safety_rules.rs` (工具安全规则)、`pre_tool_use_guard.rs` (工具使用前守卫)。但**没有专门的安全审查轮次**。

**建议方向**：
1. MCP 服务器二进制 (citation_tool_rs, financial_data_rs, gh_source_gate_rs 等) 的注入/命令执行面审查
2. web_fetch SSRF 防护有效性验证
3. 文件路径遍历审查（特别是 core-state-utils 的路径处理）
4. 环境变量注入审查

#### [P2-3] Skill 内容质量评估

**现状**：42 个路由技能，部分由外部贡献 (good-story/good-question)。SKILL.md 的声明式质量未经验证。

**建议方向**：
1. 遍历 42 个 SKILL.md，验证 `scene`、`trigger_hints`、`metadata.platforms` 的实际准确性
2. 验证每个技能的 `When to use` / `Do not use` 是否具体可操作
3. 识别低频/零使用技能，考虑折叠或降级

---

## 四、演化优先级排序

```
轮次   领域          优先级   预估工作量    关键理由
───   ─────         ────    ────────    ─────────────────
 1    framework-core   P0     ~3-5 天    最大 monolith，分层混淆
 2    framework-runtime P0   ~3-4 天    v9 执行层残留，未经审计
 3    framework-extra   P0     ~2-3 天    编排控制边界模糊
 4    测试深度审计      P1     ~3-5 天    质量门短板，属性测试缺位
 5    宿主层审计        P1     ~2-3 天    跨层通信核心
 6    工具层审计        P1     ~2-3 天    routing 审计未覆盖
 7    性能扫描          P2     ~1-2 天    长期维护基础
 8    安全审计          P2     ~1-2 天    可做但非紧急
 9    Skill 质量评估    P2     ~1-2 天    渐进改善
```

**推荐启动顺序**：P0-1 → P0-2 → P0-3 → P1-1 → P1-2/P1-3（并行）→ P2 系列。

---

## 五、关键数据汇总

| 指标 | 值 |
|------|-----|
| 总 Rust 代码行 | ~133K |
| Core crate 数 | 27 |
| 技能数 | 43 (路由表: 42) |
| 总测试数 | ~2,800 (#[test]) |
| 属性测试 (proptest) | 1 个 crate (core-state) |
| 性能基准 | 6 组 Criterion |
| Fuzz targets | 5 |
| CI 工作流 jobs | 12 |
| 已审计轮次 | 13 |
| 未审计 crate |  framework-core, framework-runtime, framework-extra, host-projection, session-supervisor, tool-routing-engine, mcp-tool-registry |

---

## 六、执行记录（2026-06-30）

### 轮次 1 执行结果
- framework-core: **无死模块**（探索代理引用追溯错误，漏查跨 crate 消费者）
- 实际清理：移除 14 个死 re-export + 内联 env_flags.rs
- 编译通过，测试通过

### 轮次 2 执行结果
- 新增 25 个测试：runtime-core checkers (15) + framework-extra closeout (10)
- 全量测试通过（预存 4 个 research-harness 失败 + 2 个路由测试失败）

### 轮次 3 执行结果
- Skill 扫描：3 P0 + 12 P1 + 10 P2
- 已修复：sentry scene 不匹配
- good-story/good-question 质量与英文技能同水平
