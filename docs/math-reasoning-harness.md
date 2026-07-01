---
last_verified: "2026-07-01"
---

# Math Reasoning Harness

数学验证工具链 — ResearchHarness 的形式化验证后端。

## §A: 工具链总览

四层分离架构：

| 层 | 位置 | 职责 |
|----|------|------|
| **Feature** | `core/research-harness/src/verification/*.rs` | 纯业务逻辑：类型、SymPy/Z3/Lean 调用、验证函数 |
| **Gate** | `core/research-harness/src/verification/*_gate.rs` | Quality Gate `GateChecker` 适配层：从 `CheckContext` 提取参数，调用 Feature 层，包装结果为 `Finding` 列表 |
| **Tool** | `core/research-harness/src/mcp_tools.rs` | MCP dispatch：JSON 参数提取 → feature/gate 层调用 → JSON 格式化 |
| **Schema** | `core/research-harness/src/mcp/mod.rs` | MCP schema 定义（input schema、工具描述） |

> **路径说明**：MCP 工具调用经 `mcp_tools.rs` 直接调用 Feature 层；Gate 层通过 Quality Gate 系统（`framework_quality_gate` stdio 命令/任务收尾）调用，注册于 `RUNTIME_REGISTRY.json` 中 scene=research。

### 完整工具清单

| 工具名 | 后端 | 用途 |
|--------|------|------|
| `math_prove_inequality` | Z3 (非线性/SMT) + minilp (线性, 自动降级) | 不等式可行性验证 |
| `math_backend_available` | Python 探测 | 后端可用状态查询（Z3/SymPy/Lean） |
| `math_asymptotic_estimate` | 纯 Rust (分类) + regime 转换 | 渐近主导阶估算（支持 ∞ 和 →0） |
| `math_asymptotic_chain` | 纯 Rust (传递性检查) | 渐近链传递性验证 |
| `math_proof_dag_init` | 纯 Rust | 初始化 Blueprint-DAG |
| `math_proof_dag_decompose` | 纯 Rust | 分解 DAG 节点为 AND/OR 子目标 |
| `math_proof_dag_verify` | 真实后端调用 | 递归验证整个 DAG（调用 Z3/SymPy/Lean 后端） |
| `math_proof_dag_status` | 纯 Rust | DAG 验证状态摘要 |
| `math_sympy_verify` | SymPy (后端可用时) + 纯 Rust 降级 | 代数恒等式验证 |
| `math_sympy_simplify` | SymPy (后端可用时) + 纯 Rust 降级 | 表达式化简 |
| `math_lean_verify` | Lean 4 (系统 PATH) | Lean 定理证明 |
| `math_backend_status` | 综合 | 全面后端诊断 |

### 架构图

```
用户/Agent
    │
    ├── MCP 工具调用 (math_*, research_verification_*)
    │   │
    │   ▼
    │   mcp/mod.rs → dispatch()
    │   │
    │   ▼
    │   mcp_tools.rs → math_tool_dispatch() / verification_tool_dispatch()
    │   │
    │   ├── "math_prove_inequality"     ─→ inequality.rs
    │   │    ├── 线性 → minilp (纯 Rust LP 求解器)
    │   │    └── 非线性 → python_bridge → Z3 (Python 子进程)
    │   │
    │   ├── "math_sympy_*"             ─→ sympy_bridge.rs
    │   │    ├── SymPy 可用 → python_bridge → SymPy
    │   │    └── SymPy 不可用 → symbolic.rs (纯 Rust 符号引擎)
    │   │
    │   ├── "math_asymptotic_*"        ─→ asymptotic.rs
    │   │    ├── regime 转换 (oo → 恒等, 0 → x → 1/x)
    │   │    └── symbolic.rs (纯 Rust 增长分类)
    │   │
    │   ├── "math_proof_dag_*"         ─→ proof_dag.rs
    │   │    └── verify() → 调用真实后端验证叶子节点
    │   │
    │   ├── "math_backend_available"   ─→ python_bridge + lean_bridge
    │   │    └── Python 子进程探测 → 缓存 30s
    │   │
    │   ├── "math_lean_verify"         ─→ lean_bridge.rs
    │   │    └── 系统 PATH → lean 子进程
    │   │
    │   ├── "math_z3_*"               ─→ z3_bridge.rs
    │   │
    │   ├── "research_verification_prose"         ─→ prose_qc.rs
    │   ├── "research_verification_statistical"    ─→ statistical.rs
    │   ├── "research_verification_literature"     ─→ literature.rs
    │   ├── "research_verification_structure"      ─→ structure.rs
    │   ├── "research_verification_reproducibility" ─→ reproducibility.rs
    │   └── "research_verification_formal"          ─→ formal.rs
    │
    ├── Quality Gate 系统 (framework_quality_gate / 任务收尾)
    │   │
    │   ▼
    │   qg_entry::trigger() → qg_route::evaluate_qg_route()
    │   │
    │   ▼
    │   Gate Checkers (verification/*_gate.rs)
    │   ├── inequality_gate.rs       ─→ inequality.rs       (research scene)
    │   ├── sympy_bridge_gate.rs     ─→ sympy_bridge.rs     (research scene)
    │   ├── asymptotic_gate.rs       ─→ asymptotic.rs       (research scene)
    │   ├── formal_gate.rs           ─→ formal.rs           (research scene)
    │   ├── literature_gate.rs       ─→ literature.rs       (research scene)
    │   ├── prose_qc_gate.rs         ─→ prose_qc.rs         (research scene)
    │   ├── reproducibility_gate.rs  ─→ reproducibility.rs  (research scene)
    │   ├── statistical_gate.rs      ─→ statistical.rs      (research scene)
    │   ├── structure_gate.rs        ─→ structure.rs        (research scene)
    │   └── symbolic_gate.rs         ─→ symbolic.rs         (research scene)
    │
    ▼
Python 子进程 (uv run -m math_backend)
    ├── SymPy → sympy_ops.py
    │    └── simplify, verify, expand, factor, series,
    │        differentiate, integrate, solve, trig_simplify,
    │        dimension_propagate
    └── Z3 → z3_ops.py
         └── check (单不等式), check_system (多约束),
             optimize (优化)
```

> **注**：灰色行 `research_verification_*` 对应 `verification/` 中的特征模块，MCP 工具直接调用；Gate Checker 层通过 Quality Gate 系统（`framework_quality_gate` stdio 命令及任务收尾流程）统一评估。所有 Gate 均在 `RUNTIME_REGISTRY.json` 中以 `"scene": "research"` 注册，运行时由 `build.rs` 编译生成注册函数。

---

## §B: 不等式引擎协议

### 架构

```
表达式 → is_nonlinear? 检测
    │
    ├── 线性 (无 ^, sin, cos 等)
    │   └── LaTeX 正则解析 → Inequality struct → minilp LP 求解器
    │
    └── 非线性 (含幂运算、三角等)
        └── python_bridge → Z3 SMT 求解器 (JSON stdin/stdout)
            └── AST 解析 (支持 Reals, sin, cos, sqrt, abs, And/Or/Not)
```

### 调用示例

```
# 线性不等式 (minilp)
math_prove_inequality(expression="x + y <= 10", timeout_ms=5000)

# 非线性不等式 (Z3)
math_prove_inequality(expression="x^2 + y^2 <= 1", timeout_ms=5000)
```

返回：
```json
{
  "check_name": "math_prove_inequality",
  "status": "Pass",
  "details": "Consistent (Z3 sat). Model: x=0.0, y=0.0"
}
```

### 安全约束

- **无 shell 注入**：用户输入仅通过 JSON stdin 传入 Python 子进程
- **超时保护**：默认 5s 超时，超时返回 Timeout 而非 panic
- **降级路径**：Z3 不可用时返回 Warn（非线性不等式无法求解）

---

## §C: 渐近分析协议

### 关系定义

| 符号 | 名称 | 定义 |
|------|------|------|
| ≲ | LessSim | `limsup|f/g| < ∞` |
| ≪ | MuchLess | `lim|f/g| = 0` |
| ≍ | Asymp | `0 < lim|f/g| < ∞` |

### Regime 支持

| regime 参数 | 变换 | 语义 |
|-------------|------|------|
| `"oo"` 或 `"inf"` | 无变换 | x→∞ |
| `"0"` 或 `"zero"` | x → 1/x | x→0 |
| 其他 | 无变换 | x→∞（默认） |

### 链验证

- **纯链**（全部 ≲ 或全部 ≪）：传递性自动 PASS
- **混合链**（含两种以上关系）：自动 WARN + 标记 human review required
- **逐项验证**：对每条边使用 `classify_growth` 确认关系成立

---

## §D: Proof DAG 分解协议

### 架构

```
Blueprint
  └── OR: 证明策略（至少一个子节点通过）
       ├── AND: 必需子目标（全部通过）
       │   ├── Leaf(InequalityEngine): "具体不等式"  → inequality.rs
       │   ├── Leaf(SymPy): "代数恒等式"              → sympy_bridge.rs
       │   ├── Leaf(Z3): "约束一致性"                 → Z3 后端
       │   └── Leaf(Asymptotic): "渐近关系"           → asymptotic.rs
       └── AND: 替代策略
           ├── Leaf(Lean): "Lean 定理证明"            → lean_bridge.rs
           └── Leaf(ManualProse): "手工证明段"        → Skip
```

### 约束

- **ManualProse ≤ 30%**：手工证明段占总叶子数不超过 30%
- **AND 节点要求**：至少有一个非 ManualProse 子节点
- **Round 语义**：每轮验证标记先前结果为 stale，结果带 `validated_at_round`

---

## §E: 后端安装与诊断

### 必需安装

```bash
# Z3 + SymPy（不等式引擎和渐近分析）
uv pip install z3-solver sympy

# Lean 4（形式定理证明，可选）
curl -L https://github.com/leanprover/elan/releases/download/v4.0.3/elan-x86_64-unknown-linux-gnu.tar.gz | tar xz
./elan-init
elan install lean4
```

### 一键诊断

```
math_backend_available(backend="all")
```

返回各后端可用状态：
```json
{
  "backends": {
    "lean": {"available": false, ...},
    "sympy": {"available": true, "version": "1.14.0", ...},
    "z3": {"available": true, "version": "4.16.0", ...},
    "python_backend": true
  },
  "summary": "SymPy: ✅ (v1.14.0), Z3: ✅ (v4.16.0), Lean: ❌, Python backend: ✅"
}
```

### 环境要求

- Python ≥ 3.12（通过 `uv` 管理）
- `z3-solver` （用于非线性不等式求解）
- `sympy` （用于 CAS 验证）
- Rust toolchain（编译框架本身）
- Lean 4（可选，仅 `math_lean_verify` 需要）

---

## §F: Python 后端协议

> **说明**：本节文档记录 Python 子进程 (`uv run -m math_backend`) 的内部协议 API，包含所有后端支持的 14 项操作。其中部分操作通过 Rust 层的 MCP 工具直接暴露给用户；其余操作用现有的 MCP 工具内部间接调用，或仅作为 Python 层 API 保留（暂未通过 MCP 暴露）。下表的 **MCP 可用性** 列标注了各操作的对外暴露情况。

### 通信格式

Request → stdin:
```json
{"id": 1, "op": "sympy_simplify", "params": {"expression": "sin(x)**2 + cos(x)**2"}}
```

Response → stdout:
```json
{"status": "ok", "result": {"result": "1"}, "id": 1}
```

Error → stdout:
```json
{"id": 1, "status": "error", "error": "SymPy simplify failed: ..."}
```

### 支持操作

| op | 模块 | 功能 | MCP 可用性 |
|----|------|------|-----------|
| `backend_status` | 综合 | 查询所有后端版本和可用性 | ✅ `math_backend_available` / `math_backend_status` |
| `sympy_simplify` | SymPy | 表达式化简（自动 trig/代数） | ✅ `math_sympy_simplify` |
| `sympy_verify` | SymPy | 恒等式验证 | ✅ `math_sympy_verify` |
| `sympy_expand` | SymPy | 多项式展开 | ❌ 仅 Python 层 API |
| `sympy_factor` | SymPy | 因式分解 | ❌ 仅 Python 层 API |
| `sympy_series` | SymPy | 级数展开 | ❌ 仅 Python 层 API |
| `sympy_differentiate` | SymPy | 符号微分 | ❌ 仅 Python 层 API |
| `sympy_integrate` | SymPy | 符号积分 | ❌ 仅 Python 层 API |
| `sympy_solve` | SymPy | 方程求解 | ❌ 仅 Python 层 API |
| `sympy_trig_simplify` | SymPy | 三角恒等式化简 | ❌ 仅 Python 层 API |
| `sympy_dimension_propagate` | SymPy | 维度传播分析 | 🔹 `research_verification_formal`(dimensional) 内部调用 |
| `z3_check` | Z3 | 单不等式 SMT 求解 | ✅ `math_prove_inequality` |
| `z3_check_system` | Z3 | 多约束系统求解 | ❌ 仅 Python 层 API |
| `z3_optimize` | Z3 | 带目标函数的优化 | ❌ 仅 Python 层 API |

---

## §G: 量纲分析

### 检查类型

1. **简单集合比对**（`check_dimensional_consistency`）：提取 `[L], [M], [T]` 标注，两侧集合相等则通过
2. **启发式传播**（本地）：给定已知变量的维度，通过乘积/除法的指数运算传播维度
3. **SymPy 传播**（后端可用时）：调用 SymPy 单位系统进行精确维度分析

### 传播示例

```
方程: F = m * a
维度映射: {F: "L*M*T^-2", m: "M", a: "L*T^-2"}

左侧计算: L*M*T^-2
右侧计算: M * (L*T^-2) = L*M*T^-2
结果: 一致 (consistent=True)
```

---

## §H: 数学建模集成

与 `$research`（discovery lane）的 `math_modeling`/`math_background_inquiry` lane 配合使用：

1. **建模阶段**（$research discovery lane）：提出问题 → 搜索文献 → 建立数学模型
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

## §I: 多轮对抗验证

Proof DAG + Quality Gate 组合使用：

```
Round 1: Decompose → Verify (调用后端) → Status
Round 2: Decompose (refine) → Verify (stale previous) → Status
...
Round N: Converged (or max rounds reached)
```

每轮：
1. `math_proof_dag_decompose` 精化未充分验证的节点
2. `math_proof_dag_verify` 递增 round 编号，标记旧结果 stale，真实调用后端验证叶子
3. `math_proof_dag_status` 检查 ManualProse 比例和整体进度

### 收敛条件

- 所有叶子节点通过验证（或可接受的状态）
- ManualProse 比例 ≤ 30%
- 连续 N 轮无新的分解变动
