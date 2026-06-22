---
last_verified: "2026-06-22"
depends_on:
  - ../spec.md
  - ../AGENTS.md
---

# ADR-009: 文档版本管理策略

## Status

Accepted (2026-06-22).

## Context

框架规约文档存在严重的版本漂移问题：

1. **版本标识混乱**：11 篇 spec 文档中，9 篇标为 `unified-v7`，1 篇主规约标为 `unified-v8.5`，另有 1 篇无版本标识。实际都在同一个代码库中维护，版本号与实际内容不匹配。
2. **正文与元数据不一致**：部分文档 frontmatter 标 v8.5，但正文引用 "v7 架构" 术语和已经废弃的接口名。
3. **无同步机制**：Phase 结束时不要求更新文档版本，文档迭代滞后于代码实现。
4. **无一致性校验**：缺少自动化检查手段，版本漂移需人工 review 发现。

## Decision

1. **统一 Frontmatter**：每篇 spec 文档必须在 frontmatter 中包含：
   ```yaml
   ---
   spec_version: "unified-v8"
   last_verified: "2026-06-22"
   applies_to_phase: "phase-5"
   ---
   ```
   - `spec_version`：当前规范版本（semver-like 但去点号，如 `unified-v8`）
   - `last_verified`：最后验证日期
   - `applies_to_phase`：当前适用的 Phase ID

2. **Phase 结束同步**：每个 Phase 结束前（closeout gate 的 checklist 项），开发/技术写作人员必须：
   - 更新受该 Phase 影响的所有 spec 文档的 frontmatter
   - 修复正文中引用已废弃/已迁移的术语和接口名
   - 在 PR 中标记 `docs: spec sync` 标签

3. **CI 验证文档版本一致性**：
   - 编写 `scripts/check-doc-versions.sh`，在 CI（pre-merge）执行
   - 检查项：
     a. 所有 `docs/spec-*.md` 文件存在 `spec_version` frontmatter（没有则警告）
     b. 所有 `spec_version` 值一致（允许 ±0.1 偏差，但禁止 major 差异如 v7 vs v8.5）
     c. `docs/spec.md` 的 `spec_version` 作为基准，其余文件与之对齐
   - 允许手动 override（通过 `// ci:doc-version-override` 注释）

4. **ADR 与 spec 的版本解耦**：ADR 不参与 spec 版本对齐。ADR 标记自己的 `last_verified` 和 `depends_on`（引用哪些 spec），但不受 spec frontmatter 约束。

## Consequences

- **优势**：
  - 文档版本状态一目了然，不再需要人工推测某篇 spec 对应哪个 Phase
  - CI 自动发现版本漂移，在新旧版本混合时提前告警
  - Phase closeout checklist 确保文档随代码同步更新
  - ADR ↔ spec 引用关系清晰，且 ADR 不增加版本对齐负担
- **代价**：
  - 初始同步成本：11 篇 spec 需一次性对齐 frontmatter 和版本号
  - CI 脚本需要维护（匹配 frontmatter 的 YAML 解析器）
  - 版本号策略虽灵活，但团队需约定何时 bump major/minor
- **迁移**：第一个 Phase closeout 前完成全部 frontmatter 初始化和版本对齐；后续 Phase 按规则增量同步。

## Related

- `docs/spec.md` — 主规约（版本基准）
- `docs/spec-*.md` 全部 11 篇规约文档
- `AGENTS.md` — 跨宿主叙述性政策（其版本与 spec 解耦）
- `scripts/check-doc-versions.sh` — CI 脚本（待创建）
