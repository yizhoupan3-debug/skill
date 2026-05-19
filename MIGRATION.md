# Skill 真源迁移（2026-05-19）

- **唯一可写 skill 源**：`/Users/joe/Developer/skill/skills/`
- **Codex 全局**：`~/.codex/skills` → `artifacts/codex-skill-surface/skills`
- **Agents 全局**：`~/.agents/skills` → 同上 surface
- **冻结（勿再维护 live）**：`/Users/joe/Documents/skill/`、`~/skills_backup`（已删）

## 日常维护

```bash
export SKILL_FRAMEWORK_ROOT=/Users/joe/Developer/skill
cd "$SKILL_FRAMEWORK_ROOT"
just publish    # 或：ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1 … update-one-shot
just doctor
```

## 其它 Cursor 项目接入

```bash
cd /path/to/project
"$SKILL_FRAMEWORK_ROOT/scripts/cursor-bootstrap-framework.sh" \
  --framework-root "$SKILL_FRAMEWORK_ROOT" \
  --with-cursor-rules --with-configs
```
