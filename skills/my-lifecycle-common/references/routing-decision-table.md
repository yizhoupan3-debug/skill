# 统一生命周期决策表

> 查阅优先级：本表为快速索引，权威真源在 `AGENTS.md` 各 § 与对应 `SKILL.md`。

## 1. Skill 与生命周期阶段映射

| 阶段 | Skill | 触发 | 核心产出 | 限制 |
|------|-------|------|----------|------|
| 讨论/需求 | `discussx` | `/discussx` | 需求澄清、约束收集 | 不变异产品代码 |
| 深度访谈 | `deepinterview` | `/deepinterview` | 模糊收敛、证据先行 | 只读，纯调研 |
| 规划 | `planx` | `/planx` | `ROADMAP.md` + `WAVE_STATE.json` | 不变异产品代码 |
| 策划文档闸门 | `plan-mode` | 宿主 Plan 模式 / 自动 | 可验收 todo + 五行证据 | 小中型任务轻量；跨模块升级 audit plan |
| 实施 | `implementx` | `/implementx` | 一口气跑完全部 wave | 主线程只调度，子代理写 lane-notes |
| 验证+交付 | `verifyx` | `/verifyx` | 证据索引、测试、closeout、goal complete | 合并 verify-work 与 ship 检查 |
| 代码审查 | `code-review-deep` | `$code-review-deep` / review 请求 | 严重度排序 findings | 默认只读 findings-only |

## 2. 场景决策矩阵

| 场景 | 推荐 skill | 备选 | 说明 |
|------|-----------|------|------|
| **新项目启动**（从零开始） | `discussx` → `planx` → `implementx` → `verifyx` | `gsd-new-project` + `gsd-plan-phase` | 默认 My 生命周期；需求不清时先 `deepinterview` |
| **需求模糊/多解** | `deepinterview` | `discussx` 多轮 | 当存在 ≥2 种可行路径或关键歧义时进入 |
| **架构讨论**（不写码） | `discussx` | `gsd-discuss-phase` | 多轮用户门控，直到显式 `/planx` |
| **任务规划**（已有需求） | `planx` | `plan-mode`（宿主 Plan 模式） | `planx` 生成完整 ROADMAP + DAG；`plan-mode` 生成轻量可验收 todo |
| **单文件/小改动** | 直接实施 | `plan-mode` 轻量五行 | 跳过 discuss/plan 阶段，用 `plan-mode` 收口即可 |
| **实施（有 ROADMAP）** | `implementx` | — | 一口气执行所有 wave；`my-light` 模式下无硬拦 |
| **实施后验证** | `verifyx` | `gsd-verify-work` | 证据索引 + closeout + goal complete 一步完成 |
| **代码审查** | `code-review-deep` | `code-review`（低/中置信度） | 深度审查用 lens 扩展目录；轻量审查用基础版 |
| **文档/论文** | `paper-workbench` | `doc` | 学术写作走 prose-chain；普通文档走 `doc` |
| **修复 CI/PR 评论** | `gh-fix-ci` / `gh-address-comments` | — | 有明确 GitHub 上下文时直接路由 |
| **多 agent 编排** | `agent-swarm-orchestration` | — | 决定本地/边车/团队编排 |

## 3. 宿主入口差异

| 宿主 | 入口方式 | Goal 驱动 | Closeout | 特殊差异 |
|------|---------|-----------|----------|----------|
| **claude** | 斜杠命令（`/discussx` 等） | `framework_goal_drive` stdio | `closeout_gate` advisory（my-light） | PreToolUse/Stop hook advisory 模式 |
| **cursor** | Plan 模式自动触发 `plan-mode` | `framework_goal_drive` stdio | advisory（my-light 下无硬拦） | `.cursor/rules/*-gate.mdc`；hook 不注入 spawn-first nudge |
| **opencode** | 斜杠命令 | `framework_goal_drive` stdio | advisory | 配置在 `opencode.json` |

**宿主权威分层**：跨宿主协议 → `AGENTS.md`；宿主执行面 → `AGENTS_<HOST>.md`；skill 路由 → `SKILL_ROUTING_RUNTIME.json`；hook 行为 → 各宿主 `hooks.json` + `router-rs`。

## 4. 默认生命周期流程

```
/discussx → /planx → /implementx → /verifyx
   需求       规划       实施          验证+交付
```

- **my-light profile**：closeout/complete 为 advisory；`REVIEW_GATE` 无硬拦；spawn-first nudge 关闭。
- **非 my-light**：`closeout_gate` 未满足时 advisory 提醒 `complete`。
- Goal 磁盘真源：`artifacts/current/<task_id>/GOAL_STATE.json`。
