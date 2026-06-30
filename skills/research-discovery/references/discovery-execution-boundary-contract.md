# Discovery-Execution 边界合约（科研发现与执行分工规范）

**目的**：消除 `research-discovery` 与 `research-execution` 之间的职责模糊，提供可路由、可执行、可抗辩的边界定义。  
**读者**：两个 SKILL 的维护者及实现方、`router-rs` 路由层、上游 `$good-question`/`$paper-workbench` 编排方。

---

## §1 Scope

本合约定义 `research-discovery`（L2, front door）与 `research-execution`（L2, execution lanes）之间的职责边界、handoff 协议和场景路由规则。两个 SKILL.md 均引用此文件作为边界裁决的规范来源。

| 领域 | 本合约覆盖 | 不在本合约范围 |
|------|-----------|---------------|
| Math 分工 | candidate theorem vs proof strategy | 纯推导执行（归 `$math-derivation`） |
| Novelty | 判断归属和 handoff | 非科研场景的 novelty（不适用） |
| Dataset | identification vs selection | 数据集构建/标注（归数据工程） |
| Experiment forensics | 失败复盘归属和流程 | 代码单步调试（归当前 coding context） |
| Verification | 共享验证合约（extracted from both SKILL.md） | manuscript 验证（归 `$paper-workbench`） |

---

## §2 Primary ownership table

| Concern | `research-discovery` | `research-execution` |
|---|---|---|
| Research question scoping | **Primary** — 目标任务/N/决策支持 | Advisory — 执行可行性评估 |
| Literature survey, prior-art retrieval | **Primary** — `external_research` lane | — |
| Theory landscape, theorem applicability | **Primary** — `math_background_inquiry` lane | Advisory — 验证时引用 |
| Knowledge gap analysis | **Primary** — gap → 下一步检索方向 | — |
| Novelty / significance assessment | **Primary** — `research_question` lane | — |
| **Dataset identification & curation (有哪些 benchmark/数据集)** | **Primary** — 有哪些 benchmark/数据集、各自属性 | —（hand off to discovery for listing） |
| **Dataset selection & justification (选哪个做实验)** | —（discovery listed candidates） | **Primary** — 选择哪个做实验 + justify |
| **Model selection justification** | **Primary** — 该领域有哪些 SOTA/候选模型 | **Primary** — 选哪个、为什么、对比条件 |
| Experiment design, ablations, baselines | — | **Primary** — `experiment_design` lane |
| **Experiment forensics / 失败复盘** | — 仅在 execution 发现未知未知时被 loop back | **Primary** — `experiment_design` sublane |
| Code verification, deterministic repro | — | **Primary** — `code_verification` lane |
| Math modeling (phenomenon→equations→nondimensional) | — | **Primary** — `math_modeling` lane |
| Math verification (checker, witnesses) | — | **Primary** — `math_verification` lane |
| Reproducibility planning | — | **Primary** — 拒绝 `$experiment-reproducibility` (L3) |

---

## §3 Math 工作分界线

**这条分界线是 P1 级别变部署（两 SKILL 边界中最容易误用的点）**

| 阶段 | Owner | 产出 | 限制 |
|------|-------|------|------|
| 候选定理识别 | `research-discovery` → `math_background_inquiry` | candidate theorem list × 每条 `applies_when` / `fails_when` + cross-domain bridges（theorem 级别） | **不许写推导路线图**、不许声称"某定理在此条件下可证" |
| 推导策略 | `research-execution` → `math_verification` | `proof_strategy_hints`（推导路线图） + 定理/引理依赖链 + checker 选项 | 在 discovery 给出 candidate theorems 后产出 |
| 推导执行 | `$math-derivation` | formal proof / 符号推导 / 数值验证 | — |
| 验证 | `$formal-verification` (L4) | Z3 / SymPy / Lean 验证结果 | — |

**关键不可逾越规则**：
- `research-discovery` **不得**产出 `proof_strategy_hints`。如果用户问"这个性质怎么证明"—— discovery 只回答"有哪些现成定理可能相关"；"怎么用这些定理推导"是 execution 的事。
- `research-execution` **可以不接受** discovery 的 candidate theorem list（如果用户直接提 execution 需求），但一旦需要使用 discovery 的定理推荐，必须先验证其 `applies_when` / `fails_when` 是否成立。
- 两 skill 均不得自称"已 verified"（"verified"、"严审通过"、"research-grade"）without witnesses + a checker/verifier。

---

## §4 Handoff protocol

### Discovery → Execution

当发现类工作完成后需执行类工作时传递：

```text
handoff: research-execution
context:
  theory_list: candidate theorems with applies_when/fails_when
  claims_to_verify: [claim_1, claim_2, ...]
  retrieval_trace: source + query + coverage + gaps
  evidence_gaps: [gap_description, ...]
  unresolved_assumptions: [assumption, ...]
  language_register: zh_manuscript | en_submission | mixed  # 仅当含非工程书面产出时
```

### Execution → Discovery (loop-back)

执行中发现了 discovery 阶段未覆盖的新未知时：

```text
handoff: research-discovery (loop-back)
reason: <what execution discovered — e.g. "baseline underperforms theory predicts, possible unknown interaction">
new_unknown: <the unresolved question>
retrieval_scope: <what additional search/citation is needed>
hypothesis: <optional — what discovery should test>
```

Loop-back 不是可选项—— execution 发现了 discovery 阶段的覆盖缺陷时**必须 loop back**，不得自行做文献检索或理论调查。

### Execution → Paper-workbench

当执行类工作完成后需升格为手稿：

```text
handoff: paper-workbench
experiment_design: <ablations / baselines / metrics used>
code_verification: <test results / repro evidence / benchmark outputs>
reproducibility: <seeds / config hashes / artifact paths>
math_results: <verification outputs / checker logs>
language_register: zh_manuscript | en_submission | mixed
```

---

## §5 Verification contract（共享部分）

两边共享的验证合约。各 SKILL 的 `Verification and failure contract` 段引用此部分，仅保留各自 lane 特有的验证要求。

### 5.1 默认关闭路径

- 将可执行证据视为默认关闭路径：commands、notebooks、deterministic probes、benchmark scripts、artifact hashes、或可追溯的已引用外部源。
- 在声称完成之前指定如何验证该声明。

### 5.2 Blocker 格式

当 Lane 无法验证时，返回 blocker 而非将其转换为置信研究结论：

```text
blocker:
  missing_input: <what is missing>
  unavailable_source: <which source / tool is unavailable>
  unrun_command: <what command needs to run>
```

### 5.3 错误摘要格式

工具或数据失败时，保存最精简的错误摘要 + 重试路径到证据地图中。不要将长日志粘贴到上下文。

### 5.4 各 skill 特有的验证要求

| Skill | 额外要求 |
|-------|---------|
| `research-execution` | 没有 baselines + controls + metrics + reproducibility 要求不得声称实验有效性 |
| `research-discovery` | 没有外部检索（当需要且允许时）不得将"深度研究"变成无来源猜测 |

---

## §6 Scenario routing table

下表覆盖边界模糊的场景（不在现有边界表或 trigger_hints 内的）：

| 用户问题 | 第一步路由 | 第二步 | 第三步 |
|----------|-----------|--------|--------|
| "这个领域该用什么 metric / metric 设计" | `research-discovery` → `external_research`（有哪些 metric） | `research-execution` → `experiment_design`（设计验证协议） | — |
| "该用什么数学理论来分析这个系统" | `research-discovery` → `math_background_inquiry`（候选定理） | `research-execution` → `math_modeling`（建数学模型） | `$math-derivation` → 推导执行 |
| "这个实验失败了，为什么" | `research-execution` → `experiment_design` / `experiment_forensics`（检查假设/变量/条件） | 如果发现 discovery 的定理覆盖不足 → loop-back 到 `research-discovery` | — |
| "这个方向有个 idea，能做吗（可行性）" | `research-discovery` → `research_question` + 初步 `external_research`（prior art） | `research-execution` → `experiment_design`（实验方案）| loop back 进一步 refine |
| "这个领域目前谁做得怎么样（综述）" | `research-discovery` → `external_research` | — | — |
| "帮我设计对比实验" | `research-execution` → `experiment_design` | 如需选择 benchmark → `research-discovery` → `external_research` | — |
| "我要做什么实验来证明这个方法有效" | `research-execution` → `experiment_design` | 如发现 baseline 不明确 → handoff 到 discovery | — |
| "哪些 benchmark 数据集适合验证" | `research-discovery` → `external_research`（列出候选） | `research-execution` → `experiment_design`（选 + justify） | — |
| "这个任务该用什么模型" | `research-discovery` → `external_research`（列出 SOTA/候选模型） | `research-execution` → `experiment_design`（选 + justify + 对比条件） | — |
| "这个实验出了这个结果，正常吗" | `research-execution` → `experiment_forensics` | 如需要理论解释  loop-back 到 discovery | — |

---

## §7 维护检查清单

- [ ] discovery 中 `math_background_inquiry` 产出不含 `proof_strategy_hints`
- [ ] execution `When to use` 不含 "novel enough"
- [ ] 两 skill 的 verification contract 引用本合同 §5
- [ ] discovery cross-reference 路径使用 `../../quality-gates/` 前缀
- [ ] execution→paper-workbench handoff 含结构化 payload
- [ ] Loop-back 在 execution 中被识别为必要条件而非可选
- [ ] Scenario routing table 新增场景后同步更新两 SKILL.md 的边界表
