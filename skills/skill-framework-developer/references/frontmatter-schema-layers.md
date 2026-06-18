# SKILL.md Frontmatter Schema 分层定义

> 本文件定义所有 skill 的 SKILL.md frontmatter 字段分层规范。
> 基于 36 个活跃 skill 的实际使用频率分析（2026-06-02）。

## L0 — 必选字段（100% 覆盖率，所有 skill 必须声明）

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | skill 唯一标识符，与目录名一致 |
| `description` | string/multiline | 能力描述，**无宿主化**（不提及特定宿主名） |
| `metadata` | object | 包含 `version`、`platforms`、`tags` |
| `routing_layer` | string | L0/L1/L2/L3/L4 |
| `routing_owner` | string | owner/gate/overlay |
| `routing_gate` | string | gate 类型（none/artifact/source/evidence/delegation 等） |
| `session_start` | string | preferred/required/n/a |
| `trigger_hints` | list | 自然语言触发词列表 |
| `user-invocable` | boolean | 用户是否可直接调用（owner=true, gate=false） |
| `disable-model-invocation` | boolean | 模型是否不可自动触发（默认 true） |

## L1 — 推荐字段（72%+ 覆盖率，建议声明）

| 字段 | 类型 | 说明 | 覆盖率 |
|------|------|------|--------|
| `risk` | string | low/medium/high | 72% (26/36) |
| `source` | string | local/community/community-adapted/external | 72% (26/36) |
| `routing_priority` | string | P0/P1/P2/P3 | 64% (23/36) |

## L2 — 扩展字段（按需声明，仅特定 skill 类型需要）

| 字段 | 类型 | 适用场景 | 覆盖率 |
|------|------|----------|--------|
| `allowed_tools` | list | 声明允许使用的工具列表 | 33% (12/36) |
| `approval_required_tools` | list | 需要用户批准的工具 | 33% (12/36) |
| `network_access` | string | conditional/none/always | 31% (11/36) |
| `filesystem_scope` | list | repo/artifacts/global 等 | 31% (11/36) |
| `framework_roles` | list | planner/detector/verifier 等框架角色 | 25% (9/36) |
| `artifact_outputs` | list | 产出的文件列表 | 25% (9/36) |
| `framework_phase` | int/string | 框架阶段标识 | 22% (8/36) |
| `framework_contracts` | object | emits/consumes 契约 | 22% (8/36) |
| `short_description` | string | 简短描述（用于索引） | 17% (6/36) |
| `runtime_requirements` | object | 命令/工具依赖 | 17% (6/36) |
| `routing_gate_evidence` | string | gate 触发的证据描述 | 8% (3/36) |
| `bridge_behavior` | string | 跨宿主桥接行为 | 6% (2/36) |

## 默认值策略

- `user-invocable`: owner 类默认 `true`，gate/overlay 类默认 `false`
- `disable-model-invocation`: 所有类型默认 `true`
- `session_start`: owner 类默认 `n/a`（用户按需调用），gate 类默认 `required`
- `routing_priority`: 未声明时默认 `P2`

## 无宿主化约束

description 和通用 instructions 中**禁止**提及特定宿主名（Codex、Claude、Cursor 等）。
仅在明确描述宿主差异的技术段落中可提及宿主名（如 hooks 集成、review gate 差异等）。
