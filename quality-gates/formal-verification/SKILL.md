---

description: 'Formal verification capability: CAS identity simplification, SMT consistency, witness validation, dimensional analysis, step dependency checking.'
metadata:
  platforms:
  - supported
  tags:
  - math
  - formal
  - verification
  - CAS
  - SMT
  - research
  version: '1.0.0'
name: formal-verification
scene: research
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: n/a
source: local
trigger_hints:
- $formal-verification
- CAS验证
- SMT检查
- formal verification
- witness验证
- 形式验证
- 量纲检查
- formal-verification
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

## Do not use

- 文稿语言质量检查（→ `$prose-verification`）
- 文献引用验证（→ `$literature-verification`）
- 统计结果审计（→ `$statistical-verification`）
- 实验设计（→ `$research` execution lane）

## Hard constraints

- 每个 PASS 必须附带具体数值证据（化简残差、sat 模型、特例值），不得仅标记 PASS 无依据
- FAIL 必须标注为 blocker 并给出修复建议，不得静默降级为 WARN
- 量纲检查失败一律为 P0 blocker，不得豁免
- 不得跳过步骤依赖图检查——断裂依赖意味着推导链不完整
- CAS 后端不可用时（SymPy/Z3 未安装），必须显式声明降级为人工审查，不得假装通过

## Input / Output

| 输入 | 输出 |
|------|------|
| 推导步骤序列（LaTeX 或符号表达式） | 每步的 PASS / FAIL 状态 |
| 已知约束 / 公理 | CAS 化简残差（应为 0） |
| 变量量纲表 | 量纲一致性报告 |

## Verification Checklist

通过 MCP tool 调用数学验证后端：

```
math_prove_inequality(expression="x + y <= 10")          # 不等式可行性
math_asymptotic_estimate(expression="x^2 + x", regime="oo")  # 渐近阶估计
math_asymptotic_chain(steps=[...])                           # 渐近链传递性
math_backend_available()                                     # 后端可用状态
```

| # | 检查名 | PASS 条件 | 调用方式 |
|---|--------|-----------|----------|
| 1 | 不等式一致性 | Z3 check() == sat | `math_prove_inequality` |
| 2 | 渐近关系链传递性 | 纯链自动 PASS，混合链 WARN | `math_asymptotic_chain` |
| 3 | Proof DAG 验证 | 递归遍历，每轮标记 stale | `math_proof_dag_verify` |
| 4 | 后端可用性 | 后端返回 available=true | `math_backend_available` |
| 5 | CAS identity 化简 | SymPy simplify(expr) == 0 | `math_prove_inequality`（parse via SymPy）或 `math_sympy_verify` |
| 6 | 符号恒等式检查 | 纯 Rust 符号引擎：代数展开/分配律/常量折叠 → 随机数值测试 | 内链符号引擎（`verification::symbolic`） |
| 7 | SymPy 桥接验证 | SymPy 后端化简/验证 | `math_sympy_verify` / `math_sympy_simplify` |
| 8 | Witness 一致性 | 代入特例值后左右两边一致 | 内链数值代入 |
| 9 | 量纲检查 | 每步方程左右两侧量纲相同 | 内链量纲分析（`verification::formal`） |
| 10 | 步骤依赖图完整性 | 无悬空引用（每步的前置步骤已定义） | 内联图检查 |

## References

- math-derivation skill：[`../../skills/math-derivation/SKILL.md`](../../skills/math-derivation/SKILL.md)（推导能力与符号计算知识库）
- framework formal_toolchain（Rust）：`core/runtime-core-contracts/src/formal_toolchain.rs`（CAS/SMT token 检测）

## Integration Contract

### Trigger

| Caller | When | Blocking | Call mode |
|--------|------|----------|-----------|
| `$research` (execution lane) | math_verification / math_modeling lane completes, before conclusion | Yes (FAIL blocks math conclusion) | Inline + MCP tool |

### Input

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `derivation_steps` | `Vec<LatexExpr>` | yes | Step-by-step LaTeX or symbolic expressions |
| `known_axioms` | `Vec<String>` | no | Known constraints / assumptions / axioms |
| `variable_dimensions` | `Vec<{name, dim}>` | no | Physical dimension table (omit = skip check #7) |

### Output

```json
{
  "status": "PASS" | "FAIL" | "WARN",
  "checks": [
    { "name": "inequality_consistency", "status": "PASS" | "SKIP" | "FAIL", "detail": "Z3 returned sat" },
    { "name": "asymptotic_chain", "status": "PASS" | "WARN", "detail": "..." },
    { "name": "proof_dag", "status": "PASS" | "FAIL", "detail": "..." },
    { "name": "backend_available", "status": "PASS" | "FAIL", "detail": "..." },
    { "name": "cas_identity", "status": "PASS" | "SKIP" | "FAIL", "detail": "simplify(expr) = 0" },
    { "name": "witness_consistency", "status": "PASS" | "SKIP" | "FAIL", "detail": "..." },
    { "name": "dimensional_analysis", "status": "PASS" | "SKIP" | "FAIL", "detail": "..." },
    { "name": "step_dependency", "status": "PASS" | "FAIL", "detail": "..." }
  ],
  "blockers": ["Step 7 depends on undefined Step 12"]
}
```

### Failure propagation

- **PASS**: caller continues normally.
- **WARN**: caller continues with annotation.
- **FAIL** (blocking caller): caller MUST NOT advance to next stage; blocker list is returned to user or upstream orchestrator.
- **Backend unavailable** (`backend_available` = FAIL): all CAS/SMT checks degrade to WARN and manual review is required.
