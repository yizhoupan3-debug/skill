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

## 默认工作流（全宿主）

- **GSD** 为默认生命周期（`/gsd-new-project` … `/gsd-ship`），已在 `RUNTIME_REGISTRY` 为 `codex-cli` / `cursor` / `claude-code` / `claude-desktop` 注册；`AGENTS.md` 与各宿主 framework 投影文案一致。
- `/autopilot` 仍为 opt-in。

## Cursor：framework 规则仅用户级

- **`framework.mdc`** 只安装到 **`$CURSOR_HOME/rules/`**（默认 `~/.cursor/rules/framework.mdc`），**不要**在业务仓库维护项目级副本。
- 一次性（或升级后）在 framework 仓执行：

```bash
cd "$SKILL_FRAMEWORK_ROOT"
cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
  framework host-integration install --framework-root "$PWD" --project-root "$PWD" \
  --artifact-root "$PWD/artifacts" --scope user --to cursor
```

## 其它 Cursor 项目接入

```bash
cd /path/to/project
"$SKILL_FRAMEWORK_ROOT/scripts/cursor-bootstrap-framework.sh" \
  --framework-root "$SKILL_FRAMEWORK_ROOT" --with-configs
# 可选：--with-cursor-rules 仅 symlink harness gate 规则（不含 framework.mdc）
```
