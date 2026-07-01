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
  version: '2.0.0'
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
- 需要用 CAS 检查恒等式是否成立（SymPy 可用时支持微积分、级数、求解）
- 需要用 SMT solver 验证约束一致性（Z3 可用时支持非线性/SMT）
- 需要用特例/极限验证公式行为
- 需要检查物理量纲一致性（支持基础传播）
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
- 后端不可用时，工具自动降级到纯 Rust（线性 minilp + 基础代数），不假装通过

## Input / Output

| 输入 | 输出 |
|------|------|
| 推导步骤序列（LaTeX 或符号表达式） | 每步的 PASS / FAIL 状态 |
| 已知约束 / 公理 | CAS 化简残差（应为 0） |
| 变量量纲表 | 量纲一致性报告 |

## Verification Checklist

通过 MCP tool 调用数学验证后端：

```
math_prove_inequality(expression="x + y <= 10")                # 线性不等式 (minilp)
math_prove_inequality(expression="x^2 + y^2 <= 1")             # 非线性不等式 (Z3)
math_asymptotic_estimate(expression="x^2 + x", regime="oo")    # 渐近阶估计
math_asymptotic_chain(steps=[...])                              # 渐近链传递性
math_backend_available(backend="all")                           # 多后端可用状态
```

| # | 检查名 | PASS 条件 | 调用方式 |
|---|--------|-----------|----------|
| 1 | 不等式一致性（线性） | minilp 返回 feasible | `math_prove_inequality` |
| 2 | 不等式一致性（非线性） | Z3 check() == sat | `math_prove_inequality`（自动路由） |
| 3 | 渐近关系链传递性 | 纯链自动 PASS，混合链 WARN | `math_asymptotic_chain` |
| 4 | Proof DAG 验证 | 递归遍历，调用真实后端 | `math_proof_dag_verify` |
| 5 | 后端可用性 | 后端返回 available=true | `math_backend_available(backend="z3"\|"sympy"\|"lean"\|"all")` |
| 6 | 符号恒等式检查 | SymPy 化简差为 0，或纯 Rust 展开+代数化简+100次数值采样 | `math_sympy_verify` |
| 7 | 表达式化简 | SymPy 或纯 Rust 化简 | `math_sympy_simplify` |
| 8 | Witness 一致性 | 代入特例值后左右两边一致 | `research_verification_formal(check="witness", expression=..., witnesses=[{var: val}])` |
| 9 | 量纲检查 | 每步方程左右两侧量纲相同（支持传播分析） | `research_verification_formal(check="dimensional")` + 传播分析 |
| 10 | 步骤依赖图完整性 | 无悬空引用 | `research_verification_formal(check="step_dependency", steps=[{id, depends_on}])` |

### 后端状态说明

| 后端 | 需要安装 | 可用检查 | 不可用降级 |
|------|---------|----------|-----------|
| Z3 (SMT) | `uv pip install z3-solver` | `python_bridge::z3_available()` | 非线性不等式 → Warn，线性 → minilp 继续 |
| SymPy (CAS) | `uv pip install sympy` | `python_bridge::sympy_available()` | 所有 sympy 操作 → 纯 Rust symbol 引擎 |
| Lean 4 | 系统 PATH 安装 | `which lean` | 返回 Lean 不可用 |
| minilp (LP) | 内置（纯 Rust） | 恒 true | 无降级 |

## References

- math-derivation skill：[`../../skills/math-derivation/SKILL.md`](../../skills/math-derivation/SKILL.md)
- Python backend：`core/research-harness/python/math_backend/`
- framework formal_toolchain（令牌检测）：`core/framework-core/src/formal_toolchain.rs`

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
| `variable_dimensions` | `Vec<{name, dim}>` | no | Physical dimension table (omit = skip dimension checks) |

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
- **Backend unavailable**: tool auto-degrades (Z3 → minilp for linear, SymPy → pure Rust), no silent pass-through.
