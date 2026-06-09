---
name: formal-verification
description: |
  无状态内部 skill：用 CAS / SMT / witness 和量纲检查验证数学推导的正确性。
  由 research-execution 内联调用，不由用户直接触发。
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
user-invocable: false
disable-model-invocation: true
risk: low
source: local
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [math, formal, verification, CAS, SMT, research]
trigger_hints:
  - 数学推导验证
  - CAS 化简
  - SMT 检查
  - 量纲检查
  - witness 一致性
---

# Formal Verification

无状态能力 skill：对数学推导和公式做形式化验证。不独立编排会话。

## When to Use

- 前门 skill 需要验证推导步骤的代数正确性
- 需要用 CAS 检查恒等式是否成立
- 需要用 SMT solver 验证约束一致性
- 需要用特例/极限验证公式行为
- 需要检查物理量纲一致性
- 需要验证推导步骤依赖图无断裂

## Input / Output

| 输入 | 输出 |
|------|------|
| 推导步骤序列（LaTeX 或符号表达式） | 每步的 PASS / FAIL 状态 |
| 已知约束 / 公理 | CAS 化简残差（应为 0） |
| 变量量纲表 | 量纲一致性报告 |

## Verification Checklist

> **Note**: verify commands below are pattern templates. Actual commands depend on project setup and available tools.

| # | 检查名 | PASS 条件 | verify command |
|---|--------|-----------|----------------|
| 1 | CAS identity 化简 | SymPy simplify(expr) == 0 | `python -c "from sympy import simplify; assert simplify(expr) == 0"` |
| 2 | SMT 预期一致性 | Z3 check() == sat | `python -c "from z3 import *; s = Solver(); s.add(...); assert s.check() == sat"` |
| 3 | Witness 一致性 | 代入特例值后左右两边一致 | `python -c "from sympy import symbols; ...; assert lhs.subs(vals) == rhs.subs(vals)"` |
| 4 | 量纲检查 | 每步方程左右两侧量纲相同 | `grep -c 'DIMENSION_MISMATCH' dimension_report.txt` → exit 0 = 0 条不匹配 |
| 5 | 步骤依赖图完整性 | 无悬空引用（每步的前置步骤已定义） | `python -c "import networkx as G; ...; assert len(orphans) == 0"` |

## References

- math-derivation skill：[`../math-derivation/SKILL.md`](../math-derivation/SKILL.md)（推导能力与符号计算知识库）
- framework math_verify（Rust）：`core/core-math/`（`formal_toolchain`；CAS + SMT 后端，roadmap v5 B0）

## Integration

前门 skill 在以下时机内联调用本 skill：

- **research-execution**：数学推导完成后，做形式化门禁检查

调用方式：将推导步骤序列传入，按验证清单逐项执行，FAIL 项作为 blocker 回写。
