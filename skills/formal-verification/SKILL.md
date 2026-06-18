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

## Do not use

- 推导新公式或做数学证明 → 使用 `$math-derivation`
- 选择统计检验方法 → 使用 `$statistical-analysis`
- 纯符号化简无需验证 → 在当前 coding context 直接回答
- 需要文献调研或理论背景 → 使用 `$research-discovery`

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

可执行脚本：[`scripts/verify/formal.sh`](../../scripts/verify/formal.sh)

```bash
# 检查 SymPy 表达式恒等性：
EXPR="sin(x)**2 + cos(x)**2 - 1" scripts/verify/formal.sh

# 检查量纲报告：
DIMENSION_FILE=dimension_report.txt scripts/verify/formal.sh
```

| # | 检查名 | PASS 条件 |
|---|--------|-----------|
| 1 | CAS identity 化简 | SymPy simplify(expr) == 0 |
| 2 | SMT 预期一致性 | Z3 check() == sat |
| 3 | Witness 一致性 | 代入特例值后左右两边一致 |
| 4 | 量纲检查 | 每步方程左右两侧量纲相同 |
| 5 | 步骤依赖图完整性 | 无悬空引用（每步的前置步骤已定义） |

## References

- math-derivation skill：[`../math-derivation/SKILL.md`](../math-derivation/SKILL.md)（推导能力与符号计算知识库）
- framework formal_toolchain（Rust）：`core/runtime-core/src/contracts/formal_toolchain.rs`（CAS/SMT token 检测）

## Integration

前门 skill 在以下时机内联调用本 skill：

- **research-execution**：数学推导完成后，做形式化门禁检查

调用方式：将推导步骤序列传入，按验证清单逐项执行，FAIL 项作为 blocker 回写。
