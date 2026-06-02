# P2 优化任务：Manifest Schema 与路径分析

> 生成时间：2026-06-02 | 任务来源：P2-1 / P2-2 / P2-4

---

## 任务1: Top Skill 版本化检查（P2-4）

所有 10 个目标 skill 均已包含 `metadata.version`，无需新增或更新：

| Skill | Version | Frontmatter 位置 |
|-------|---------|------------------|
| agent-swarm-orchestration | 1.1.2 | L27 |
| code-review-deep | 1.2.3 | L36 |
| gitx | 1.0.3 | L32 |
| paper-workbench | 1.16.0 | L108 |
| plan-mode | 1.6.6 | L29 |
| skill-framework-developer | 3.3.1 | L59 |
| discussx | 0.2.1 | L17 |
| planx | 0.1.0 | L18 |
| implementx | 0.1.0 | L19 |
| verifyx | 0.2.0 | L19 |

**结论**：全部就绪，无需修改。

---

## 任务2: SKILL_MANIFEST.json 全 null 字段分析（P2-1）

Manifest 包含 42 个 skill，schema_version = `skill-manifest-v2`，plugin_abi_version = `skill-plugin-abi-v1`。

### 全 null 字段（100%）

| 字段 | null 比例 | 说明 |
|------|-----------|------|
| `disableModelInvocations` | 42/42 (100%) | 完全未从 SKILL.md frontmatter 的 `disable-model-invocation` 映射 |
| `context` | 42/42 (100%) | 未使用；可能预留用于 skill 上下文注入 |

### 几乎全 null 字段（>90%）

| 字段 | null 比例 | 非 null 示例 |
|------|-----------|-------------|
| `model` | 40/42 (95%) | `discussx` = `haiku`, `planx` = `sonnet` |
| `allowedTools` | 39/42 (93%) | `code-review-deep` = `['Read','Bash','Agent']`, `paper-workbench` = `['Read','Write','Edit','Bash','WebSearch','WebFetch']`, `slides` = `['Read','Write','Edit','Bash']` |

### 偶发 null 字段

| 字段 | null 比例 | null 的 skill |
|------|-----------|--------------|
| `gate` | 2/42 (5%) | `token-optimization`, `mcp-server-management` |

### 建议

1. **`disableModelInvocations`**：Rust sync 工具链应从 SKILL.md frontmatter 的 `disable-model-invocation` 字段映射。目前完全未映射，导致宿主层无法通过 manifest 读取此约束。
2. **`context`**：如果暂无用途，可在 schema 中标记为 optional 并从 keys 列表降级；否则需定义其数据来源。
3. **`allowedTools` / `model`**：多数 skill 未声明，这是合理的（使用默认值）。但 Rust sync 仍应正确映射已声明的值。
4. **`gate` null**：`token-optimization` 和 `mcp-server-management` 的 SKILL.md 中可能缺少 `routing_gate` 字段，sync 时落为 null。建议检查这两个 skill 的 frontmatter 并补充 gate 声明。

---

## 任务3: 嵌套路径分析（P2-2）

### 现状

标准结构为 `skills/<slug>/SKILL.md`（42 个 skill 中 41 个遵循此模式）。

唯一例外：

```
skills/primary-runtime/
  references/              # 共享参考文档（非 skill）
    artifact-protocol.md
  spreadsheets/            # 嵌套 skill
    SKILL.md
    agents/openai.yaml
    assets/file-spreadsheet.png
    references/api-surface.md
    references/workflow.md
    references/xlsx-rust-workflow.md
    style_guidelines.md
    templates/financial_models.md
```

manifest 中的映射：`spreadsheets` -> `skills/primary-runtime/spreadsheets/SKILL.md`

### 与标准路径的差异

| 维度 | 标准 `<slug>` | `primary-runtime/spreadsheets` |
|------|--------------|-------------------------------|
| 路径 | `skills/<slug>/` | `skills/primary-runtime/spreadsheets/` |
| 路径深度 | 2 层 | 3 层 |
| 共享资源 | 无 | `primary-runtime/references/` 存在共享 artifact protocol |
| skill_path | `skills/<slug>/SKILL.md` | `skills/primary-runtime/spreadsheets/SKILL.md` |

### 建议

**不建议立即迁移路径**，原因：
1. `primary-runtime` 是一个有意义的分组命名空间，未来可能容纳更多 runtime 级 skill（如 sheet-cell、csv-wizard 等）。
2. 共享的 `references/artifact-protocol.md` 被 spreadsheets 内部引用，迁移到平级会破坏这一组织关系。
3. 路由层已正确解析 `skill_path`，功能无损。

**如果要规范化**，建议方向：
- 保持 `primary-runtime/` 作为命名空间目录
- 在路由文档中显式记录此例外模式，避免后续贡献者困惑
- 考虑在 `SKILL_MAINTENANCE_GUIDE.md` 中增加「命名空间目录」的说明段落
