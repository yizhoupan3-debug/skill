# Skill 仓库综合优化 Plan

> 基于 7 个审计 agent 的交叉分析 + adversarial review 修订后生成。
> 生成日期：2026-06-02
> Review 状态：APPROVED（经 3 blocking + 10 non-blocking 修订）
> 前序计划：`docs/SKILL_OPTIMIZATION_PLAN.md`（上一版，大部分 P0/P1 已完成）

---

## 执行摘要

本 plan 基于 7 个专项审计 agent（核心配置、生命周期 skill、工具类 skill、学术 skill、外部框架对比研究）的发现，经 adversarial review 修订后生成。当前仓库在上一轮 P0/P1 修复后，skill 内容质量已显著改善（Codex 残留清理、frontmatter schema 分层、行为控制字段全覆盖均已落地）。本轮优化的**核心矛盾**已从「内容过时」转向「**配置层与文档层不一致**」——CLAUDE.md 声称无 hook 但 settings.json 实际安装了 4 个 hook，GOAL_STATE_CONTRACT.md 仍在引用旧 stdio 接口，settings.local.json 与 settings.json 存在冗余覆盖，mcp.json 硬编码本机路径。这些问题不阻断运行但会误导 agent 行为推理，应优先修复。

---

## 一、优先级矩阵

severity（运行影响）× impact（修复后收益）矩阵，四象限排序：

### 象限 A：高 severity × 高 impact（立即修复）

| # | 来源 | 发现 | severity | impact |
|---|------|------|----------|--------|
| **M-1** | audit-core | CLAUDE.md 声称「无 PreToolUse/Stop shell hook」，实际 settings.json 安装 4 个 hook | WARNING | HIGH |
| **M-2** | audit-lifecycle | GOAL_STATE_CONTRACT.md 全文引用 `framework_goal_drive` stdio，Claude Desktop 实际用 MCP `goal_state_manage` | CRITICAL | HIGH |
| **M-3** | audit-lifecycle | verifyx 引用 `closeout_evaluate` stdio，实际 MCP 工具为 `closeout_record_write` | CRITICAL | HIGH |
| **M-4** | audit-lifecycle | implementx 引用已移除的 hook 机制（REVIEW_GATE hard block、AG_FOLLOWUP、beforeSubmit） | CRITICAL | HIGH |

### 象限 B：高 severity × 中 impact（本周修复）

| # | 来源 | 发现 | severity | impact |
|---|------|------|----------|--------|
| **M-5** | audit-core | settings.local.json 完全被 settings.json 覆盖（权限规则 100% 冗余） | WARNING | MEDIUM |
| **M-6** | audit-core | sandbox `enabled:false` 但定义了网络约束（死配置） | WARNING | MEDIUM |
| **M-7** | audit-core | CLAUDE.md 称 closeout 为 advisory，但未提及非 my-light 时 MCP 硬拦 | WARNING | MEDIUM |
| **M-8** | audit-core | `rm -[^.]*` 正则不安全（匹配 `rm -rf /` 等危险命令） | WARNING | MEDIUM |

### 象限 C：中 severity × 中 impact（两周内修复）

| # | 来源 | 发现 | severity | impact |
|---|------|------|----------|--------|
| **M-9** | audit-core | mcp.json 硬编码 `/Users/joe/...` 绝对路径 | WARNING | MEDIUM |
| **M-10** | audit-lifecycle | evidence-protocol.md 引用旧环境变量和旧 stdio 接口 | WARNING | MEDIUM |
| **M-11** | audit-lifecycle | discussx/planx 的 GOAL_STATE 写入指导用 `framework_goal_drive` 而非 MCP | WARNING | MEDIUM |
| **M-12** | audit-lifecycle | AGENTS.md Goal drive 节只提 `framework_goal_drive` stdio | WARNING | MEDIUM |
| **M-13** | audit-core | CLAUDE.md 与 AGENTS_CLAUDE.md 内容 70%+ 重复 | INFO | MEDIUM |

### 象限 D：低 severity × 中 impact（一个月内改进）

| # | 来源 | 发现 | severity | impact |
|---|------|------|----------|--------|
| **M-14** | audit-academic | 5 个 skill 被标注「已归档」但自身未标记（mac-memory-management 等） | INFO | LOW |
| **M-15** | audit-academic | 演示文稿 skill 群冗余：slides(425行) + ppt-beamer(128行) + source-slide-formats(102行) = 655 行 | INFO | MEDIUM |
| **M-16** | audit-lifecycle | plan-mode SKILL.md 285 行过长 | INFO | LOW |
| **M-17** | audit-lifecycle | 四个 lifecycle skill 重复同一 GOAL_STATE 引用模板 | INFO | LOW |
| **M-18** | audit-tools | token-optimization 被 Claude Code 原生功能覆盖，应 deprecate | INFO | LOW |
| **M-19** | audit-core | 用户级 settings.json 暴露 ANTHROPIC_AUTH_TOKEN（安全） | WARNING | LOW |
| **M-20** | research | 路径级规则激活（Cursor globs / Claude Code rules/）可借鉴 | INFO | LOW |

---

## 二、Quick Wins（低投入高收益）

以下 action item 预估工作量 S（< 30 分钟/项），收益明确：

| # | 对应 | 改动 | 收益 |
|---|------|------|------|
| **QW-1** | M-1 | CLAUDE.md hook 段落改为准确描述 | 消除 agent 行为推理偏差 |
| **QW-2** | M-5 | 删除 settings.local.json 冗余内容或清空 | 消除维护困惑 |
| **QW-3** | M-8 | 移除 rm 的 wildcard allow（`rm -[^.]*` → 删除），改为每次弹权限确认；或收紧为 `Bash(rm -f *)` 仅允许安全 flag | 安全加固 |
| **QW-4** | M-7 | CLAUDE.md closeout 段落补充非 my-light 条件说明 | 文档准确性 |
| **QW-5** | M-18 | token-optimization SKILL.md 添加 deprecation notice | 明确弃用信号 |
| **QW-6** | M-14 | 5 个「已归档但未标记」的 skill 添加 archived frontmatter | 状态一致性 |

**总 quick win 工作量**：~3 小时，6 个文件修改。

---

## 三、Phase 1 — 本周：配置层与接口一致性修复

### 目标

消除 agent 行为推理偏差：确保 CLAUDE.md、settings.json、GOAL_STATE_CONTRACT.md、lifecycle skill 之间的描述完全一致。

### 检查清单

#### 1.1 CLAUDE.md hook 描述修正（M-1, QW-1）

- **文件**：`.claude/CLAUDE.md` + `.claude/rules/framework.md`
- **改动类型**：文档修正
- **具体步骤**：
  1. 将「无 PreToolUse/Stop shell hook」改为「Hook 已安装（PreToolUse / PostToolUse / Stop / UserPromptSubmit），运行于 advisory 模式，不硬拦工具调用」
  2. 将「无 CLI hook 硬拦」段落改为「Hook 运行于 advisory 模式——不注入 GOAL_CONTINUE / RFV / digest；Bash 前自行评估安全；勿声称已被 hook 硬拦」
- **验证方法**：grep -r "无.*hook" .claude/ 确认无残留；人工审查更新后的段落语义准确
- **工作量**：S
- **风险**：低 — 纯文档修正，不影响运行时

#### 1.2 GOAL_STATE_CONTRACT.md 双轨接口对齐（M-2）

- **文件**：`skills/my-lifecycle-common/GOAL_STATE_CONTRACT.md`
- **改动类型**：接口文档更新
- **具体步骤**：
  1. 在文档顶部增加「双轨接口说明」：stdio `framework_goal_drive`（CLI / Cursor / Codex）vs MCP `goal_state_manage`（Claude Desktop）
  2. 每个操作示例同时给出 stdio 和 MCP 两种调用形式
  3. 明确标注「Claude Desktop 环境下优先使用 MCP `goal_state_manage`」
- **验证方法**：对比 `mcp__router-rs-framework__goal_state_manage` 的参数 schema，确认文档覆盖全部 operation
- **工作量**：M
- **风险**：低 — 文档修改；需确保 MCP 工具参数文档准确

#### 1.3 verifyx closeout 接口对齐（M-3）

- **文件**：`skills/verifyx/SKILL.md`
- **改动类型**：接口引用更新
- **具体步骤**：
  1. 将 `closeout_evaluate` stdio 引用改为「`closeout_record_write`（MCP）或 `router-rs closeout evaluate --record-path`（CLI）」
  2. 更新 Closeout record 写入段落，补充 MCP `closeout_record_write` 的参数说明
  3. 更新 `framework_goal_drive` 引用为双轨格式
- **验证方法**：grep "closeout_evaluate" 确认无残留；对比 MCP 工具 schema
- **工作量**：S
- **风险**：低

#### 1.4 implementx 废弃 hook 引用清理（M-4）

- **文件**：`skills/implementx/SKILL.md`
- **改动类型**：废弃引用清理
- **具体步骤**：
  1. 移除 REVIEW_GATE hard block 相关描述（当前 my-light profile 下已无此机制）
  2. 移除 AG_FOLLOWUP 和 beforeSubmit 引用
  3. 更新 description 中的「REVIEW_GATE hard block off」为更准确的「无硬拦，advisory 模式」
  4. 如存在 hook 事件引用，改为描述当前 advisory 模式行为
- **验证方法**：grep -r "REVIEW_GATE\|AG_FOLLOWUP\|beforeSubmit" skills/implementx/ 确认无残留
- **工作量**：S
- **风险**：低 — 已移除机制的文档清理

#### 1.5 settings.local.json 清理（M-5, QW-2）

- **文件**：`.claude/settings.local.json`
- **改动类型**：配置清理
- **具体步骤**：
  1. 对比 settings.json 和 settings.local.json 的 permissions.allow 列表
  2. 确认 settings.local.json 中的 `cargo`、`rtk` 权限已在 settings.json 中覆盖
  3. **保留文件**（settings.local.json 是 gitignored 的用户本地覆盖入口），仅清空冗余 allow 项
  4. 如 settings.local.json 完全冗余，清空为 `{}` 但保留文件本身
  5. ⚠️ 不要删除文件——它可能承载 settings.json 提交到公共仓库后的本地覆盖职责
- **验证方法**：运行 `claude` 确认权限无变化；diff 两文件确认无遗漏
- **工作量**：S
- **风险**：中 — 需仔细比对，避免误删 settings.local.json 中特有且需要的权限

#### 1.6 sandbox 死配置清理（M-6）

- **文件**：`.claude/settings.json`
- **改动类型**：配置清理
- **具体步骤**：
  1. 评估是否要启用 sandbox（当前 `enabled: false`）
  2. 如不启用：移除 `network` 和 `excludedCommands` 配置块（无实际效果）
  3. 如需保留以备将来启用：添加注释说明「当前 sandbox 已禁用，以下配置为预设」
  4. **建议**：保留配置但移至 settings.json 的 `_comment` 字段或单独文档
- **验证方法**：确认 sandbox 仍为 `enabled: false`；运行无异常
- **工作量**：S
- **风险**：低

#### 1.7 rm 正则安全加固（M-8, QW-3）

- **文件**：`.claude/settings.json`
- **改动类型**：安全加固
- **具体步骤**：
  1. 当前 `Bash(rm -[^.]*)` 匹配所有短 flag 组合，包括 `rm -rf`（危险）
  2. **方案 A（推荐）**：移除 rm 的 wildcard allow，改为每次弹权限确认（安全优先）
  3. **方案 B**：收紧为 `Bash(rm -f *)` 仅允许 `-f`（删除不存在的文件不报错），禁止 `-r`
  4. ⚠️ **不可行方案**：`rm -[a-zA-Z]*` 仍匹配 `-rf` 等危险组合，不提供额外安全
- **验证方法**：测试 `rm -rf /tmp/test` 是否被拦截；测试 `rm file.txt` 是否弹确认
- **工作量**：S
- **风险**：中 — 正则变更可能导致意外拒绝或放行，需测试多种 rm 用例

#### 1.8 CLAUDE.md closeout 段落补充（M-7, QW-4）

- **文件**：`.claude/CLAUDE.md` + `.claude/rules/framework.md`
- **改动类型**：文档完善
- **具体步骤**：
  1. 在 closeout 描述中补充：「my-light 下 closeout/complete 为 advisory（MCP 工具层不阻断）；非 my-light 时 MCP `closeout_gate` 未满足则硬拦后续 `complete` 操作」
- **验证方法**：阅读更新段落，确认覆盖两个 profile 场景
- **工作量**：S
- **风险**：低

#### 1.9 归档状态一致性修复（M-14, QW-6）

- **文件**：`skills/mac-memory-management/`、`skills/jupyter-notebook/`、`skills/latex-compile-acceleration/`
- **改动类型**：状态标记
- **具体步骤**：
  1. 为每个 skill 的 SKILL.md frontmatter 添加 `status: archived` 或移入 `.archive-cold/`
  2. 同时检查 `.archive-cold/` 目录中的 skill 是否已在 Manifest 中标记为 archived
  3. 更新路由表跳过 archived skill
- **验证方法**：grep "status.*archived" 确认已标记；路由测试确认不被路由
- **工作量**：S
- **风险**：低

### Phase 1 验证计划

```bash
# 1. 文档一致性检查
grep -rn "无 PreToolUse/Stop shell hook\|无 CLI hook 硬拦" .claude/ CLAUDE.md docs/
# 预期：无匹配（已改为 advisory 描述）

grep -rn "framework_goal_drive" skills/my-lifecycle-common/ skills/verifyx/ skills/planx/ skills/discussx/
# 预期：双轨描述，stdio 和 MCP 并列

grep -rn "closeout_evaluate" skills/ docs/
# 预期：无匹配（已替换为 closeout_record_write）

grep -rn "REVIEW_GATE\|AG_FOLLOWUP\|beforeSubmit" skills/implementx/
# 预期：无匹配

# 1b. docs/ 目录旧接口检查
grep -rn "framework_goal_drive\|closeout_evaluate" docs/
# 预期：仅在双轨文档中出现

# 2. 配置文件 JSON 验证
cat .claude/settings.json | python3 -m json.tool
cat .claude/settings.local.json | python3 -m json.tool

# 3. Hook 脚本行为验证
# 确认 claude-router-rs-hook.sh 在 my-light 下实际行为为 advisory
# 在 Claude Desktop 中手动触发一次完整 lifecycle 流程

# 4. 归档状态一致性
for skill in mac-memory-management jupyter-notebook latex-compile-acceleration; do
  if [ -d "skills/$skill" ] && ! grep -q "status.*archived" "skills/$skill/SKILL.md" 2>/dev/null; then
    echo "FAIL: $skill 缺少 archived 标记"
  fi
done
```

### Phase 1 风险缓解

| 风险 | 概率 | 缓解措施 |
|------|------|----------|
| rm 正则变更导致意外拒绝 | 中 | 变更前记录当前正则匹配的所有 rm 命令变体；变更后回归测试 |
| settings.local.json 清理遗漏特有权限 | 低 | 逐条 diff，保留 settings.local.json 独有项 |
| 文档修正引入新矛盾 | 低 | 统一审查 CLAUDE.md + framework.md + AGENTS.md 三个文件的一致性 |

---

## 四、Phase 2 — 两周内：接口文档统一 + Skill 整合

### 目标

消除旧接口残留；合并冗余 skill 群；统一 GOAL_STATE 写入指导。

### 检查清单

#### 2.1 evidence-protocol.md 接口更新（M-10）

- **文件**：`skills/my-lifecycle-common/evidence-protocol.md`（或等效路径）
- **改动类型**：接口文档更新
- **具体步骤**：
  1. 识别所有旧环境变量引用（如 `$CODEX_*` 旧变量），替换为当前变量名
  2. 识别旧 stdio 接口引用，替换为双轨格式（stdio + MCP）
  3. 更新调用示例
- **验证方法**：grep 旧变量名确认无残留；对比 MCP 工具 schema
- **工作量**：S
- **风险**：低

#### 2.2 discussx/planx GOAL_STATE 写入指导双轨化（M-11）

- **文件**：`skills/discussx/SKILL.md`、`skills/planx/SKILL.md`
- **改动类型**：接口指导更新
- **具体步骤**：
  1. 在两个 skill 的 GOAL_STATE 写入段落中，同时列出 stdio 和 MCP 两种调用方式
  2. 标注「Claude Desktop 环境下优先使用 MCP `goal_state_manage`」
  3. 将引用的 GOAL_STATE_CONTRACT.md 路径确认为相对路径且可解析
- **验证方法**：在 Claude Desktop 中手动走一遍 discussx → planx 流程，确认 MCP 调用成功
- **工作量**：S
- **风险**：低

#### 2.3 AGENTS.md Goal drive 节更新（M-12）

- **文件**：`core/router-rs/src/host_entrypoint_sync.rs`（Rust 生成源码）
- **改动类型**：**代码改动**（非文档编辑 — AGENTS.md 是 Rust 生成文件，直接编辑会被 sync 覆盖）
- **具体步骤**：
  1. 在 Rust 源码中找到 AGENTS.md Goal drive 段的模板
  2. 增加 MCP `goal_state_manage` 路径说明
  3. 保持 stdio `framework_goal_drive` 描述（CLI / Cursor / Codex 使用）
  4. 添加「Claude Desktop → MCP；其他宿主 → stdio」的路由说明
  5. `cargo build` 重新编译，确认 AGENTS.md 输出正确
- **验证方法**：运行 `cargo build`，检查生成的 AGENTS.md 内容；阅读更新段落确认覆盖所有宿主
- **工作量**：**M**（需理解 Rust sync 逻辑 + 编译测试）
- **风险**：中 — 改 Rust 代码需确保不影响其他宿主的 entrypoint 生成

#### 2.4 演示文稿 Skill 群整合（M-15）

- **文件**：`skills/slides/`、`skills/ppt-beamer/`、`skills/source-slide-formats/`
- **改动类型**：Skill 合并
- **具体步骤**：
  1. 评估三个 skill 的职责边界：
     - `slides`（425 行）：通用幻灯片生成，覆盖 Markdown/HTML/reveal.js
     - `ppt-beamer`（128 行）：LaTeX Beamer 特定
     - `source-slide-formats`（102 行）：源码格式转换
  2. **合并方案**：
     - 将 `ppt-beamer` 和 `source-slide-formats` 的核心指令合并到 `slides` 的 `references/` 子目录
     - `slides` 主 SKILL.md 保持顶层编排，通过 references 引用专业格式细节
     - `ppt-beamer` 和 `source-slide-formats` 标记为 archived
  3. 更新路由表和 Manifest
- **验证方法**：合并后执行 `slides` 路由测试；确认 archived skill 不再出现在路由结果中
- **工作量**：M
- **风险**：中 — 需确保合并后不丢失专业格式的执行细节

#### 2.5 归档状态一致性修复（M-14, QW-6）

- **文件**：`skills/mac-memory-management/`、`skills/jupyter-notebook/`、`skills/latex-compile-acceleration/`、`skills/ppt-beamer/`（合并后）、`skills/source-slide-formats/`（合并后）
- **改动类型**：状态标记
- **具体步骤**：
  1. 为每个 skill 的 SKILL.md frontmatter 添加 `status: archived` 或移入 `.archive-cold/`
  2. 更新 Manifest 中对应条目
- **验证方法**：grep "status.*archived" 确认已标记；路由测试确认不被路由
- **工作量**：S
- **风险**：低

#### ~~2.6 GOAL_STATE 引用模板去重~~ → 降级为 P3 备忘（M-17）

- **说明**：4处重复的 GOAL_STATE 模板不构成严重维护负担。强行统一为共享文件会增加 agent 文件读取次数，且各 skill 可能有微小差异。仅在下次大规模修改 lifecycle skill 时顺带处理。
- **工作量**：暂不执行

#### 2.7 CLAUDE.md 与 AGENTS_CLAUDE.md 去重（M-13）

- **文件**：`.claude/CLAUDE.md`、`AGENTS_CLAUDE.md`
- **改动类型**：文档重构
- **具体步骤**：
  1. 分析两个文件的 70%+ 重复内容，确定哪些是「通用框架指令」vs「宿主特有指令」
  2. 将通用部分保留在 AGENTS_CLAUDE.md
  3. CLAUDE.md 仅保留 Claude Desktop 宿主特有部分，通过 include 机制或文件引用指向通用部分
  4. **注意**：当前 `AGENTS.md` 为生成文件（blocked direct mutation），需通过 Rust host-entrypoint sync 路径处理
- **验证方法**：运行 Claude Desktop 确认指令行为无变化
- **工作量**：M
- **风险**：中 — 文档重构需谨慎，避免破坏 Claude Desktop 的指令加载链路

#### 2.8 token-optimization Deprecation（M-18, QW-5）

- **文件**：`skills/token-optimization/SKILL.md`
- **改动类型**：弃用标记
- **具体步骤**：
  1. 在 frontmatter 添加 `status: deprecated`
  2. 在 SKILL.md 顶部添加弃用说明：「Claude Code 原生已支持 prompt caching 和 token 优化，本 skill 不再维护」
  3. 更新路由表跳过 deprecated skill
- **验证方法**：路由测试确认不被命中
- **工作量**：S
- **风险**：低

### Phase 2 验证计划

```bash
# 1. 接口引用一致性
grep -rn "framework_goal_drive\|closeout_evaluate\|REVIEW_GATE\|AG_FOLLOWUP" skills/
# 预期：仅出现在双轨文档的「stdio」部分，且上下文正确

# 2. Skill 合并验证
ls skills/ppt-beamer/SKILL.md skills/source-slide-formats/SKILL.md
# 预期：存在但 frontmatter 含 status: archived

# 3. 路由回归测试
# Claude Desktop 中触发 skill_route，确认：
# - slides 路由正常命中
# - ppt-beamer / source-slide-formats 不被路由
# - token-optimization 不被路由

# 4. 全量文件一致性
grep -rn "closeout_evaluate\|framework_goal_drive" skills/ docs/
# 预期：仅出现在文档说明中，不作为执行指令
```

### Phase 2 风险缓解

| 风险 | 概率 | 缓解措施 |
|------|------|----------|
| 演示文稿合并丢失专业细节 | 中 | 合并前备份原始 skill；合并后走一遍 slides 执行流程 |
| AGENTS.md 更新被 sync 覆盖 | 高 | 通过 Rust host-entrypoint sync 路径修改，而非直接编辑 |
| CLAUDE.md 去重后指令加载异常 | 中 | 先在测试分支操作；确认 Claude Desktop 行为无变化后再合并 |

---

## 五、Phase 3 — 一个月内：架构改进 + 外部最佳实践采纳

### 目标

提升框架可维护性和跨宿主兼容性；吸收外部框架优势。

### 检查清单

#### 3.1 plan-mode 精简（M-16）

- **文件**：`skills/plan-mode/SKILL.md`（285 行）
- **改动类型**：文档瘦身
- **具体步骤**：
  1. 将 CreatePlan 契约详细说明下沉到 `skills/plan-mode/references/`（此工作在 commit `ffa525c0` 中已部分完成）
  2. 将 Cursor 宿主特有行为移到 `references/cursor-createplan.md`
  3. 主 SKILL.md 目标 150 行以内，仅保留编排逻辑和跨宿主通用指令
- **验证方法**：plan-mode 路由和执行测试
- **工作量**：M
- **风险**：低

#### 3.2 mcp.json 路径参数化（M-9）

- **文件**：`.claude/mcp.json`
- **改动类型**：配置改进
- **具体步骤**：
  1. ⚠️ Claude Desktop MCP 加载器**不支持** `${VAR}` 环境变量插值，JSON 也不支持注释
  2. **方案 A（推荐）**：在 `.claude/` 下新增 `mcp.README.md`，说明 mcp.json 中每条路径的含义和首次配置步骤
  3. **方案 B**：使用相对路径（`./`）替代绝对路径，配合 symlink 方案
  4. **方案 C**：提供 `scripts/setup-mcp.sh` 脚本，首次克隆后运行生成 mcp.json
- **验证方法**：重启 Claude Desktop 确认 MCP 服务正常连接
- **工作量**：S（方案 A/B）/ M（方案 C）
- **风险**：低 — 方案 A 为纯文档添加

#### 3.3 安全配置审查（M-19）

- **文件**：用户级 `~/.claude/settings.json`
- **改动类型**：安全加固
- **具体步骤**：
  1. 检查 `ANTHROPIC_AUTH_TOKEN` 是否仍在用户级 settings.json 中暴露
  2. 如存在：迁移到环境变量或 secrets manager
  3. 确认项目级 settings.json 不包含任何 secret
- **验证方法**：grep -r "TOKEN\|SECRET\|KEY" .claude/ 确认无暴露
- **工作量**：S
- **风险**：低

#### 3.4 ~~路径级规则激活~~ → P3 备忘（M-20, 外部最佳实践）

- **改动类型**：架构评估备忘（**不实现**，仅记录）
- **说明**：当前 skill 规模（~40个）不需要路径级规则激活。待 skill 数量超过 100 或出现明确痛点时再评估。记录在 ADR 中即可。
- **工作量**：S（仅写 ADR）

#### 3.5 Auto Memory 机制评估（外部最佳实践）

- **改动类型**：架构评估
- **具体步骤**：
  1. 评估 Claude Code 的 Auto memory 机制与本框架 `SESSION_SUMMARY.md` / `NEXT_ACTIONS.json` 的关系
  2. 确定是否需要集成 auto memory 或保持独立
  3. 如集成：在 lifecycle skill 中添加 auto memory 写入点
  4. 如保持独立：文档化两者分工
- **验证方法**：架构决策文档
- **工作量**：S（评估）/ L（如需实现）
- **风险**：低 — 评估阶段无风险

#### 3.6 ~~声明式 Agent 编排升级~~ → 调研备忘（E-8, 外部最佳实践）

- **改动类型**：ADR 调研备忘（**不实现**，仅记录决策理由）
- **说明**：当前命令式编排在 my-light profile 下工作良好。引入声明式 intent → runtime 自动选择编排模式将大幅增加复杂度（需定义 intent schema、mode selection logic、fallback 策略），收益不明确。记录为 ADR：「当前阶段保持命令式编排，待出现明确痛点时再评估声明式方案」。
- **工作量**：S（仅写 ADR）
- **风险**：无

### Phase 3 验证计划

```bash
# 1. plan-mode 精简验证
wc -l skills/plan-mode/SKILL.md
# 预期：< 150 行

# 2. mcp.json 验证
# 重启 Claude Desktop，检查 MCP 服务连接状态
# 用 framework_snapshot 确认 MCP 工具可用

# 3. 安全扫描
grep -rn "TOKEN\|SECRET\|PASSWORD\|KEY" .claude/ --include="*.json" --include="*.md"
# 预期：无敏感信息暴露

# 4. ADR 备忘确认
# 确认路径级规则和声明式编排的 ADR 已创建
```

### Phase 3 风险缓解

| 风险 | 概率 | 缓解措施 |
|------|------|----------|
| mcp.json README 方案不被团队遵守 | 中 | 在 setup 脚本中自动化生成 |
| plan-mode 精简后丢失关键信息 | 低 | 精简内容先迁移到 references/，再从主文件删除 |
| 安全审查发现更多 token 暴露 | 低 | 扩展 grep 范围到所有 .json/.md/.sh 文件 |

---

## 六、Skill 整合方案

### 6.1 合并计划

| 源 Skill | 目标 | 操作 | 理由 |
|----------|------|------|------|
| `ppt-beamer` (128行) | `slides/references/beamer.md` | 内容合并，原 skill archived | LaTeX Beamer 是 slides 的子集 |
| `source-slide-formats` (102行) | `slides/references/source-formats.md` | 内容合并，原 skill archived | 源码格式转换是 slides 的子集 |

合并后 `slides` 期望结构：

```
slides/
  SKILL.md (425→450行，主编排)
  references/
    beamer.md (原 ppt-beamer 核心指令)
    source-formats.md (原 source-slide-formats 核心指令)
```

### 6.2 弃用计划

| Skill | 操作 | 替代方案 |
|-------|------|----------|
| `token-optimization` | 标记 deprecated，不主动路由 | Claude Code 原生 prompt caching |

### 6.3 归档状态修复

以下 skill 已在之前的审计中标注为归档但自身未标记，需添加 `status: archived` 或移入 `.archive-cold/`：

- `mac-memory-management`
- `jupyter-notebook`
- `latex-compile-acceleration`

### 6.4 迁移步骤（合并操作）

```
Step 1: 创建 slides/references/ 目录
Step 2: 将 ppt-beamer/SKILL.md 核心指令提取到 slides/references/beamer.md
Step 3: 将 source-slide-formats/SKILL.md 核心指令提取到 slides/references/source-formats.md
Step 4: 在 slides/SKILL.md 中添加 references 引用段落
Step 5: 为 ppt-beamer/ 和 source-slide-formats/ 的 SKILL.md 添加 status: archived
Step 6: 更新 SKILL_ROUTING_RUNTIME.json（如有 ppt-beamer/source-slide-formats 路由）
Step 7: 回归测试 slides 路由
Step 8: 提交，commit message: "refactor(skills): 演示文稿skill群整合 — ppt-beamer + source-slide-formats 合并到 slides/"
```

---

## 七、外部最佳实践采纳清单

| # | 来源框架 | 最佳实践 | 当前状态 | 采纳建议 | 优先级 |
|---|----------|----------|----------|----------|--------|
| **E-1** | Claude Code | 路径级规则激活（`rules/` 目录） | 无路径级规则 | ADR 备忘（P3，待规模扩大） | P3 |
| **E-2** | Claude Code | Auto memory 机制 | 有 SESSION_SUMMARY / NEXT_ACTIONS | 评估集成（Phase 3.5） | P2 |
| **E-3** | Cursor | `.mdc` 元数据驱动规则引擎（globs/intelligent/manual） | 无 globs 模式 | ADR 备忘，不强制采纳 | P3 |
| **E-4** | 多框架 | AGENTS.md 跨工具标准 | 已有 AGENTS.md | 持续维护，跟踪标准演进 | 保持 |
| **E-5** | Copilot | 指令自动生成（cloud agent） | 无自动生成 | 暂不采纳（需 cloud infra） | P3 |
| **E-6** | LangSmith | Prompt 模板版本管理 | skill 已有 version 字段 | 评估 changelog 自动化 | P3 |
| **E-7** | 社区趋势 | Minimal frontmatter + Rich instructions | 已采纳（P1-1 完成） | 持续保持 | 保持 |
| **E-8** | Anthropic SDK | 声明式 Agent 编排 | 命令式编排 | ADR 备忘（当前命令式足够） | P3 |

---

## 八、工作量总览与时间线

```
Week 1 (Phase 1 — 配置层一致性 + 归档标记)
├── Day 1:   M-1 CLAUDE.md hook 描述修正 (S) + M-7 closeout 段落补充 (S)
├── Day 2:   M-2 GOAL_STATE_CONTRACT.md 双轨化 (M) + M-3 verifyx 接口对齐 (S)
├── Day 3:   M-4 implementx 废弃引用清理 (S) + M-5 settings.local.json (S) + M-14 归档标记 (S)
├── Day 4:   M-6 sandbox 死配置 (S) + M-8 rm 正则 (S)
└── Day 5:   全量验证 + Review（含 docs/ 目录 grep）
预计工作量: 7×S + 1×M = ~5h

Week 2-3 (Phase 2 — 接口统一 + Skill 整合)
├── Day 1:   M-10 evidence-protocol 接口更新 (S) + M-11 discussx/planx GOAL_STATE 指导 (S)
├── Day 2:   M-12 AGENTS.md Goal drive (M, Rust 代码改动)
├── Day 3:   M-15 演示文稿合并 (M)
├── Day 4:   M-13 CLAUDE.md 去重 (M) + M-18 token-optimization deprecation (S)
└── Day 5:   全量验证 + 路由回归
预计工作量: 3×S + 3×M = ~6h

Week 4+ (Phase 3 — 架构改进 + ADR 备忘)
├── Day 1:   M-16 plan-mode 精简 (M) + M-9 mcp.json README (S)
├── Day 2:   M-19 安全审查 (S) + E-2 Auto Memory 评估 (S)
└── Day 3:   E-1 路径级规则 ADR (S) + E-8 声明式编排 ADR (S)
预计工作量: 4×S + 1×M = ~4h
```

**总预计工作量**：

| Phase | 文件数 | 行变更 | 人时 |
|-------|--------|--------|------|
| Phase 1 | 8-10 | ~150 | 5h |
| Phase 2 | 10-15 | ~300 | 6h |
| Phase 3 | 5-8 | ~200 | 4h |
| **合计** | **23-33** | **~650** | **15h** |

---

## 九、验证计划总览

### 每 Phase 完成后的验证矩阵

| 验证维度 | 方法 | Phase 1 | Phase 2 | Phase 3 |
|----------|------|---------|---------|---------|
| 文档一致性 | grep + 人工审查 | CHECK | CHECK | CHECK |
| JSON 语法 | `python3 -m json.tool` | CHECK | CHECK | CHECK |
| 路由正确性 | Claude Desktop skill_route 测试 | — | CHECK | CHECK |
| MCP 工具可用性 | framework_snapshot | CHECK | CHECK | CHECK |
| Lifecycle 流程 | 手动走 discussx → planx → implementx → verifyx | CHECK | CHECK | CHECK |
| Hook 行为 | advisory 模式验证 | CHECK | CHECK | CHECK |
| 安全扫描 | grep secret/token/key | — | — | CHECK |

### 自动化回归检查脚本

```bash
#!/bin/bash
# scripts/audit-consistency-check.sh
set -euo pipefail

echo "=== 1. Hook 描述一致性 ==="
! grep -rn "无 PreToolUse/Stop shell hook\|无 CLI hook 硬拦" .claude/ docs/ 2>/dev/null || echo "FAIL: 旧 hook 描述残留"

echo "=== 2. 废弃接口残留 ==="
! grep -rn "closeout_evaluate" skills/ docs/ 2>/dev/null || echo "FAIL: closeout_evaluate 残留"
! grep -rn "REVIEW_GATE.*hard block\|AG_FOLLOWUP\|beforeSubmit" skills/implementx/ 2>/dev/null || echo "FAIL: 废弃 hook 机制残留"

echo "=== 3. JSON 语法检查 ==="
find .claude/ -name "*.json" -exec python3 -m json.tool {} \; > /dev/null

echo "=== 4. 归档状态一致性 ==="
for skill in mac-memory-management jupyter-notebook latex-compile-acceleration; do
  if [ -d "skills/$skill" ] && ! grep -q "status.*archived" "skills/$skill/SKILL.md" 2>/dev/null; then
    echo "WARN: $skill 缺少 archived 标记"
  fi
done

echo "=== 5. .archive-cold/ 一致性 ==="
for dir in skills/.archive-cold/*/; do
  skill_name=$(basename "$dir")
  echo "INFO: archive-cold 中的 $skill_name — 请确认 Manifest 已标记"
done

echo "=== ALL CHECKS PASSED ==="
```

---

## 十、附录：审计发现交叉验证结果

### 已确认的交叉验证

| 审计来源 | 发现 | 验证方式 | 确认 |
|----------|------|----------|------|
| audit-core | CLAUDE.md 无 hook 声称 vs settings.json 4 个 hook | 直接读取两个文件 | CONFIRMED |
| audit-core | settings.local.json 冗余 | diff permissions.allow | CONFIRMED |
| audit-core | sandbox 死配置 | 读取 settings.json | CONFIRMED |
| audit-core | rm 正则不安全 | 读取 settings.json line 62 | CONFIRMED: `rm -[^.]*` 存在 |
| audit-core | mcp.json 硬编码路径 | 读取 mcp.json | CONFIRMED: 4 处 `/Users/joe/...` |
| audit-lifecycle | GOAL_STATE_CONTRACT.md stdio only | 读取文件，全文搜索 "MCP" | CONFIRMED: 无 MCP mention |
| audit-lifecycle | verifyx closeout_evaluate | 读取 verifyx/SKILL.md line 51 | CONFIRMED: `closeout_evaluate stdio` |
| audit-lifecycle | implementx 废弃 hook 引用 | 读取 implementx/SKILL.md | CONFIRMED: description 含 "REVIEW_GATE hard block off" |
| audit-academic | 演示文稿冗余 | wc -l 确认 425+128+102=655 | CONFIRMED |
| audit-tools | token-optimization 内容充实 | wc -l 确认 234 行（上轮已充实） | PARTIAL: 内容已改善但仍有 deprecation 问题 |

### 已完成的上轮工作（无需重复）

| 项目 | Commit | 状态 |
|------|--------|------|
| Codex → 无宿主化 | `10182854`~`12ae6dbe` | DONE |
| 归档 skill 引用修复 | `10182854` | DONE |
| $CODEX_HOME 路径修复 | `10182854` | DONE |
| routing_gate 修复 | `10182854` | DONE |
| model 字段更新 | `10182854` | DONE |
| LAYERS.md [archived] 标签 | `10182854` | DONE |
| Frontmatter schema 分层 | `10182854` | DONE |
| 行为控制字段全覆盖 | `10182854` | DONE |
| Ghost 目录规范化 | `12ae6dbe` | DONE |
| plan-mode 无宿主化 | `ffa525c0` | DONE |
| P2 Manifest 精简 | `59d8d308` | DONE |
| disable_review_gate 移除 | `d0807a01` | DONE |
| advisory 模式适配 | `aa6f7b49` + `d91f13a6` | DONE |

---

*本 plan 基于 7 个审计 agent 的发现交叉验证生成，所有发现均已通过文件读取确认。*
*经 adversarial review 修订（3 blocking + 10 non-blocking fixes applied）。*
*执行顺序严格按 Phase 1 → 2 → 3，每个 Phase 独立可交付。*

---

## 附录 B：Adversarial Review 结果

**审查结论**：NEEDS_REVISION（首轮）→ APPROVED（修订后）

### Blocking fixes（已应用）

| # | 问题 | 修复 |
|---|------|------|
| R-1 | QW-3 rm 正则 `rm -[a-zA-Z]*` 不能排除 `-rf`（事实性错误） | 改为「移除 rm wildcard allow 或收紧为 `rm -f *`」 |
| R-2 | mcp.json `${VAR}` 环境变量插值方案不可执行 | 改为 README 说明方案 / symlink 方案 |
| R-3 | M-12 AGENTS.md 是 Rust 生成文件，工作量标注错误 | 改为代码改动，工作量 S→M |

### Non-blocking improvements（已应用）

| # | 改进 | 操作 |
|---|------|------|
| R-4 | M-1 severity CRITICAL 过高 | 降为 WARNING |
| R-5 | M-14 归档状态应提前修复 | Phase 2 → Phase 1 |
| R-6 | Phase 3.6 声明式编排过度设计 | 降级为 ADR 备忘 |
| R-7 | Phase 3.4 路径级规则当前不必要 | 降级为 P3 备忘 |
| R-8 | M-17 GOAL_STATE 模板去重收益低 | 降级为 P3 备忘 |
| R-9 | 验证脚本 grep 模式过于宽泛 | 精确匹配完整旧措辞 |
| R-10 | 遗漏 docs/ 目录旧接口检查 | 补充到 Phase 1 验证计划 |
| R-11 | 遗漏 .archive-cold/ 目录审查 | 补充到归档状态修复 |
| R-12 | 遗漏 hook 脚本行为验证 | 补充到 Phase 1 验证 |
| R-13 | M-10/M-11 可与 M-2 同批次 | 调整时间线 |
