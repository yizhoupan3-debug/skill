---
last_verified: "2026-06-07"
depends_on:
  - reasoning-depth-contract.md
  - lane-templates.md
  - external-research-harness.md
  - ../../spec.md
---

# 数理推理强度 harness（STEM）

> **status: done** — 语义层（witness / 双轨 / RFV lane）仍为契约真源；**Rust 运行时 checker**（`framework_math_verify`，roadmap v4 phase 5b N1–N4）已迁 **B0 `core/core-math/`**（`formal_toolchain`；历史路径 `core/framework-core/src/math_verify/` 已退役），GOAL depth advisory rollup 已接线（N6），见 **§H**。RFV 多轮 loop 在 `my-light` profile 下仍很少使用；§D–G 编排语义以 aspirational 为主。

**语义层真源**：与 [推理深度契约](reasoning-depth-contract.md) 同层（L5）；**运行时**仍只认 **L1 可执行验证 + L2 证据落盘**，不把自然语言「像证明/像建模」当作通过标准。

**编排入口**：多轮 RFV 见 [lane-templates.md](lane-templates.md) 中的数理专项 lane；My 执行区（`/implementx` 等）须在 **Goal 契约**里写明 **双轨/脚本级** `validation_commands`。[`spec.md`](../../spec.md) 中的 `verify_commands` 与 **`EVIDENCE_INDEX`** 规则同样适用。

| 场景 | 契约节 | RFV 要点 |
|------|--------|----------|
| 证明 / 推导 / 审证 | §A–C | witness + checker |
| 结构 / 猜想探索 | §D | discovery → promotion → 证伪 |
| **数学建模**（现象→方程→检验） | **§F** | `model_spec` + 量纲/退化 witness + 数值轨 |
| **未知性质 / 找数学背景** | **§G** | `theory_background` + 外研 strict + 类比失效面 |

---

## A. 可检验中间对象（intermediate witnesses）

把题目拆成若干 **可独立核对** 的小命题；通过标准是 **特例一致、极限相容、无明显矛盾**，不是「读起来顺」。

| 机制 | 要求 |
|------|------|
| **量纲 / 退化极限 / 特例** | 在 handoff 或 `goal` 附件中列出 **Witness 清单**（例如 \(t\to 0\) 标度、对称性、边界情形应满足的阶或常数）。 |
| **区间与误差** | 主结论须标明 **误差阶 \(O(\cdot)\)、显式区间或常数上界**，并标注 **每一步依赖的假设**（哪怕只能定性）。 |
| **双轨对照** | **解析/代数轨** 与 **数值或枚举轨** 并行：数值轨须给 **固定种子、容差、对照协议**（Monte Carlo / brute-force 小范围）；`verify_commands` 中至少一条可复跑脚本。 |

**Review 收窄**：本轮 reviewer 只做 **假设—结论逐项对照** + **与 witness 清单一致性**；不在本轮扩写完整证明散文。

**Counterexample lane**（只读）：专门寻找与 witness 或主结论矛盾的实例；发现矛盾则 **FAIL**，写入 `review_summary` 并驱动 fix。

---

## B. 符号层 verifier（CAS / SMT / 证明助手）

Harness **只认 checker 输出**（exit code + 约定 stdout/stderr），不认「写作风格像定理」。

| 工具类 | 适用 | PASS 条件 |
|--------|------|-----------|
| **SymPy 等 CAS** | 恒等变形、求导、化简 | 脚本以 0 退出且输出与 golden 或自洽检查一致 |
| **Z3 / SMT** | 小范围可行性、不变式 | 输出 `sat`/`unsat` 等与契约一致 |
| **Lean / Coq** | 仅在团队已有模板与 CI 成本可接受时 | `lake build` / `coqc` 无 sorry |

**升级顺序**：CAS → SMT → ITP；任一层给出 **显式反例** 即 **FAIL**，优先记入 `append_round` 与 `EVIDENCE_INDEX`。

### B.1 Rust 统一入口（`framework_math_verify`）

stdio 命令 **`framework_math_verify`**（`router-rs` runtime_ops；实现 `core/core-math/`）提供与 §A–C 对齐的可编程 checker，schema `framework-math-verify-v1`：

| `operation` | 模块 | 说明 |
|-------------|------|------|
| `status` | — | 报告各 backend 可用性（`dimension_checker` 纯 Rust；SymPy/Z3 子进程） |
| `dimension_check` | N2 | `lhs` / `rhs` 量纲向量对照（§F 量纲 witness） |
| `formal_verify` | N1 + N3 | `backend`: `sympy` \| `z3`；`request`: `FormalVerifyRequest`（见 `core/core-math/`） |
| `step_verify` | N4 | `request`: 编号步骤链 + 可选逐步 `formal_check`（§C 依赖图） |

**`FormalVerifyRequest.kind`**：

| kind | backend | PASS |
|------|---------|------|
| `identity` | sympy | `lhs - rhs` 化简为 0 |
| `simplify_to_zero` | sympy | `expression` 化简为 0 |
| `smt_check` | z3（强制） | stdout `sat`/`unsat`/`unknown` 与 `expected_smt_status` 一致 |

**`StepVerifier`**（N4）：校验 `step_id` 唯一、依赖仅指向前序步骤、未满足依赖即 **FAIL**；带 `formal_check` 的步骤按 kind 路由 SymPy 或 Z3；可选 **`dimension_check`**（N2 witness）与 **`conclusion`**（量纲字符串，后续步可用 `@step_id` 引用）。整体 `verdict`: `pass` \| `fail` \| `error`。

**深度 advisory 信号**（**不**直接改写 `depth_score`，见 [reasoning-depth-contract.md](reasoning-depth-contract.md)）：

- **`formal_verify_depth_signal`**（N3）：`FormalVerifyResult` 在 `verdict=pass` 且含 `smt_status` 时贡献 `1`（SymPy 纯符号 pass 为 `0`）。GOAL rollup 调用同名函数，**不**读取 result 上的 `depth_signal` 字段。
- **`step_verify_depth_signal`**（N4）：每步 formal pass 计 `1`，叠加该步 `formal_verify_depth_signal`。
- **GOAL rollup**（N6）：可选 `GOAL_STATE.math_verify_formal_results[]`（`FormalVerifyResult` 形状）由 `depth_compliance_aggregate` 汇总为 `DepthCompliance.math_verify_formal_depth_signal`（advisory）。

示例（SymPy 恒等式）：

```json
{ "operation": "formal_verify", "backend": "sympy",
  "request": { "kind": "simplify_to_zero", "expression": "sin(x)**2 + cos(x)**2 - 1", "variable": "x" } }
```

示例（Z3 矛盾 unsat + 步骤链）见 `core/core-math/` 内测试 fixture。

**证据落盘两条路径**（与 [`spec.md`](../../spec.md) 一致）：宿主 **`PostTool`** 在启发式命中时自动追加一行到 `EVIDENCE_INDEX`（`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` 未关、连续性就绪）；**`framework hook-evidence-append`** 供长尾命令显式记账（非 `cursor_*` 来源时仍走同一验证启发式，含 SymPy / Z3 / Lean / Coq 等 **窄子串**，见 `router-rs` `framework_runtime`）。数理脚本请避免仅写裸 `python` 作为唯一可识别串。`framework_math_verify`  stdout 结果可另写入 `GOAL_STATE.math_verify_formal_results` 供 depth advisory rollup。

---

## C. 挡「似是而非推导」

| 机制 | 做法 |
|------|------|
| **逐步依赖图** | Fixer 交付 **编号步骤表**：每步 = 结论 + **引用的引理/步骤编号**。Reviewer **只攻击**「本步是否引入未证依赖」。运行时对照：**`framework_math_verify` → `step_verify`**（N4）可机读校验 `depends_on` 与前向引用；每步可选 `formal_check` 走 N1/N3。 |
| **反事实探针（数理 fuzz）** | 独立只读 lane 使用 **错误代入 / 错误极限顺序**；主答须 **拒错前提** 或推出矛盾。**盲从** → 本轮记 `probe_failed`，不得标为通过。 |

---

## D. 结构探索（discovery → promotion → 证伪）

本节补 **「探索新结构/性质」** 的 harness 语义；**运行时 PASS/FAIL 不变**——探索产出在 promotion 之前一律为 **`open`**，升格后仍须走 §A–C 与 Verifier。

### 入口（二选一，勿混宏）

| 用户意图 | 入口 | 说明 |
|----------|------|------|
| 单轮证明/推导/审证 | [`$math-derivation`](../../../skills/math-derivation/SKILL.md) | witness + 自检；不需 RFV |
| 多轮：猜想、结构探索、文献+证伪 | `framework_rfv_loop` + [lane-templates.md](lane-templates.md) §D | **勿**与同 task 活跃 `GOAL_STATE` / `/implementx` 并行 |
| **数学建模**（建模型、定方程、量纲、机制） | [`$research-execution`](../../../skills/research-execution/SKILL.md) → RFV **`external_mode=modeling`** 或 **`STEM_MODEL_FORMULATOR`** | §F；实现/仿真脚本走 fix + verify |
| **深挖未知性质 / 补数学背景** | RFV **`external_mode=math_background`** + External strict | §G；与 §D `conjecture_list` 可同轮但分字段 |
| 项目级方法/实验+数学 | `research-execution` 多 lane 组合 | 建模 §F + 背景 §G + 必要时 §D |

### 单轮 RFV 内子阶段（禁止叠「Round A′」）

1. **只读并行（≤3 路）**：`Reviewer ‖ External（stem_discovery 或 deep+conjecture_list）`；可选第三路 `STEM_CONJECTURE_EXPLORER`。
2. **Supervisor promotion**（**非** subagent）：筛 `promoted | open | rejected`；`promoted` → 填 `WITNESS_LIST` + 检验意图草案。
3. **只读并行**：`STEM_WITNESS_REVIEWER ‖ COUNTEREXAMPLE ‖ ADVERSARIAL_PROBE`（模板见 lane-templates）。
4. **Fix**（仅 promoted 相关范围；代码/脚本修复走 RFV `fix_scope` **或** 显式 `/implementx`，二选一）→ **Verifier** 跑 `verify_commands` → **一次 `append_round`**。

**硬门**：promotion **不能**替代 verify；`probe_passed` **不是**「PRV 通过」；外研/猜想 **不能** 在无 `EVIDENCE_INDEX` 成功行时 close。

### 落盘决策表（避免与外研/证伪重复）

| 内容类型 | 放哪里 | 何时写 |
|----------|--------|--------|
| 文献主张、矛盾扫描、检索轨迹 | `external_research` / External lane `claims` | Phase A，strict 时走 schema |
| 候选结构/引理/不变量（未升格） | External `conjecture_list`（lane 或 `external_research` JSON，见 `RFV_EXTERNAL_RESEARCH.schema.json`）或 CONJECTURE lane；promotion 前 `status: open` | Phase A |
| promotion 后待执行检验 | `falsification_tests[]`（`append_round`，`RFV_FALSIFICATION_TESTS.schema.json`；Rust 非空时校验） | promotion 后、verifier 前起草；verifier 后带结果 |
| 已构造反例或推翻 | `adversarial_findings[]` | Counterexample / 证伪成功后 |
| 可执行 PASS/FAIL | `verify_result` + `EVIDENCE_INDEX` | Verifier 轮 only |

`depth_score` **不认**猜想条数；strict 第三分仍看 `falsification_tests` 等（见 [reasoning-depth-contract.md](reasoning-depth-contract.md)）。**不要**为 discovery 新增 `conjecture_candidates` 计分或改 Rust rollup，除非已有真实任务证明缺口。

### 与非 discovery 的边界

- **Witness 清单真源**：promotion 后由 supervisor 写入 goal/handoff/`{{WITNESS_LIST}}`；`STEM_WITNESS_REVIEWER` **只对照**，不负责从零规划清单。
- **不要**泛化 AlphaGeometry 式「随机前提 + 符号闭包」到无闭包语义的领域；候选须能映射到 **已有 CAS/SMT/ITP 或 repo 脚本**，否则保持 `open`。
- **不要**把 QED 式子目标树当作猜想生成器；子目标分解仅用于 **已有主命题证明** 的编号步骤（§C 依赖图）。

---

## E. 操作员 closeout（数理 + RFV）

1. Verifier 从 repo root 跑 `verify_commands`（STEM：符号与数值分条；命令串避开「仅裸 `python`」作唯一 PostTool 信号）。
2. **`EVIDENCE_INDEX`** 至少一条成功行：`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1` 时依赖 PostTool；**默认关**时用 `framework hook-evidence-append` 或粘贴 exit 摘要。
3. `append_round`：`verify_result=PASS|FAIL`（discovery 中间轮可为 `SKIPPED`，但 **close** 时若 opt-in `close_gates.require_last_round_verify_pass` 须 PASS）。
4. 叙事 closeout 只引用 **promoted 且** 有证据行的结论；`open`/`rejected` 记入摘要即可。

2026-05 起 **无** hook `RFV_LOOP_CONTINUE`；续跑靠手动画板 + `framework_rfv_loop status`。

---

## F. 数学建模（phenomenon → model_spec → 可检验）

面向 **现象机理 → 变量/参数 → 控制方程/本构/边界初值 → 无量纲与渐近 regime → 可数值或符号检验**；不是替代 §C 的完整证明，而是把「模型陈述」变成可 witness 的对象。

### 建模中间对象（model witnesses）

| 块 | 要求 |
|----|------|
| **现象与尺度** | 一句话现象 + 特征时间与空间尺度（哪怕量级估计） |
| **状态/参数** | 区分 **状态变量**、**参数**、**外源**；单位写明 |
| **控制方程** | ODE/PDE/代数-微分/优化条件/随机演化（类型标明）；**不**省略边界/初值/守恒或耗散说明 |
| **本构与闭合** | 每条本构单独列出 **假设** 与 **失效情形** |
| **量纲 / 无量纲** | 至少一组 **π 群或特征尺度**；退化极限（如 Re→0、噪声→0）写入 witness |
| **Regime 图** | 参数空间中 **主导平衡/适用近似** 的分区（可定性）；每区对应不同简化模型 |
| **可辨识性风险** | 哪些参数/项从现有数据/实验 **不可辨识** 或需额外观测 |

### RFV 编排（与 §D 共用单轮，勿叠轮）

**只读并行（≤3 路，择二或三）**：

- `Reviewer`（对照 goal 与已有数据/实验描述）
- `External`（`external_mode=modeling`：文献中的标准模型族、典型无量纲数、反例文献）
- **`STEM_MODEL_FORMULATOR`**（只读：产出 `model_spec` 草案，见 [lane-templates.md](lane-templates.md)）

**Supervisor model promotion**（非 subagent）：将 `model_spec` 中 **采纳的方程组/本构** 标为 `promoted`；导出 **`{{WITNESS_LIST}}`**（量纲、退化极限、守恒、对称、小参数展开）与 **`falsification_tests`**（符号化简、量纲齐次性脚本、固定 seed 的数值对照）。

**证伪与验证**：仍走 `STEM_WITNESS_REVIEWER ‖ COUNTEREXAMPLE ‖ ADVERSARIAL_PROBE` + Verifier。建模特化 counterexample：**量纲不一致、稳态不存在、非物理振荡、错误极限标度**。

### 结构化落盘

| 内容 | 字段 |
|------|------|
| 模型陈述 | `external_research.model_spec`（`RFV_EXTERNAL_RESEARCH.schema.json`） |
| 待检验 | `append_round.falsification_tests`（量纲检查、残差、与简化模型对照） |
| PASS/FAIL | `verify_commands` + `EVIDENCE_INDEX` |

**禁止**：仅用叙事「模型合理」close；**禁止**把未 promotion 的备选本构当作最终模型。

### 与代码/实验

- 仿真/拟合脚本在 **fixer** 范围；`verify_commands` 含 **固定 seed** 的短跑（与 §A 双轨一致）。
- 统计推断、实验设计仍走 [`$research-execution`](../../../skills/research-execution/SKILL.md) `experiment_design` / `$statistical-analysis`。

---

## G. 未知数学性质与数学背景（theory landscape）

面向 **「还不清楚该用哪套数学语言 / 哪些定理可能相关 / 类比是否成立」** 的深度探讨；产出是 **可引用的背景地图 + 命名定理与假设 + 类比失效面**，不是一篇无来源综述。

**深度 runbook（检索扇出、类比语义适配、外部实践对照）**：[math-background-inquiry.md](math-background-inquiry.md)。

### 背景中间对象

| 块 | 要求 |
|----|------|
| **问题类** | 属于哪类数学对象/理论（例：椭圆型 PDE、鞅、凸优化、代数几何中的…） |
| **标准对象** | 该领域 **常用结构**（空间、范数、算子、分布假设）及 **为何相关** |
| **命名结果** | **定理/引理名称 + 所需假设**（不展开证明）；每条挂 **可追溯来源** |
| **定理适用性** | `theorem_applicability`：**`applies_when` / `fails_when`** 分栏，避免「定理名堆砌」 |
| **跨域桥接** | `cross_domain_bridges`：桥接想法 + **断裂条件**（非符号替换式类比） |
| **证明策略提示** | `proof_strategy_hints`：模式级提示（来源领域、局限），**不是** 完整证明 |
| **类比候选** | `analogy_candidates`：`mapping` + 硬性 **`breaks_when`** |
| **开放数学缺口** | `open_mathematical_gaps`：阻断进展的缺口 |
| **检索扇出计划** | `retrieval_fanout_plan`：与 `retrieval_trace` 对齐的多源查询意图 |

### RFV 编排

- **External `external_mode=math_background`**：在 strict `claims` / `contradiction_sweep` / `retrieval_trace` 之外，填 **`theory_background`**（或 lane 同名字段）；可与 `conjecture_list` **同轮并存**（背景 vs 待证构造分轨）。
- **可选** `STEM_CONJECTURE_EXPLORER`：当缺口需 **构造型猜想** 时启用（§D）。
- **不要**用背景探讨跳过 verify：背景块 **不** 抬 `depth_score`；第三分仍看 falsification / verify（同 §D）。

### 结构化落盘

| 内容 | 字段 |
|------|------|
| 文献与地图 | `external_research.theory_background` + 常规 `claims` |
| 待证性质 | `conjecture_list`（§D） |
| 检验 | `falsification_tests` + `EVIDENCE_INDEX` |

### 操作员提示

- 深度背景探讨默认 **`allow_external_research=true`** 且 **structured + strict**（见 [external-research-harness.md](external-research-harness.md)）。
- 检索扇出：[`research-execution` … `academic-sources.md`](../../../skills/research-discovery/references/academic-sources.md)。

---

## H. 实现映射（roadmap v4 phase 5b）

| 切片 | 路径 | 契约节 |
|------|------|--------|
| N1 `FormalVerifier` trait + request/result | `math_verify/formal.rs` | §B CAS/SMT 请求面 |
| N2 `DimensionChecker`（纯 Rust SI 基量纲） | `math_verify/dimension.rs` | §F 量纲 witness |
| N3 `Z3Backend` + `formal_verify_depth_signal` | `math_verify/backends/z3.rs` | §B SMT；depth advisory |
| N4 `StepVerifier` + `step_verify_depth_signal` + 量纲步/`@step_id` 结论传递 | `math_verify/step_verify.rs` | §C 依赖图；§F 量纲 witness |
| N5 `Lean4Backend`（P3 可选） | `math_verify/backends/lean4.rs` | §B ITP backend |
| N6 文档 + GOAL depth rollup | 本文 + `task_state::depth_compliance_aggregate` | §B.1 advisory |

**未实现 / 仍靠脚本或 ITP 模板**：Lean/Coq 子进程 backend、反事实 fuzz lane 自动化、discovery promotion 与 checker 的自动升格。ITP 与长尾脚本仍走 `verify_commands` + `EVIDENCE_INDEX`。

**操作员最短路径**：

1. `framework_math_verify` → `status` 确认 SymPy/Z3 可用。
2. 建模量纲 → `dimension_check`；单步恒等 → `formal_verify`；多步证明 → `step_verify`。
3. PASS 结果写入 `EVIDENCE_INDEX`（PostTool 或 `hook-evidence-append`）；可选把 `FormalVerifyResult` 追加到 `GOAL_STATE.math_verify_formal_results` 供 advisory rollup。

---

## 与非目标

- 不在 L3/L4 实现自动定理证明；默认仍是 **小 checker + 强对照**。
- **不做**通用自动定理发现/合成数据闭包（AlphaGeometry 类）；只做 **可 checker 升格** 的候选 + 证伪。
- **不把** §F 建模叙述或 §G 背景地图当作 **已验证模型/已证性质**；二者须落到 witness + falsification + `EVIDENCE_INDEX`。
- **不用** `conjecture_list` / promotion 抬高 `depth_score` 或替代 `falsification_tests`。
- 不新增第二套证据 schema；仍用 **`EVIDENCE_INDEX`** + **`append_round`**（`falsification_tests` / `adversarial_findings` 已存在）。
- 长版操作员文案见 `configs/framework/HARNESS_OPERATOR_NUDGES.json`（**参考**；**不**经 hook `GOAL_CONTINUE`/`RFV_LOOP_CONTINUE` 或 digest 注入，2026-05）。`harness_context_signals` 仍服务 PostTool → `EVIDENCE_INDEX`（opt-in）与 stdio 对照；`validation_commands` / `verify_commands` 扫描与 `framework_runtime` 对齐。英文 `proof` 仅 ASCII 词边界并排除 **proof of concept** / **proof-of-concept**；已去掉裸 **`derive`** 以免 Rust/Serde 误触。
- **PoC 与 toolchain**：PoC 短语下宽松英文词不单独触发；仍可由中文数理词、toolchain 子串（SymPy / Z3 / Lean 等）或 **`formal proof`** / **`mathematical proof`** 触发。
