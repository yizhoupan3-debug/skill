---
description: Adversarial review and formal verification of mathematical content
metadata:
  platforms:
  - supported
  tags:
  - mathematics
  - proof-review
  - formal-verification
  - adversarial
  - counterexample
  - CAS
  - SMT
  - inequality
  - asymptotic
  version: '1.0.0'
name: math-verify
scene: research
sub_scene: formal
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: Adversarial review and formal verification of mathematical content
trigger_hints:
- 数学证明审查
- 公式正确性验证
- 推导步骤验证
- 数学论证审阅
- 验证数学
- 审一下这个推导对不对
- 检查这个证明有无漏洞
- math-verify
- 数学推理正确性
- 不等式推导审查
- 证明漏洞
- 反例寻找
- 形式验证
- 数学正确性
- 检查这个定理证明
---
# Math Verify

审查已有数学推理的正确性，对抗式寻找漏洞，调用后端工具链进行形式化验证。

**定位**：审稿/审查路径 — 专注于检查、质疑、验证既有的数学推导和证明。

## 双Skill分工

| Skill | 路径 | 时机 |
|-------|------|------|
| **`math-verify` (本 skill)** | **验证/审查** | **审查既有推导的正确性、发掘漏洞** |
| `math-explore` | 发现/探索 | 探索新结构、生成猜想、性质分析 |

两者共享底层 `math-reasoning-harness` 工具链和 `$formal-verification` quality gate。推导执行能力作为本 skill 的内部工作流步骤（审查过程中发现推导问题后自主修复），不设独立入口。

## When to use

- 用户提供了数学推导/证明序列，需要审查每一步的正确性
- 用户想验证一个数学命题或不等式是否成立
- 用户需要对抗式漏洞分析——找反例、找逻辑断裂、找未声明的假设
- 用户有研究手稿或审稿任务中的数学部分需要检查
- 用户想验证既有公式推导是否存在操作错误（符号错误、极限交换非法等）
- 证毕的证明需要进行第二轮独立验证
- 适合如下的请求：
  - "审一下这个推导，看看每一步对不对"
  - "验证这个不等式是否对所有正数成立"
  - "这个证明有漏洞吗？帮我找反例"
  - "检查一下这个证明的极限交换是否合法"
  - "这个定理的证明是否正确？写个审查报告"

## Do not use

- 用户需要探索新数学性质/生成猜想（→ `$math-explore`）
- 用户需要从零开始构建新证明（本 skill 在审查推导后可一并修复，但不独立提供"从零推导"入口）
- 用户仅需要数值计算或实现数学代码（→ 当前上下文内联处理）
- 用户需要统计方法选型与解读（→ `$statistical-analysis`）
- 用户需要LaTeX编译渲染（→ 当前上下文内联处理）
- 非数学内容或非正确性类的文字审查（→ `$prose-verification`）

## Core workflow

1. **整体评估**：理解证明/推导的结构：结论、假设、证明策略、步骤数
2. **步骤分解**：将推导拆解为可独立验证的原子单元（每一步一个 claim）
3. **工具调用**：对每个可验证步骤，调用最适配的后端检查器：

   | 检查类型 | 最佳后端 | 调用方式 |
   |----------|---------|----------|
   | 代数恒等式（等式推导） | SymPy | `math_sympy_verify` |
   | 代数化简验证 | SymPy | `math_sympy_simplify` |
   | 不等式约束一致性 | Z3/minilp | `math_prove_inequality` |
   | 数值 witness 验证 | SymPy | `math_sympy_lambdify` |
   | 不等式边界收紧 | Z3/SymPy | `math_tighten_bounds` |
   | 渐近关系/链 | 纯 Rust | `math_asymptotic_estimate` / `math_asymptotic_chain` |
   | 符号微分/积分 | SymPy | `math_sympy_differentiate` / `math_sympy_integrate` |
   | 方程求解回溯 | SymPy | `math_sympy_solve` |
   | 三角恒等式 | SymPy | `math_sympy_trig_simplify` |
   | 级数展开 | SymPy | `math_sympy_series` |
   | 极限计算 | SymPy | `math_sympy_limit` |
   | 物理量纲一致性 | SymPy/Rust | `research_verification_formal(check="dimensional")` |
   | 量纲传播分析 | SymPy | `math_sympy_dimension_propagate` |
   | 步骤依赖完整性 | Rust | `research_verification_formal(check="step_dependency")` |
   | Witness 验证 | SymPy/Z3 | `math_witness_consistency` |
   | 多步 AND-OR 分解 | Proof DAG | `math_proof_dag_init` → `math_proof_dag_decompose` → `math_proof_dag_verify` |
   | 同态检测 | SymPy/Z3 | `math_check_homomorphism` |

4. **对抗式漏洞挖掘**：主动搜索以下常见陷阱（与形式化验证硬约束对齐）：
   - 除零和逆运算分母为零检查
   - 极限交换未引用 DCT/MCT/Leibniz
   - 点态收敛 vs 一致收敛混淆
   - 数学归纳法缺基例/归纳步
   - 循环论证（结论出现在某步中）
   - 隐含连续性/可微性/可积性假设
   - 不等式方向混淆（≤ vs <）
   - 量词顺序问题（∀∃ 混淆）
   - 充分/必要条件混淆（⇒ 写成了 ⇔）
   - 命名定理假设未验证
   - "显然"/"trivially" 处缺少 justification
5. **反例搜索**：尝试构造反例或边界情形来证伪
6. **合成审查报告**：以 `PASS` / `WARN` / `FAIL` + blocker 列表输出

## 审查报告模板

```
## 审查概况
**源文件/推导**：...
**步骤数**：N
**审查结论**：PASS / WARN 阻塞 / FAIL

## 逐步骤验证

### Step N: [描述]
- **claim**: $...$
- **验证方法**: [`math_sympy_verify` | `math_prove_inequality` | 反例搜索 | 手工推理]
- **验证结果**: PASS / FAIL / WARN
- **证据**: (化简残差 / sat 模型 / 反例值 / 依赖图)
- **发现**: (如有漏洞，P0/P1/P2 分级)

## 对抗式漏洞分析

| # | 检查维度 | 状态 | 详述 |
|---|----------|------|------|
| 1 | 除零检查 | ✅ ❌ | ... |
| 2 | 极限交换 | ✅ ❌ | ... |
| 3 | 数学归纳完备性 | ✅ ❌ | ... |
| 4 | 循环论证 | ✅ ❌ | ... |
| 5 | 假设显式化 | ✅ ❌ | ... |
| 6 | 不等式方向 | ✅ ❌ | ... |
| 7 | 命名定理假设验证 | ✅ ❌ | ... |
| 8 | 量词顺序 | ✅ ❌ | ... |
| 9 | ⇒ / ⇔ 区分 | ✅ ❌ | ... |
| 10 | "显然" 审查 | ✅ ❌ | ... |

## 反例探测
- **尝试的反例**：（列出尝试过最终被排除的反例）
- **有效反例**：（如有，列出并说明反驳了哪一步）

## 严重性分级

| 级别 | 含义 | 行动 |
|------|------|------|
| **P0** | 证明根本性错误 | 必须修正后方可声称证明成立 |
| **P1** | 局部漏洞但不影响整体结论 | 建议补充 justification |
| **P2** | 小疏忽或不影响正确性的不完整 | 建议完善 |
| **Caveat** | 可疑但当前证据不足 | 建议进一步审查 |

## 结论
**总体 verdict**: ACCEPT / REJECT / CONDITIONAL
**主要 blocker**: ...
**修正建议**: ...
```

## 严重性分级

| 级别 | 含义 | 行动 |
|------|------|------|
| **P0** | 证明根本性错误 | 必须修正后方可声称证明成立 |
| **P1** | 局部漏洞但不影响整体结论 | 建议补充 justification |
| **P2** | 小疏忽或不影响正确性的不完整 | 建议完善 |
| **Caveat** | 可疑但当前证据不足 | 建议进一步审查 |

## Hard constraints

> [!CAUTION]
> 适用于本 skill 的所有审查输出。

1. **每个 PASS 必须有具体证据**：不得仅写 "通过"——必须附上化简残差、sat 模型、特例数值或反例是否定。
2. **FAIL 必须标注 blocker 并给出修复建议**：不得静默降级为 WARN。
3. **每一步都需验证**：不得跳过中间步骤假设其正确。
4. **反例探测**：对于所有关键步骤，至少尝试一个边界情形或反例。
5. **命名定理必须验证假设**：审查时确认每个引用的定理的假设在当前上下文中被声明并满足。
6. **"verified" 必须 checker-backed**：仅 prose 验证不能标记为 "通过"。
7. **对审查结论附置信度**：明确标注某个 claim 是 "后备可用验证"、"人工推理" 还是 "无法验证"。
8. **后端不可用时间标注**：当后端不可用导致降级时，在审查报告中标注 `[BACKEND_UNAVAILABLE]`。

## References

- [`../../quality-gates/formal-verification/SKILL.md`](../../quality-gates/formal-verification/SKILL.md) — Quality Gate 形式验证
- [`../../docs/math-reasoning-harness.md`](../../docs/math-reasoning-harness.md) — 数学验证工具链文档
