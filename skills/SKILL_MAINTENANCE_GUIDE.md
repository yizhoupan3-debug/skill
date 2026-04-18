# Skill 库维护约定

## 单一事实来源

- `skills/` 是唯一可写的 skill 源目录。`~/.codex/skills` 必须只是指向它的符号链接。
- system skill 放 `skills/.system/`。不要同时保留两份 live source。

## 新增 Skill 最小清单

1. 创建 `skills/<skill-name>/SKILL.md`，frontmatter 必填：`name`, `description`, `routing_layer`, `routing_owner`, `routing_gate`, `session_start`
2. Body 必含：`## When to use` + `## Do not use`
3. 更新 [SKILL_ROUTING_INDEX.md](file:///Users/joe/Documents/skill/skills/SKILL_ROUTING_INDEX.md)
4. 运行验证：
   ```bash
   python3 scripts/sync_skills.py --apply
   python3 scripts/check_skills.py --verify-codex-link
   python3 scripts/check_skills.py --include-system --verify-codex-link
   ```
   本地人工执行这些高输出命令时，可按 [`RTK.md`](/Users/joe/Documents/skill/RTK.md) 改用 `rtk ...` 包装形式。
5. 提交后 CI 自动验证（`.github/workflows/skill-ci.yml`）

## 改 Skill 必查

- 触发词是否变化 → 更新 description
- 边界是否变化 → 更新索引
- 是否引入第二份 live source → 删除多余副本

## 边界重叠处理

默认 **incumbent-first**：优先修改旧 skill。仅当 owner/gate/overlay 角色变化、运行时差异明显、或旧 skill 触发精度严重受损时才新建。

## Description 写法

```
[角色] + [领域名词] + [用户自然说法] + [边界词]
```

- 第一行 brief：≤ 120 chars
- 整体推荐：180–450 chars，> 600 chars 视为偏重
- 覆盖用户真实说法（中英混合）
- session_start 为 required/preferred 时，必须包含 "每轮对话开始 / first-turn / conversation start"

## Git hooks

已配置 `.githooks/`（`pre-commit` 自动校验 + 评分，`post-commit` 自动 push）。首次安装：

```bash
python3 scripts/sync_skills.py --install-hooks
```

## 技能演化与凝结 (Evolution & Condensation)

### 1. 自动演化审计
每周 Cron 任务通过 `evolution_engine.py` 自动执行：
- **动态健康分**：结合静态评分与路由记录。低于 60 分即标记为 `Critical Outlier`。
- **冲突审计**：识别高频错配对（Reroute Pairs），强制建议收紧 `init` 技能边界。

### 2. 工作流凝结协议 (Workflow-to-Skill)
当审计报告识别出“待批阅工作流”时，需严格执行以下流程：
1. **模式确认**：人工批阅审计 Issue，确认该任务流具备独立凝结价值。
2. **标准化生成**：必须基于 `$skill-developer` 协议，显式处理与 `iterative-optimizer` 等通用技能的 `Do not use` 边界。
3. **回归校验**：新技能就绪后，需运行 `python3 scripts/sync_skills.py --apply` 更新注册表。
4. **闭环验证**：初始设为 `P2` 优先级，在接下来的会话中观察是否成功拦截原有的“通用型”路由。
