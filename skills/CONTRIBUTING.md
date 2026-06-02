# Skill 贡献指南

## 快速开始

1. 在 `skills/` 下创建目录：`skills/<slug>/`
2. 编写 `SKILL.md`（参考下方模板）
3. 在 `skills/SKILL_MANIFEST.json` 中注册
4. 运行 `router-rs framework skills refresh --write --write-companions` 同步 catalog/tiers
5. 运行 `python3 scripts/validate-manifest.py` 确认全绿
6. 提交 PR

## SKILL.md 模板

```markdown
---
slug: my-new-skill
layer: L3
owner: none
gate: none
priority: P2
description: 一句话描述技能做什么
session_start: preferred
trigger_hints:
  - 中文触发词
  - english trigger phrase
  - /slash-command
source: project
source_position: null
skill_path: skills/my-new-skill/SKILL.md
host_platforms:
  - antigravity
  - claude-desktop
  - claude-code
  - cursor
  - opencode
kind: skill
allowedTools:
  - Bash
  - Read
  - Write
invocation: auto
---

# my-new-skill

## 触发条件
明确描述何时激活此 skill。

## 执行指令
Claude 执行的具体步骤。

## Do not use
明确列出不应使用此 skill 的场景。

## References
- [参考文档](references/guide.md)
```

## Frontmatter 字段说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `slug` | 是 | kebab-case 唯一标识符 |
| `layer` | 是 | L0/L1/L2/L3/L4 |
| `owner` | 是 | gate/none/evidence/artifact/delegation/source |
| `gate` | 是 | 门控类型（delegation/source/artifact/evidence/none） |
| `priority` | L0 必填 | P1/P2 |
| `description` | 是 | 一句话触发条件描述 |
| `session_start` | 是 | required/preferred/n/a |
| `trigger_hints` | 是 | 中英文触发词列表 |
| `host_platforms` | 是 | 支持的宿主平台（5 个全集） |
| `kind` | 是 | skill/cold |
| `allowedTools` | 推荐 | 预授权工具列表（减少运行时弹窗） |
| `invocation` | 推荐 | auto/manual（Claude 自动判断 vs 仅用户手动） |

## 分层规范

| 层 | 定位 | gate | 典型 priority |
|----|------|------|---------------|
| L0 | 框架内核/路由/gate | delegation/source | P1-P2 |
| L1 | 核心方法论 | 混合 | P1 |
| L2 | 技术底座 | 混合 | P1-P2 |
| L3 | 平台/工具/产物 | artifact/evidence | P1-P2 |
| L4 | 高语义专业领域 | none | P2 |

## 三层元数据同步

本仓库有三层元数据，修改时需保持一致：

1. **SKILL_MANIFEST.json** — 源头数据（手动编辑）
2. **SKILL_ROUTING_RUNTIME.json** — 路由表（手动编辑或 `router-rs` 生成）
3. **SKILL_PLUGIN_CATALOG.json** — 插件目录（自动生成，勿手动编辑）

**同步流程**：
```bash
# 编辑 manifest 后
router-rs framework skills refresh --write --write-companions
# 验证
python3 scripts/validate-manifest.py
```

## 校验规则

运行 `python3 scripts/validate-manifest.py` 检查：

| 规则 | 说明 |
|------|------|
| R1 | host_platforms 无重复（manifest + routing） |
| R2 | routing skill_path 文件存在性 |
| R3 | 冷热一致性（kind=cold 不在 routing） |
| R4 | L0 gate skill 必须有 priority |
| R5 | manifest 与 catalog slug 集合一致 |
| R6 | catalog host_support.platforms 无重复 |
| R7 | catalog skill_path 文件存在性 |
| R8 | trigger_hints 跨 skill 重叠检测（INFO 级别） |

## CI 管线

| 脚本 | 功能 |
|------|------|
| `scripts/validate-manifest.py` | 8 条元数据校验规则 |
| `scripts/ci/check-routing-regression.sh` | 路由回归测试（accuracy >= 0.95） |
| `scripts/ci/check-skills-no-operator-pip.sh` | 禁止 pip install（用 uv） |
| `scripts/ci/check-cursor-hooks-parity.sh` | Cursor hooks 一致性 |

## References 目录

大型 skill 应将详细指南放入 `references/` 子目录，实现 deferred loading：

```
skills/my-skill/
  SKILL.md          # 主文件（精简，<15KB）
  references/
    guide.md        # 详细指南（按需加载）
    checklist.md    # 检查清单
```

主文件通过 `[references/guide.md](references/guide.md)` 引用，Claude 仅在需要时读取。

## 注意事项

- `SKILL_PLUGIN_CATALOG.json` 由 `router-rs` 自动生成，**不要手动编辑**
- `host_platforms` 必须包含全部 5 个平台（antigravity, claude-desktop, claude-code, cursor, opencode）
- trigger_hints 中英文都要包含
- SKILL.md 正文控制在 15KB 以内，超出部分下沉到 references/
