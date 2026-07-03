---
description: Explore and discover new mathematical properties, relations, and structures
metadata:
  platforms:
  - supported
  tags:
  - mathematics
  - exploration
  - discovery
  - conjecture
  - property-testing
  - CAS
  - SMT
  - inequality
  - asymptotic
  - generalization
  version: '1.0.0'
name: math-explore
scene: research
sub_scene: discovery
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: Explore and discover new mathematical properties, relations, and structures
source: project
trigger_hints:
- 探索数学性质
- 猜猜这个有没有什么性质
- 数学发现
- 测试数学猜想
- 找规律
- 这个表达式有什么有趣的性质
- 尝试推广
- 什么条件下这个成立
- 猜想验证
- 探索
- 数学新性质
- 生成猜想
- 数学实验
- 试探性数学
- math-explore
- 暴搜索特例找规律
- 数值实验找模式
---
# Math Explore

探索和发现新的数学性质、模式、关系和结构。这是"发现/探索"路径——面对开放性问题时，通过计算实验、特例扫描、模式识别来生成可检验的猜想。

**定位**：当你不确定某条数学路径通向何处时，从这里出发。

## 双Skill分工

| Skill | 路径 | 时机 |
|-------|------|------|
| `math-verify` | 验证/审查 | 审查既有推导的正确性 |
| **`math-explore` (本 skill)** | **发现/探索** | **未知性质，试探和发现** |

两者共享底层 `math-reasoning-harness` 工具链和 `$formal-verification` quality gate。推导执行能力作为探索阶段的内部工作流（发现性质后如需形式证明，在本 skill 内完成）。

## When to use

- 你有一个表达式、函数或结构，想知道它有什么有趣的性质
- 你想测试一个数学猜想在特例下是否成立
- 你想推广已知结果到更一般的设定
- 你想通过数值实验寻找模式或规律
- 你想探索两个不同数学结构之间的关系
- 你想找某个不等式对于什么参数范围成立
- 你想渐近分析一个复杂表达式在边界情况下的行为
- 你要为某个问题的解决寻找思路/线索
- 适合如下的请求：
  - "这个函数在边界上有什么行为？"
  - "试试对这个不等式做推广"
  - "找一下这个表达式有什么对称性"
  - "当参数趋向什么值时这个式子有有趣的性质？"
  - "这个映射在什么条件下保持某种结构？"
  - "数值扫一下，看看有什么模式"
  - "从特殊情形推断一般形式"

## Do not use

- 用户需要审查已有推导的正确性（→ `$math-verify`）
- 用户已知明确目标、只需要计算执行（→ 当前上下文内联处理）
- 这是统计探索/数据模式而非数学性质（→ `$statistical-analysis`）
- 这是文献知识检索而非数学探索（→ `$research` discovery lane）
- 用户已经确定了猜想、需要严格的 checker-backed 证据（→ 在本 skill 内完**形式化验证，或通过 `$math-verify` 路径追加对抗式审查）

## Core workflow

```
输入问题/表达式/结构
    │
    ├── (A) 静态分析
    │   ├── 符号化简探索——表达式结构解析
    │   ├── 渐近行为分析——∞和0近邻
    │   ├── 量纲探索——物理量纲一致性
    │   └── 代数结构检测——对称性/周期性/因子分解
    │
    ├── (B) 数值探索
    │   ├── 随机采样——快速大规模数值试探
    │   ├── 边界扫描——参数边界附近的异常行为
    │   └── 模式可视化建议——建议用户可视化的特征
    │
    ├── (C) 猜想生成
    │   ├── 从数值模式形成猜想
    │   ├── 从渐近行为猜测渐近形式
    │   └── 特殊情况→推广假设
    │
    └── (D) 猜想评估
        ├── 快速 falsify：用工具链测试猜想是否显然有反例
        ├── 置信度评估：基于已测试的特例覆盖
        └── Promote 建议：如果猜想站住脚，在本 skill 内完成形式化验证
```

### 阶段详解

#### (A) 静态分析

| 操作 | 工具 | 产出 |
|------|------|------|
| 符号化简 | `math_sympy_simplify` | 表达式最简形式 |
| 因式分解 | `math_sympy_factor` | 因子结构 |
| 多项式展开 | `math_sympy_expand` | 展开形式 |
| 级数展开 | `math_sympy_series` | 局部近似 |
| 极限计算 | `math_sympy_limit` | 边界行为 |
| 量纲分析 | `research_verification_formal(check="dimensional")` | 物理量纲 |
| 渐近估计 | `math_asymptotic_estimate` | 主导增长阶 |
| 微积分行为 | `math_sympy_differentiate` / `math_sympy_integrate` | 单调性/凹凸性/积分形式 |
| 方程求解（根） | `math_sympy_solve` | 零点结构 |
| 同态/同构 | `math_check_homomorphism` | 结构保持关系 |

#### (B) 数值探索

| 操作 | 工具 | 产出 |
|------|------|------|
| 符号转数值函数 | `math_sympy_lambdify` | 可调用的数值处理器 |
| 贝叶斯扫描 | 结合 `math_witness_consistency` 生成随机批量测试 | 不等式的数值合理性 |
| 边界探测 | 对参数边界（0、∞、±边界值）计算函数值 | 异常行为点 |

**注意**：数值探索不产生证明——它只提供证据和线索。所有数值发现均标注为 `[NUMERICAL_EVIDENCE]`，不做代数保证。

#### (C) 猜想生成

从 (A) 和 (B) 的发现中，综合可检验的猜想：

1. **模式归纳**：从数值模式到数学形式的猜想
   - "数值提示 $f(x) = \mathcal{O}(x^2)$ 当 $x \to \infty$"
   - "数值解显示 $\sum_{k=1}^n k^3 = \left(\frac{n(n+1)}{2}\right)^2$ 在测试范围内成立"
2. **边界行为猜想**：从极限分析得到边界附近的渐近形式
3. **推广猜想**：从特殊情况推广到更一般条件
   - "这个不等式在 $n=2,3,4$ 时成立，推广到 $n>4$ 可能需要额外条件..."

#### (D) 猜想评估

在投入形式证明之前，用工具链快速 falsify：

1. **反例快速搜索**：在边界和反直觉区域采样
2. **渐近一致性检查**：猜想中的渐近关系是否与已知行为一致
3. **特例全面覆盖**：评价猜想在已测试空间上的置信度
4. **Promote 决策**：
   - 猜想站住脚 → 输出正式猜想声明，在本 skill 内使用工具链完成形式化证明
   - 猜想被 falsify → 输出反例，记录为什么失败，提出修正方向
   - 猜测不确定 → 建议进一步探索或缩小范围

### 与 `$research` 的关系

当数学探索需要文献支持（"这个性质是否已知？有没有相关定理？"），使用 `$research` discovery lane 做文献调研。本 skill 专注于对表达式的**内部探索**（计算实验 + 模式识别），不负责文献回顾。

## Output template

```
## 探索目标
[问题陈述 / 表达式 / 结构]

## (A) 静态分析
### 表达式结构
- 化简结果：...
- 因子结构：...
- 级数展开：...
### 渐近行为
- x → ∞：...
- x → 0：...
- 边界情况：...
### 微积分性质
- 导数：...
- 单调性：...
- 积分：...
### 代数结构
- 对称性/周期性：...
- 同态映射：...
- 特殊值：...

## (B) 数值探索
- 采样范围：[参数范围，采样密度]
- 发现模式：[数值提示]
- 异常点：[如有]

## (C) 猜想
- **猜想 1**：$...$ [置信度：高/中/低]
   *证据*：[来自 (A)/(B) 的证据链]
- **猜想 2**：$...$

## (D) 猜想评估
- **已测试覆盖**：[特例/N 个随机点]
- **反例**：[找到 / 未找到]
- **falsify 建议**：[如有]
- **推荐路径**：[内部形式化 | 细化探索 | 放弃]

## 结论
[一句话总结探索结果]
```

## Hard constraints

> [!CAUTION]
> 适用于本 skill 的所有探索输出。

1. **数值证据 ≠ 证明**：所有非代数推导的结论（仅基于数值采样）必须标注 `[NUMERICAL_EVIDENCE]`，不得声称"已验证"。
2. **猜想 vs 结论**：所有未证明的断言必须称为"猜想"而非"结论"——只有经过严格推导并用 checker 验证后才可称结论。
3. **探测范围透明**：数值探索必须注明采样范围、密度和随机种子（如适用）。
4. **反例优先**：在提出猜想前，优先尝试 falsify 而非 confirm。
5. **从特殊到一般**：不跳跃——从特例/简单情形开始，逐步推广，每一步标注推广的合理性和风险。
6. **不供假阳性建议**：如果证据不足以形成合理猜想，说明为什么，不要强行编造模式。
7. **后端不可用时标注**：若 SymPy/Z3 不可用导致探索不完整，标注 `[BACKEND_LIMITED]`。

## References

- [`../math-verify/SKILL.md`](../math-verify/SKILL.md) — 数学验证 skill（审查已有推导）
- [`../../quality-gates/formal-verification/SKILL.md`](../../quality-gates/formal-verification/SKILL.md) — Quality Gate 形式验证
- [`../../docs/math-reasoning-harness.md`](../../docs/math-reasoning-harness.md) — 数学验证工具链文档

## Lifecycle integration

探索 → 猜想定型 → 形式化验证闭环：

```
math-explore  →  产生猜想
      │
      ▼
  (用户决定: 值得证明吗?)
      │
      ├── 是 → 在本 skill 内形式化证明 + $math-verify 对抗式审查
      │              │
      │              ▼
      │          $formal-verification Quality Gate
      │
      └── 否 → 记录发现，存档
```
