---
last_verified: "2026-06-26"
---

# Math Reasoning Harness

数学验证工具链 — ResearchHarness 的形式化验证后端。

## §A: 工具链总览

三层分离架构：

| 层 | 位置 | 职责 |
|----|------|------|
| **Feature** | `core/research-harness/src/verification/*.rs` | 纯业务逻辑：类型、SymPy/Z3/Lean 调用、验证函数 |
| **Tool** | `core/research-harness/src/mcp_tools.rs` | MCP dispatch：JSON 参数提取 → feature 层调用 → JSON 格式化；每个工具设置自己的 `check_name`，通过 `*_with_name()` feature 层函数传递 |
| **Schema** | `core/host-projection/src/hosts/mcp_stdio_harness/mod.rs` | MCP schema 注册 |

### 完整工具清单

| 工具名 | Phase | 后端 | 用途 |
|--------|-------|------|------|
| `math_prove_inequality` | 1 | Z3 + SymPy | 线性不等式可行性验证 |
| `math_backend_available` | 1 | — | 后端可用状态快速查询 |
| `math_asymptotic_estimate` | 2 | SymPy | 渐近主导阶估算 |
| `math_asymptotic_chain` | 2 | SymPy + 纯 Rust | 渐近链传递性验证 |
| `math_proof_dag_init` | 3 | 纯 Rust | 初始化 Blueprint-DAG |
| `math_proof_dag_decompose` | 3 | 纯 Rust | 分解 DAG 节点为 AND/OR 子目标 |
| `math_proof_dag_verify` | 3 | 纯 Rust | 递归验证整个 DAG |
| `math_proof_dag_status` | 3 | 纯 Rust | DAG 验证状态摘要 |
| `math_sympy_verify` | 4 | SymPy | 代数恒等式验证 |
| `math_sympy_simplify` | 4 | SymPy | 表达式化简 |
| `math_lean_verify` | 4 | Lean 4 | Lean 定理证明 |
| `math_backend_status` | 4 | — | 全面后端诊断 |

---

## §B: 不等式引擎协议

### 架构

```
LaTeX 表达式 → SymPy 解析 → 正则 fallback → Inequality struct
                                                      ↓
                                              Z3 子进程 (JSON stdin/stdout)
                                                      ↓
                                              FeasibilityResult
```

### 调用示例

```
math_prove_inequality(expression="x^2 + y^2 <= 1", timeout_ms=5000)
```

返回：
```json
{
  "check_name": "math_prove_inequality",
  "status": "Pass",
  "details": "Consistent. Model: x=0.5, y=0.5",
  "expression": "x^2 + y^2 <= 1"
}
```

### 安全约束

- **无 shell 注入**：用户输入仅通过 JSON stdin 传入 Python 子进程
- **超时保护**：默认 5s 超时，超时返回 Timeout 而非 panic
- **降级路径**：Z3 不可用时返回 Warn（小型系统降级提示）

---

## §C: 渐近分析协议

### 关系定义

| 符号 | 名称 | 定义 |
|------|------|------|
| ≲ | LessSim | `limsup|f/g| < ∞` |
| ≪ | MuchLess | `lim|f/g| = 0` |
| ≍ | Asymp | `0 < lim|f/g| < ∞` |

### 链验证

- **纯链**（全部 ≲ 或全部 ≪）：传递性自动 PASS
- **混合链**（含两种以上关系）：自动 WARN + 标记 human review required
- **SymPy 逐项验证**：对每条边计算 `limit(f/g)` 确认关系成立

### 调用示例

```
math_asymptotic_estimate(expression="n^2 + n*log(n)", variable="n", regime="oo")
```

返回：
```json
{
  "check_name": "math_asymptotic_estimate",
  "status": "Pass",
  "details": "n^2 + n*log(n) ~ n^2 (order: O(n^2)) as n→oo",
  "expression": "n^2 + n*log(n)"
}
```

```
math_asymptotic_chain(steps=[
  {"premise": "n", "conclusion": "n^2", "relation": "LessSim", "justification": "polynomial"},
  {"premise": "n^2", "conclusion": "2^n", "relation": "MuchLess", "justification": "exponential"}
], variable="n", regime="oo")
```

返回（混合链）：
```json
{
  "check_name": "math_asymptotic_chain",
  "status": "Warn",
  "details": "mixed chain (≲, ≪) — human review required for mixed relation chain",
  "steps": [...]
}
```

返回（纯链通过）：
```json
{
  "check_name": "math_asymptotic_chain",
  "status": "Pass",
  "details": "Pure chain (2 steps): all ≲ relations are consistent",
  "steps": [...]
}
```

---

## §D: Proof DAG 分解协议

### 架构（来自 LEAP arXiv:2606.03303）

```
Blueprint
  └── OR: 证明策略（至少一个子节点通过）
       ├── AND: 必需子目标（全部通过）
       │   ├── Leaf(InequalityEngine): "具体不等式"
       │   └── Leaf(SymPy): "代数恒等式"
       └── AND: 替代策略
           ├── Leaf(Z3): "约束一致性"
           └── Leaf(ManualProse): "手工证明段"
```

### 约束

- **ManualProse ≤ 30%**：手工证明段占总叶子数不超过 30%
- **AND 节点要求**：至少有一个非 ManualProse 子节点
- **单调精化**：DAG 结构只增不减（backtrack 是清除子节点回到 OR 粗粒度状态）
- **Round 语义**：每轮验证标记先前结果为 stale，结果带 `validated_at_round`

### 调用示例

```
math_proof_dag_init(goal="证明 AM-GM 不等式", name="am-gm")
math_proof_dag_decompose(parent_id="root", children=[
  {"OrNode": {"id": "s1", "label": "标准证明", "children": []}},
  {"OrNode": {"id": "s2", "label": "凸性法", "children": []}}
])
math_proof_dag_verify()
math_proof_dag_status()
```

---

## §E: 后端安装与诊断

### 必需安装

```bash
# Z3 + SymPy（不等式引擎和渐近分析）
uv pip install z3-solver sympy

# Lean 4（形式定理证明）
curl -L https://github.com/leanprover/elan/releases/download/v4.0.3/elan-x86_64-unknown-linux-gnu.tar.gz | tar xz
./elan-init
elan install lean4
```

### 一键诊断

```
math_backend_status()
```

返回各后端可用状态和安装指引：
```json
{
  "inequality_engine": {"available": true, "description": "Z3-based linear inequality verification"},
  "sympy": {"available": true, "description": "Symbolic identity simplification"},
  "lean": {"available": false, "description": "Lean theorem prover — not yet configured"},
  "install_hint": "uv pip install z3-solver sympy"
}
```

### 环境要求

- Python ≥ 3.11（通过 `uv` 管理）
- Rust toolchain（编译框架本身）
- Lean 4（可选，仅 `math_lean_verify` 需要）

---

## §F: 数学建模集成

与 `research-discovery` skill 的 `math_modeling`/`math_background_inquiry` lane 配合使用：

1. **建模阶段**（research-discovery）：提出问题 → 搜索文献 → 建立数学模型
2. **推导阶段**（math-derivation）：在模型基础上执行推导
3. **验证阶段**（formal-verification）：使用本工具链进行形式化验证
4. **审查阶段**（framework_quality_gate）：多轮对抗审查 → 收敛判定

### 激活条件

当以下任一条件满足时，路由到本工具链：
- 推导步骤需要 CAS/SMT 验证
- 存在渐近关系链（≲/≪/≍）
- 证明需要多轮 AND-OR 分解
- 要求 "verified" 或 "严审通过" 的结论

---

## §G: 多轮对抗验证

Proof DAG + Quality Gate 组合使用：

```
Round 1: Decompose → Verify → Status
Round 2: Decompose (refine) → Verify (stale previous) → Status
...
Round N: Converged (or max rounds reached)
```

每轮：
1. `math_proof_dag_decompose` 精化未充分验证的节点
2. `math_proof_dag_verify` 递增 round 编号，标记旧结果 stale
3. `math_proof_dag_status` 检查 ManualProse 比例和整体进度

### 收敛条件

- 所有叶子节点通过验证
- ManualProse 比例 ≤ 30%
- 连续 N 轮无新的分解变动
