---
allowed_tools:
- Read
- Write
- Edit
- Bash
- Glob
- Grep
- Agent
description: 行为守恒的三维度并行代码简化审查（复用/质量/效率）。默认 findings-only，可选自动修复模式。与 code-review-deep 互补。不用于找 bug 或安全漏洞——那属于 code-review-deep。集成框架生命周期。
metadata:
  platforms:
  - supported
  tags:
  - code-quality
  - simplification
  - refactoring
  - quality-gate
  - parallel-agents
  version: '1.0.0'
name: simplify
risk: low
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P2
session_start: preferred
source: project
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

- **Purpose**: 行为守恒的三维度并行代码简化审查 — 发现可简化的代码并生成 findings 报告，可选自动修复
- **Key Rules**: 默认只报不改；简化前定义行为契约；三维度子代理并行审查；简化前后跑测试门控；与 code-review-deep 互补
- **Trigger**: "simplify"、"代码简化"、"简化代码"、"清理代码"、"去重"、"/simplify"

<!-- full content below; load on demand -->

# Simplify — 行为守恒的代码质量门

行为守恒的三维度并行代码简化审查。默认只输出 findings 报告，不自动修改代码。与 `code-review-deep` 互补：review 是审查专家（findings-only 深度对抗式），simplify 是质量门（审查 + 可选修复，聚焦简化）。

## 默认姿态

- **Findings-only by default (hard stop)**: 收到 simplify 请求后，**不**编辑文件、不添加测试、不运行修复提交，除非用户**明确**退出只读模式（例如 "apply these"、"fix it"、传入 `--apply` 参数）。以 findings 报告结束，不进入执行。
- **行为守恒**: 简化绝不改变程序的外部行为。简化前必须建立行为契约（Behavior Contract），明确"什么不能变"。
- **三维度并行**: 复用、质量、效率三个子代理并行审查，聚合去重后输出统一报告。
- **测试门控**: 简化前后必须运行同一组测试，测试失败则自动回滚。
- **与 code-review-deep 互补**:
  - `code-review-deep` = 审查专家 — 深度对抗式审查，findings-only，聚焦正确性/安全/API 兼容性
  - `simplify` = 质量门 — 审查 + 可选修复，聚焦复用/质量/效率简化
  - 可串联使用: `/review` → `/simplify` → commit

## 行为契约（Behavior Contract）

简化前**必须**定义"什么不能变"。这是 simplify 的安全基石。

### 契约要素

| 维度 | 描述 | 示例 |
|------|------|------|
| **输入** | 函数/模块接受的参数与前置条件 | `parse_config(path) → path 必须存在` |
| **输出** | 返回值类型、结构、语义 | `返回 Vec<Record>，按 id 升序` |
| **副作用** | 文件 I/O、网络请求、数据库写入 | `写入 artifacts/log.json` |
| **错误行为** | 错误类型、panic 条件、graceful degradation | `Err(ConfigError) 不 panic` |
| **持久化** | 文件/数据库的状态一致性 | `write 后 fsync` |
| **排序** | 隐含的顺序保证 | `事件按 timestamp 排序` |
| **用户可见行为** | UI 输出、日志格式、CLI 输出 | `输出 JSON 格式不变` |

### 契约生成流程

1. **自动推断**: 从函数签名、文档注释、测试用例中推断行为约束
2. **代码内证据**: 检查 git diff 中的变更，识别被修改的公共 API 和行为边界
3. **显式声明**: 将推断结果写入契约，作为后续简化修改的不可变约束

### 契约违规处理

任何修改如果改变了契约中定义的行为 = **停下，请求用户确认**。不自动应用。

## 三维度并行审查架构

```
/simplify [focus on <area>]
│
├─ Phase 0: 行为契约 — 定义"什么不能变"
├─ Phase 1: 变更检测 — git diff 确定范围
├─ Phase 2: 三维度并行审查
│   ├── 子代理 A: 复用维度（重复/已有工具/可复用组件）
│   ├── 子代理 B: 质量维度（命名/分解/控制流/过度工程）
│   └── 子代理 C: 效率维度（冗余计算/N+1/并发机会）
├─ Phase 3: 聚合去重 → findings-only 报告
├─ Phase 4: [可选] --apply 自动修复 + 测试门控
└─ Phase 5: 持久化 simplification-pass.md
```

### Phase 0: 行为契约

在审查开始前，确定简化范围和行为边界。

```markdown
# Behavior Contract

## Scope
- Target: <files/modules from git diff or user指定>
- Baseline: <commit hash / branch>

## Invariants (what MUST NOT change)
- 输入/输出/副作用/错误行为/持久化/排序/UX

## Verification commands
- <test commands to run before and after>
```

### Phase 1: 变更检测

```bash
# 默认范围: 近期变更
git diff --name-only HEAD~1

# 用户指定范围
# /simplify focus on <path or area>
```

范围限定：
- 默认: git diff 中的近 1 次提交变更文件
- 可扩展: 用户指定路径/模块/关注点
- 超出范围的文件**不在审查范围内**

### Phase 2: 三维度并行审查

三个子代理**并行**执行，各自聚焦一个维度。每个子代理是只读的，只输出 findings 列表。

#### 子代理 A: 复用维度（Reuse）

**角色**: 识别重复逻辑、已有可复用组件、冗余实现。

**审查维度**:
- 同一模块内重复的代码块（copy-paste 逻辑）
- 已有工具函数/辅助模块可替代的重复实现
- 跨模块的相似模式（可提取为共享组件）
- 注释掉的代码（dead code，应删除）

**输出格式**:
```markdown
## Reuse Findings

### [Reuse-1] <file>:<line>
- **Issue**: <重复描述>
- **Existing solution**: <已有可复用函数/组件>
- **Impact**: <维护负担/一致性风险>
- **Suggestion**: <替换为已有实现>

### [Reuse-2] ...
```

#### 子代理 B: 质量维度（Quality）

**角色**: 识别命名问题、过度分解、控制流复杂度、过度工程。

**审查维度**:
- 误导性命名（变量/函数/类型名称与实际语义不符）
- 过度分解（函数拆得太细，增加认知负担无收益）
- 深层嵌套（可用守卫子句 flatten）
- 过度工程（不需要的抽象层、工厂模式、策略模式）
- 未使用的 imports、变量、参数
- 过时的 TODO/FIXME 注释

**输出格式**:
```markdown
## Quality Findings

### [Quality-1] <file>:<line>
- **Issue**: <质量问题描述>
- **Current complexity**: <当前复杂度>
- **Suggestion**: <简化建议>
- **Impact**: <可读性/维护性改善>

### [Quality-2] ...
```

#### 子代理 C: 效率维度（Efficiency）

**角色**: 识别冗余计算、不必要的分配、N+1 查询模式、并发机会。

**审查维度**:
- 冗余计算（同一值重复计算，应缓存）
- 不必要的内存分配（可就地操作）
- N+1 查询/请求模式（可批量处理）
- 串行执行可并行化的独立操作
- 未使用的返回值或中间变量

**输出格式**:
```markdown
## Efficiency Findings

### [Efficiency-1] <file>:<line>
- **Issue**: <效率问题描述>
- **Current cost**: <当前开销>
- **Suggestion**: <优化建议>
- **Measured impact**: <有 benchmark 则给出数据，无则注明>

### [Efficiency-2] ...
```

### Phase 3: 聚合与去重

三个子代理的 findings 汇总后：

1. **去重**: 多个维度发现的同一问题合并为一条，标注涉及的维度
2. **按影响排序**: 代码行数减少量、可读性改善、维护负担降低
3. **分级**:
   - **High**: 消除大量重复/明显过度工程/显著性能问题
   - **Medium**: 命名改善/中等重构/效率提升
   - **Low**: 微小清理/风格偏好/注释改善

### Phase 4: [可选] 自动修复 + 测试门控

仅在 `--apply` 模式或用户确认后执行。

**修复顺序**:
1. 先运行测试（基准）
2. 逐批应用修复（每批一个维度）
3. 每批后运行测试
4. 测试失败 → 回滚该批 → 记录到 deferred risks
5. 全部完成后运行完整测试

**修复约束**:
- 每次修改后验证行为契约不变
- 不修改公共 API 签名
- 不改变错误处理语义

### Phase 5: 持久化报告

生成 `simplification-pass.md` 并写入 skill 本地产物目录（`simplify/artifacts/`）。

## 允许清单（Allowlist）

以下简化操作在行为契约范围内**允许执行**:

| 操作 | 说明 |
|------|------|
| 删除未使用的 imports | 不影响运行时行为 |
| 删除未使用的变量/参数 | 不影响运行时行为 |
| 删除死分支 | 已证明不可达的分支 |
| 删除注释代码 | 已被版本控制保留 |
| 守卫子句替代深层嵌套 | 控制流等价变换 |
| 提取重复为命名函数 | 消除 copy-paste |
| 删除无语义包装层 | 减少间接层 |
| 窄范围机械重命名 | 提高可读性，不改语义 |
| 合并条件分支 | 逻辑等价简化 |
| 消除中间变量（当名称无信息量时） | 减少噪声 |

## 禁止清单（Denylist）

以下操作**绝不允许**，即使看起来"更简洁":

| 禁止操作 | 原因 |
|----------|------|
| 添加新功能 | 超出 simplify 范围 |
| 修改测试来通过简化代码 | 掩盖问题 |
| 范围蔓延到不相关重构 | 违反 scope 约束 |
| 性能优化（无 profile 证据） | 过早优化 |
| 削弱验证/授权逻辑 | 安全退化 |
| 削弱错误处理/重试逻辑 | 可靠性退化 |
| 削弱日志/审计轨迹 | 可观测性退化 |
| 删除无法证明不可达的代码 | 可能丢失边界情况 |
| 改变公共 API 签名 | 破坏兼容性 |
| 替换算法（即使"更优"） | 无 profile 证据不换 |

## 测试门控

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  Before:     │     │  Apply       │     │  After:      │
│  Run tests   │────▶│  simplifi-   │────▶│  Run same    │
│  (baseline)  │     │  cations     │     │  tests       │
└─────────────┘     └──────────────┘     └──────┬───────┘
                                                 │
                                        ┌────────┴────────┐
                                        │                  │
                                   Pass ▼             Fail ▼
                               ┌──────────┐     ┌──────────────┐
                               │ Commit   │     │ Rollback     │
                               │ changes  │     │ + record in  │
                               └──────────┘     │ deferred     │
                                                └──────────────┘
```

### 门控规则

1. **简化前**: 运行项目测试套件，记录基线结果
2. **简化后**: 运行**完全相同**的测试命令
3. **全部通过**: 修改保留
4. **任何失败**: 回滚该批修改，记录到 "Deferred risks"
5. **无测试覆盖**: 两条路径可选
   - **推荐**: 先写 characterization test（捕获当前行为的测试），再简化
   - **备选**: 跳过该文件，在报告中记录 "no test coverage — skipped"

### 测试命令检测

```bash
# 自动检测项目测试命令（按优先级）
# 1. Rust
cargo test
# 2. Node.js
npm test / yarn test / pnpm test
# 3. Python
pytest / python -m pytest
# 4. Go
go test ./...
# 5. 通用
make test
```

## 产出模板（simplification-pass.md）

```markdown
# Simplification Pass

- Trigger: <review advisory / user request / file path>
- Scope: <files/modules touched>
- Behavior contract: <what stayed identical>
- Verification before: <commands and result>
- Simplifications applied:
  - <file>: <change>
- Verification after: <commands and result>
- Deferred risks: <anything intentionally not simplified>
```

产出路径: `skills/simplify/artifacts/simplification-pass.md`

## 与 code-review-deep 的互补说明

| 维度 | code-review-deep | simplify |
|------|-----------------|----------|
| **定位** | 审查专家 | 质量门 |
| **默认行为** | findings-only（只审不改） | findings-only（只报不改） |
| **修改模式** | 用户明确退出只读才改 | `--apply` 或用户确认后修复 |
| **审查焦点** | 正确性/安全/API 兼容/依赖安全 | 复用/质量/效率简化 |
| **审查立场** | hostile-but-fair 对抗式 | 行为守恒 constructive |
| **子代理** | 并行 reviewer（read-only） | 三维度并行（read-only findings） |
| **产出** | severity-sorted findings | dimensional findings + 修复建议 |
| **生命周期位置** | 前后均可 | 实现 → simplify → 验证 |
| **风险等级** | medium | low（默认）/ medium（--apply） |

### 串联使用

```
/实现 (实现)
    ↓
/code-review-deep (深度审查 — 正确性/安全)
    ↓
/simplify (质量门 — 复用/质量/效率)
    ↓
/验证 (验证)
    ↓
commit
```

## 模式切换

### 默认模式: findings-only

```
/simplify
/simplify focus on <area>
```

- 输出三维度 findings 报告
- 不修改任何代码
- 等待用户决定下一步

### 自动修复模式: --apply

```
/simplify --apply
/simplify focus on <area> --apply
```

- 按 findings 报告自动应用修复
- 每批修复后运行测试门控
- 测试失败自动回滚
- 生成 simplification-pass.md

### Focus 模式

```
/simplify focus on auth module
/simplify focus on src/core/
```

- 聚焦特定文件/模块/关注点
- 三个子代理仍然并行，但 scope 限定在指定区域

## 跨宿主兼容

所有宿主共享同一份 `SKILL.md`，通过子代理并行执行三维度简化（具体子代理接入方式因宿主而异），行为契约和测试门控逻辑不变。

## References

- **Lens 目录**: [`references/simplify-dimensions.md`](references/simplify-dimensions.md) — 可扩展的简化维度目录
- **互补技能**: [`code-review-deep/SKILL.md`](../code-review-deep/SKILL.md) — 深度对抗式审查
- **框架生命周期**: `AGENTS.md` — lifecycle profile 与 review gate 集成
