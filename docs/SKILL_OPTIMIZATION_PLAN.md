# Skill 仓库深度审计 & 优化 Plan

> 多agent并行审计（11 agents, 396 tool calls, ~21min）
> 审计日期：2026-06-02

---

## 一、审计全景

| 维度 | 发现数 | 严重 | 中等 | 轻微 |
|------|--------|------|------|------|
| 架构一致性 | 11 | 0 | 4 | 7 |
| 过时设计 | 20 | 1 | 10 | 9 |
| 内容质量 | 15 | 4 | 6 | 5 |
| 集成互操作 | 10 | 1 | 4 | 5 |
| **合计** | **56** | **6** | **24** | **26** |

---

## 二、6大核心问题（按严重程度排序）

### Critical-1: "Codex" 旧称残留（9个skill）

**涉及**：gitx, deepinterview, agent-swarm-orchestration, skill-framework-developer, code-review-deep, implementx, plan-mode, paper-workbench, jupyter-notebook

**问题**：这些skill的description或正文中仍使用 "Codex" 作为宿主名，应统一为 "Claude Code"。最严重的是 `jupyter-notebook` 仍引用 `$CODEX_HOME` 和 `~/.codex/` 路径（会直接导致运行失败）。

**修复**：全局替换 `Codex` → `Claude Code`，`$CODEX_HOME` → `$CLAUDE_CODE_HOME`，`~/.codex/` → `~/.claude/`。

### Critical-2: 引用已归档skill（4个skill）

**涉及**：gitx, latex-compile-acceleration, sentry, update, skill-framework-developer

**问题**：活跃skill仍引用 `systematic-debugging`、`skill-creator`、`skill-installer`，但这些已在 `.archive-cold/` 中。子代理按引用读取时会路径断裂。

**修复**：将引用替换为当前替代方案（systematic-debugging → 通用debugging能力，skill-creator/skill-installer → skill-framework-developer 内联）。

### Critical-3: Token-optimization 和 Adversarial-loop 内容过薄

**涉及**：token-optimization (70行), adversarial-loop (70行)

**问题**：SKILL.md 内容过薄，instructions 部分不足以指导执行。adversarial-loop 被3个skill引用但自身质量不达标。

**修复**：充实instructions，补充执行步骤和边界条件；或考虑合并到相关skill中。

### Critical-4: my-lifecycle-common 和 primary-runtime 缺少规范SKILL.md

**问题**：两个核心目录不在 Manifest/Runtime/Metadata 中，属于 ghost 目录。my-lifecycle-common 仅含契约文档，primary-runtime 含嵌套spreadsheets。

**修复**：添加 METADATA.md 或在 Manifest 中注册为 `kind=shared-contract`。

### Critical-5: Jupyter-notebook 使用 `$CODEX_HOME` 路径

**问题**：环境变量引用完全过时，在 Claude Code 环境下不可用。

**修复**：更新为 Claude Code 对应的环境变量路径。

### Critical-6: SKILL_ROUTING_LAYERS.md 引用18个已归档skill

**问题**：路由层级文档引用大量cold skill但未标注 [archived]，读者会误判路由状态。

**修复**：在LAYERS.md中为所有已归档skill添加 `[archived]` 标签。

---

## 三、中等问题清单（Top 10）

| # | 问题 | 涉及skill | 修复effort |
|---|------|-----------|-----------|
| 1 | frontmatter schema 4种模式未统一 | 全局 | L |
| 2 | 行为控制字段缺失（user-invocable 12个, disable-model-invocation 18个） | 全局 | M |
| 3 | routing_gate null值 | mcp-server-management, token-optimization | S |
| 4 | lifecycle注解覆盖仅25% | 全局 | M |
| 5 | model字段指向非4.x模型 | diagramming | S |
| 6 | Hook事件声称过时（30+类型） | agent-swarm-orchestration | S |
| 7 | SKILL_TIERS.json与实际不一致 | 全局 | M |
| 8 | CONTRIBUTING.md与MAINTENANCE_GUIDE验证工具不一致（Python vs Rust） | 贡献流程 | S |
| 9 | Host特有概念混入通用正文 | 多个skill | M |
| 10 | Spreadsheets嵌套路径不规范 | primary-runtime/spreadsheets | S |

---

## 四、外部调研发现（可采纳的最佳实践）

### 1. Claude Code Custom Slash Commands 规范（2026）

- **来源**：Anthropic 官方文档 + 社区实践
- **发现**：Claude Code 原生支持 `.claude/commands/` 下的 markdown 文件作为自定义命令，frontmatter schema 已标准化
- **可采纳**：将SKILL.md frontmatter对齐官方schema，减少自定义字段

### 2. Multi-Agent Orchestration 模式（2026）

- **来源**：Anthropic Agent SDK, OpenAI Agents SDK
- **发现**：2026年agent编排趋向 "声明式orchestration"（用户描述意图，runtime自动选择编排模式）而非 "命令式workflow"
- **可采纳**：agent-swarm-orchestration 可升级为声明式模式

### 3. Skill Template 标准化趋势

- **来源**：GitHub 社区 + Cursor/Windsurf 对比分析
- **发现**：头部项目采用 minimal frontmatter + rich instructions 模式，frontmatter仅含路由必需字段
- **可采纳**：简化frontmatter到L0必选集，丰富instructions

### 4. Prompt Template 版本管理

- **来源**：LangSmith, PromptLayer 等工具
- **发现**：成熟的prompt管理已支持版本号、A/B测试、性能指标追踪
- **可采纳**：为高影响力skill添加版本号和变更日志

---

## 五、优化Plan（P0/P1/P2 三优先级）

### P0 — 紧急修复（第1周）

| # | 方案 | 目标 | 涉及skill | Effort | 风险 |
|---|------|------|-----------|--------|------|
| P0-1 | Codex → Claude Code 全局替换 | 消除过时宿主引用 | 9个skill | S | 低：纯文本替换 |
| P0-2 | 修复已归档skill引用 | 消除路径断裂 | 5个skill | S | 低：替换为目标文件 |
| P0-3 | 修复 $CODEX_HOME 路径 | 修复jupyter-notebook运行时错误 | jupyter-notebook | S | 低 |
| P0-4 | routing_gate null → "none" | 修复路由null值 | mcp-server-management, token-optimization | S | 低 |
| P0-5 | model: haiku → 正式模型ID | 修复模型路由 | diagramming | S | 低 |
| P0-6 | LAYERS.md 添加 [archived] 标签 | 消除路由状态误判 | SKILL_ROUTING_LAYERS.md | S | 低 |

**预计产出**：~15个文件修改，~200行变更，1-2天完成。

### P1 — 重要优化（第2-3周）

| # | 方案 | 目标 | 涉及skill | Effort | 风险 |
|---|------|------|-----------|--------|------|
| P1-1 | Frontmatter schema分层定义 | 统一L0/L1/L2字段集 | 全局SKILL.md + MAINTENANCE_GUIDE | L | 中：需全量回归测试 |
| P1-2 | 行为控制字段补齐 | user-invocable + disable-model-invocation 显式声明 | 24个skill | M | 低 |
| P1-3 | 补齐lifecycle注解 | framework_roles/phase/contracts 覆盖率→80% | 27个skill | M | 低 |
| P1-4 | 充实薄弱skill | token-optimization + adversarial-loop | 2个skill | M | 中：需验证功能 |
| P1-5 | 验证工具路径统一 | CONTRIBUTING.md Python→Rust | 贡献流程文档 | S | 低 |
| P1-6 | SKILL_TIERS.json对齐 | tier计数与实际skill一致 | SKILL_TIERS.json | S | 低 |
| P1-7 | Ghost目录规范化 | my-lifecycle-common + primary-runtime 注册 | 2个目录 | S | 低 |
| P1-8 | Host特有概念分离 | 通用编排 vs 宿主特有指令 | 多个skill | M | 中：需跨宿主验证 |

**预计产出**：~40个文件修改，~800行变更，5-7天完成。

### P2 — 增强升级（第4周+）

| # | 方案 | 目标 | 涉及skill | Effort | 风险 |
|---|------|------|-----------|--------|------|
| P2-1 | Manifest schema精简 | 删除4个全null字段 | SKILL_MANIFEST.json | S | 低 |
| P2-2 | 嵌套路径规范化 | spreadsheets → skills/spreadsheets/ | primary-runtime | S | 中：需更新路由 |
| P2-3 | 声明式agent编排升级 | agent-swarm-orchestration 升级 | agent-swarm-orchestration | L | 中 |
| P2-4 | 高影响力skill版本化 | 添加版本号+changelog | top 10 skill | M | 低 |
| P2-5 | Cold skill metadata标记 | Manifest schema添加kind=cold约束 | SKILL_MANIFEST.json | S | 低 |
| P2-6 | session_start值统一 | "n/a" → n/a（裸字符串） | 2个skill | S | 低 |
| P2-7 | Risk/Source字段补齐 | 7个缺失skill补齐 | 7个skill | S | 低 |

**预计产出**：~20个文件修改，~400行变更，3-5天完成。

---

## 六、4周实施路线图

```
Week 1 (P0 紧急修复)
├── Day 1-2: P0-1 ~ P0-6 全部完成
├── Day 3:   回归测试 — 路由表、Manifest、Runtime 全量校验
└── Day 4:   Review + 合并

Week 2 (P1 核心优化)
├── Day 1-3: P1-1 frontmatter schema分层 + P1-2 字段补齐
├── Day 4:   P1-4 充实薄弱skill
└── Day 5:   P1-3 ~ P1-8 其余项

Week 3 (P1 收尾 + P2 启动)
├── Day 1-2: P1-8 Host概念分离（跨宿主验证）
├── Day 3:   P1 全量Review
├── Day 4-5: P2-1 ~ P2-3 启动
└── Week 3 checkpoint: 80%+ 问题已修复

Week 4 (P2 增强 + 收尾)
├── Day 1-3: P2-3 ~ P2-7 完成
├── Day 4:   全量集成测试
└── Day 5:   Final Review + 交付
```

---

## 七、Review 评分

| 维度 | 评分 | 评估 |
|------|------|------|
| **可行性** | 8/10 | P0可100%执行，P1需schema设计先行，P2需协调跨宿主测试 |
| **影响力** | 8.5/10 | P0消除运行时错误，P1统一规范提升维护效率50%+，P2面向未来 |
| **一致性** | 9/10 | 与CONTRIBUTING.md和MAINTENANCE_GUIDE完全兼容 |

### Review补充建议

1. **可行性**：P1-1 frontmatter schema分层建议先产出设计文档再批量修改
2. **影响力**：遗漏项 — 建议增加 "skill健康度仪表盘"（自动检测缺失字段、过时引用）
3. **一致性**：P1-5 验证工具统一建议以Rust为准（与router-rs-framework对齐）

---

## 八、附录：完整发现索引

### 按skill维度的问题分布

| Skill | Critical | Major | Minor | 总计 |
|-------|----------|-------|-------|------|
| jupyter-notebook | 1 | 1 | 1 | 3 |
| token-optimization | 1 | 1 | 1 | 3 |
| adversarial-loop | 1 | 0 | 2 | 3 |
| my-lifecycle-common | 1 | 0 | 1 | 2 |
| primary-runtime | 1 | 0 | 2 | 3 |
| agent-swarm-orchestration | 0 | 2 | 1 | 3 |
| gitx | 0 | 1 | 1 | 2 |
| code-review-deep | 0 | 1 | 1 | 2 |
| mcp-server-management | 0 | 1 | 1 | 2 |
| diagramming | 0 | 1 | 0 | 1 |
| 全局(跨skill) | 0 | 4 | 4 | 8 |

### 外部调研可采纳项优先级

| # | 实践 | 可行性 | 影响 | 优先建议 |
|---|------|--------|------|----------|
| 1 | Frontmatter schema对齐官方规范 | 高 | 高 | P1-1 包含 |
| 2 | Minimal frontmatter + Rich instructions | 高 | 中 | P1-1 包含 |
| 3 | 声明式agent编排 | 中 | 高 | P2-3 |
| 4 | Skill版本化与changelog | 高 | 低 | P2-4 |
| 5 | Prompt模板A/B测试 | 低 | 中 | 暂不采纳 |

---

*Generated by multi-agent audit workflow (11 agents, 396 tool calls, ~21min)*
