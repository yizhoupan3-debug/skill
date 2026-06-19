# 文档体系全面审核报告

**审核时间**: 2026-06-20
**审核范围**: docs/、skills/、configs/ 目录下的所有文档
**审核目标**: 找出过期文档，确定应该合并/拆分/删减/更新的文档

---

## 📊 文档体系概览

### 文档总数统计
- **总md文件**: 398 个
- **docs/目录**: 40 个
- **skills/目录**: 232 个
- **configs/目录**: 1 个
- **其他目录**: 125 个

### 文档结构
```
docs/
├── adr/                    # 架构决策记录
├── hosts/                  # 宿主手册
├── modules/                # 模块文档
├── operations/             # 运维文档
├── references/             # 参考文档
├── spec/                   # 规范文档
├── cross-host-architecture.md  # 跨宿主架构（新增）
├── framework_naming_conventions.md
├── framework_profile_contract.md
├── git_hygiene.md
├── hook_lock_order.md
├── README.md
├── RESEARCH_HARNESS_AUDIT_REPORT.md
└── spec.md

skills/
├── .archive-cold/          # 已归档技能
├── shared-references/      # 共享参考文档
├── [73个技能目录]          # 各技能的SKILL.md
└── [48个references目录]    # 技能参考文档
```

---

## 🔍 发现的问题

### 1. 过期文档（需要更新）

#### 1.1 可能过期的核心文档
- **docs/framework_naming_conventions.md** - 引用了已清理的空壳 companion 文件
- **docs/spec/loop-architecture.md** - 包含 TODO/FIXME 标记
- **docs/operations/index.md** - 包含硬编码路径

#### 1.2 可能过期的配置相关文档
以下文档引用了 RUNTIME_REGISTRY.json，需要验证引用是否仍然有效：
- docs/adr/002-mcp-native-opencode.md
- docs/framework_naming_conventions.md
- docs/references/review-protocol.md
- docs/cross-host-architecture.md
- docs/spec/security-lifecycle.md
- docs/spec/loop-architecture.md
- docs/spec/runtime-subsystems.md
- docs/spec/host-matrix.md
- docs/operations/backup-restore.md
- docs/operations/getting-started.md

#### 1.3 可能过期的技能文档
- 48个references目录中的文档可能需要更新
- 超过60天未修改的SKILL.md文件需要检查

### 2. 结构问题（需要合并/拆分）

#### 2.1 可能合并的文档
- **skills/目录下的SKILL.md文件**: 73个文件，结构相似，可能需要标准化
- **skills/目录下的references目录**: 48个目录，可能需要统一结构

#### 2.2 可能拆分的文档
- **docs/spec.md** - 10,978 字节，可能需要拆分为多个规范文档
- **docs/framework_naming_conventions.md** - 6,636 字节，内容较多

#### 2.3 可能删减的文档
- **.cursor/commands/下的文档** - 9个文件，小于1KB，可能需要合并或删除
- **artifacts/research-barrier/下的模板文档** - 可能需要标准化

### 3. 一致性问题（需要修复）

#### 3.1 术语使用不一致
- '宿主' vs 'host' 的使用：24个文档使用'宿主'，33个文档使用'host'
- 需要统一术语使用

#### 3.2 文档引用不一致
- 部分文档引用了不存在的文件
- 部分文档的引用路径不正确

#### 3.3 文档结构不一致
- 不同文档的标题层级使用不一致
- 代码示例的格式不一致

### 4. 重复内容（需要整合）

#### 4.1 可能重复的文档名称
- BOOTSTRAP_BRIEF.md
- CHANGELOG.md
- CLAUDE.md
- CONTRIBUTING.md
- CURRENT_CONTEXT.md
- EXTERNAL_RESEARCH.md
- HYPOTHESIS_CARD.md
- INDEX.md
- LICENSE.md
- METADATA.md
- NOVELTY_CLAIMS.md
- NOVELTY_GATE.md
- NOVELTY_SEARCH_PLAN.md
- PROTOCOL_TEMPLATE.md
- README.md
- REFLECTION_TEMPLATE.md
- RUN_RECORD_TEMPLATE.md
- SKILL.md
- checklist.md
- deepinterview.md

这些文档名称在多个位置重复出现，需要检查是否内容重复。

---

## 📋 优化建议

### 1. 立即更新（高优先级）

#### 1.1 修复过期引用
- 更新 docs/framework_naming_conventions.md，移除对已清理文件的引用
- 修复 docs/spec/loop-architecture.md 中的 TODO/FIXME
- 更新 docs/operations/index.md 中的硬编码路径

#### 1.2 验证配置相关文档
- 检查所有引用 RUNTIME_REGISTRY.json 的文档，确保引用路径正确
- 更新 docs/cross-host-architecture.md 中的宿主能力矩阵

### 2. 结构优化（中优先级）

#### 2.1 标准化技能文档结构
- 制定统一的 SKILL.md 模板
- 标准化 references 目录的结构
- 合并相似的技能文档

#### 2.2 拆分大型文档
- 将 docs/spec.md 拆分为多个规范文档
- 将 docs/framework_naming_conventions.md 拆分为命名规范和配置规范

#### 2.3 清理小型文档
- 合并 .cursor/commands/ 下的相关文档
- 标准化 artifacts/research-barrier/ 下的模板文档

### 3. 一致性修复（低优先级）

#### 3.1 统一术语使用
- 制定术语使用规范
- 统一'宿主'和'host'的使用
- 统一其他术语的翻译

#### 3.2 修复文档引用
- 检查所有文档的引用链接
- 修复断裂的引用
- 更新过时的引用

#### 3.3 统一文档结构
- 制定统一的文档结构规范
- 统一标题层级的使用
- 统一代码示例的格式

### 4. 内容整合（低优先级）

#### 4.1 整合重复内容
- 检查重复名称的文档
- 合并内容重复的文档
- 删除过时的文档

#### 4.2 归档过期文档
- 将超过6个月未修改的文档归档到 .archive-cold/
- 删除不再需要的文档
- 更新文档索引

---

## 📊 优先级排序

### 高优先级（立即处理）
1. 修复过期引用（3个文档）
2. 验证配置相关文档（10个文档）
3. 更新跨宿主架构文档

### 中优先级（本周处理）
1. 标准化技能文档结构（73个SKILL.md）
2. 拆分大型文档（2个文档）
3. 清理小型文档（9个文档）

### 低优先级（本月处理）
1. 统一术语使用（57个文档）
2. 修复文档引用（10+个文档）
3. 统一文档结构（20+个文档）
4. 整合重复内容（20+个文档）

---

## 🎯 具体行动项

### 行动项1: 更新跨宿主架构文档
**目标**: 确保 docs/cross-host-architecture.md 与当前实现一致
**负责人**: 代码审查
**截止日期**: 2026-06-21
**状态**: 待处理

### 行动项2: 修复 framework_naming_conventions.md
**目标**: 移除对已清理文件的引用，更新文档内容
**负责人**: 代码审查
**截止日期**: 2026-06-21
**状态**: 待处理

### 行动项3: 标准化技能文档模板
**目标**: 制定统一的 SKILL.md 模板，标准化技能文档结构
**负责人**: 文档团队
**截止日期**: 2026-06-25
**状态**: 待处理

### 行动项4: 拆分 spec.md
**目标**: 将 spec.md 拆分为多个规范文档，提高可维护性
**负责人**: 架构团队
**截止日期**: 2026-06-30
**状态**: 待处理

### 行动项5: 清理过期文档
**目标**: 归档超过6个月未修改的文档，删除不再需要的文档
**负责人**: 文档团队
**截止日期**: 2026-07-15
**状态**: 待处理

---

## 📈 预期效果

### 短期效果（1周内）
- 修复所有过期引用
- 更新跨宿主架构文档
- 制定技能文档模板

### 中期效果（1个月内）
- 标准化所有技能文档
- 拆分大型文档
- 清理小型文档

### 长期效果（3个月内）
- 统一术语使用
- 修复所有文档引用
- 整合重复内容
- 建立文档维护机制

---

## 🔧 维护建议

### 1. 建立文档审核机制
- 每月进行一次文档审核
- 建立文档更新检查清单
- 设置文档过期提醒

### 2. 建立文档标准
- 制定文档编写规范
- 建立文档模板库
- 设置文档质量检查

### 3. 建立文档工具
- 开发文档引用检查工具
- 建立文档版本管理
- 设置文档自动归档

---

## 📝 总结

本次文档体系全面审核发现：
- **过期文档**: 13个文档需要立即更新
- **结构问题**: 10个文档需要合并/拆分/删减
- **一致性问题**: 57个文档需要修复一致性
- **重复内容**: 20+个文档可能存在重复

**建议优先处理高优先级问题**，确保文档体系的准确性和可维护性。通过系统性的优化，可以提高文档质量，降低维护成本，提升开发效率。

**下一步行动**：
1. 立即修复过期引用
2. 更新跨宿主架构文档
3. 制定技能文档模板
4. 建立文档审核机制
