---
allowed_tools:
- Read
- Bash
- Glob
- Grep
- Agent
description: 行为守恒的双维度并行代码简化审查（复用+质量）。默认 findings-only，与 code-review-deep 互补。不用于找 bug 或安全漏洞。
metadata:
  platforms:
  - supported
  tags:
  - code-quality
  - simplification
  - refactoring
  version: '2.1.0'
name: simplify
scene: code_quality
risk: medium
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P2
session_start: preferred
source: local
trigger_hints:
- /simplify
- clean up code
- code simplify
- simplify
- 代码简化清理
- 代码简化
- 去重
- 简化代码
- 质量门
- 过度工程
- 重构简化
---
## Quick Ref

- **Purpose**: 行为守恒的代码简化审查（复用+质量），必须产出具体的、可执行的 finding（附行号），不打空炮
- **Key Rules**: 默认只报不改；行为守恒；小范围直审不 spawn、大范围 spawn 双维度；与 code-review-deep 互补
- **Trigger**: `/simplify`、"代码简化"、"去重"。

---

# Simplify — 代码简化质量门

行为守恒的代码简化审查。默认 findings-only，不修改代码。

**与 code-review-deep 互补**：review 对抗审正确性/安全，simplify 审复用/质量。

---

## 输出格式（硬约束）

```
[High/R1] path:line — 完全相同的 15 行数据库查询在 3 个 handler 中各出现一次 → 提取到 dao.query()
[Med /Q3] path:line — 4 层嵌套 → 守卫子句 flatten
[Low /Q1] path:line — 布尔变量名 should_do 未表达 true 时做什么 → should_show_debug
```

每行一个 finding。**不要**分段叙述、不写摘要、不写"建议"段落。`[Severity/Dimension]` 前缀。

追加一行结尾（可选）：
```
Found 4 items (High:1, Med:2, Low:1)
```

---

## 自适应深度

| 范围 | 策略 |
|------|------|
| ≤3 files, ≤80 lines changed | 主会话直审，不 spawn 子代理 |
| >3 files 或 >80 lines | spawn Reuse + Quality 两个子代理并行 |

---

## 具体检测模式（每类找 1-3 条最严重的即可，不要堆数量）

### Reuse 维度 — 找结构重复

**不要**说"代码有些重复，可以考虑提取函数"。**要**指具体行号：

```
✅ [High/R1] src/order.rs:120-135 — 与 src/invoice.rs:45-60 的折扣计算完全一致，仅变量名不同 → 提取为 shared::calc_discount()
✅ [Med/R4] src/parser.rs:88 — 手写 CSV 解析器，csv crate 直接支持 → 替换
✅ [Med/R2] src/config.rs:30 — 自实现 load_env，项目内已有 dotenv::load() → 复用
```

**典型不报模式**（噪音过滤器）：
```
❌ "建议将函数拆分为更小的函数" — 无具体理由
❌ "代码可以重构以提高可读性" — 空话
❌ "建议使用 Optional 而不是传递 null" — 除非当前代码明确因此产生 NPE
```

### Quality 维度 — 找过度/不足/混乱

只报以下**六种具体模式**：

**Q1 过度工程**：单实现接口、工厂 for 单消费者、抽象的缓存层但只有一种后端

```
✅ [High/Q1] src/service.rs:15 — ServiceFactory.create() 始终返回 PostgresService，唯一调用方 → 删工厂直接 new
✅ [Med/Q1] src/cache.rs:25 — Cache trait 只有 MemoryCache 一个实现 → 去 trait
```

**Q2 深层嵌套**：超过 3 层 if/match/for 嵌套

```
✅ [High/Q2] src/process.rs:40 — 4 层嵌套（validate→auth→check→execute）→ 守卫子句 flatten
```

**Q3 超长函数**：超过 80 行的单函数

```
✅ [Med/Q3] src/importer.rs:120 — 120 行函数，混合 CSV 解析 + 数据验证 + 数据库写入 → 拆为 parse/validate/persist
```

**Q4 魔法数字/字符串**：未命名的字面量常量

```
✅ [Low/Q4] src/tax.rs:30 — `if status == 3` → const STATUS_ACTIVE = 3
```

**Q5 过度参数化**：超过 6 个参数的函数

```
✅ [Low/Q5] src/api.rs:50 — create_order(uid, pid, qty, addr, note, coupon, gift) 7 参数 → 提取 CreateOrderRequest 结构体
```

**Q6 死代码**：注释掉的代码、永不调用的导出函数

```
✅ [Med/Q6] src/legacy.rs — entire file is dead: last call site was removed in commit a1b2c3d
```

**典型不报模式**：
```
❌ "命名可以更清晰" — 没有证据表明引起混淆
❌ "建议添加更多注释" — simplify 不审注释
❌ "函数太长建议拆分" — 无具体拆分方案，就是空话
```

---

## 严重程度标准

| 级别 | 定义 | 示例 |
|------|------|------|
| High | 缺陷：会导致维护问题/bug | 重复代码（改了1处忘改另1处）、死代码（迷惑读者） |
| Med | 可改进：明显降低可读性但不至于出 bug | 深层嵌套、超长函数 |
| Low | 微优化：nice to have | 魔法数字命名、单参数命名 |

**不报 noise**：风格偏好、linter 已覆盖的规则、你个人觉得"更好的写法"。

---

## 子代理 prompt（仅大范围时使用）

### Reuse agent

```
找出重复代码。具体要求：
1. 同一个代码块（≥5 行）出现在 2 个以上位置
2. 项目内已有工具函数做同一件事，但新代码自己重新实现了
3. 注释掉的代码（由于 VCS，应删除）
4. 标准库或已引入依赖有现成替代

每类最多报 2 条最严重的。不要报仅语义相似但实现不同的代码。
输出格式：[Severity/Rn] path:line — issue → suggestion
```

### Quality agent

```
找出代码质量问题。只报以下 6 类：
1. 过度工程：单实现接口/工厂/不必要的抽象（High）
2. 深层嵌套：>3 层（High/Med）
3. 超长函数：>80 行混合多个关注点（Med）
4. 魔法数字/字符串：未命名字面量（Low）
5. 过度参数化：>6 个参数（Low）
6. 死代码：注释代码/永不调用（Med）

每类最多报 1 条最严重的。宁缺毋滥。
不要报：命名风格偏好、注释缺失、linter 已覆盖的规则。
输出格式：[Severity/Qn] path:line — issue → suggestion
```

---

## 禁止事项

- 不添加新功能
- 不改公共 API 签名
- 不削弱验证/安全/错误处理/日志
- 不替换算法（无 profile 证据）
- 不删除无法证明不可达的代码
- 修改变更前须确认行为不变

---

## References

- [简化维度目录](references/simplify-dimensions.md) — 扩展维度参考
- [code-review-deep](../code-review-deep/SKILL.md) — 深度对抗式审查
