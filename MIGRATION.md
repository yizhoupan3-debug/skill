# Skill 真源迁移（2026-05-19）

- **唯一可写 skill 源**：`$SKILL_FRAMEWORK_ROOT/skills/`（示例：`$SKILL_FRAMEWORK_ROOT/skills/`）
- **Codex 全局**：`~/.codex/skills` → `artifacts/codex-skill-surface/skills`
- **Agents 全局**：`~/.agents/skills` → 同上 surface
- **已删除（2026-05-19 激进清理）**：`~/Documents/skill`、`~/Documents/skill.nosync` 及同批空壳/无关目录；勿再引用。`~/skills_backup` 此前已删。
- **生成物漂移检测**：`router-rs` 仍把历史路径 `/Users/joe/Documents/skill` 当作 forbidden marker，用于拒绝陈旧 bootstrap/投影。

## 日常维护

```bash
export SKILL_FRAMEWORK_ROOT="${SKILL_FRAMEWORK_ROOT:-/path/to/Developer/skill}"
cd "$SKILL_FRAMEWORK_ROOT"
just publish    # 或：ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1 … update-one-shot
just doctor
```

## 投影同步清单（改 AGENTS / 叙事 / GSD 文案后）

1. 仓库根改 `AGENTS.md` 且依赖 Codex：`cargo build --manifest-path scripts/router-rs/Cargo.toml` + `router-rs framework sync-entrypoints --repo-root "$SKILL_FRAMEWORK_ROOT"`（或 `codex sync --repo-root "$SKILL_FRAMEWORK_ROOT"`）。
2. Cursor 用户级 framework：`router-rs framework host-integration install --to cursor --scope user --framework-root "$SKILL_FRAMEWORK_ROOT" --project-root "$SKILL_FRAMEWORK_ROOT"`.
3. 改 `configs/framework/host_projection_narrative.json` 或 `RUNTIME_REGISTRY.json` **review_gate**：**无需** rebuild；重启 hook 子进程。
4. 发布前：`router-rs framework maint update-one-shot`（全量 drift-gate）；日常仅 `framework doctor` **不等于** drift-gate 通过。

## 默认工作流（全宿主）

- **GSD** 为默认生命周期（`/gsd-new-project` … `/gsd-ship`），已在 `RUNTIME_REGISTRY` 为 `codex-cli` / `cursor` / `claude-code` / `claude-desktop` 注册；`AGENTS.md` 与各宿主 framework 投影文案一致。
- `/autopilot` 已退役；连续执行请用 `/gsd-execute-phase`（goal drive 经 `GOAL_STATE.json` + `framework_autopilot_goal` stdio）。

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

## router-rs 连续性 / 门控（findings-remediation-2026-05）

| 变更 | 操作面 |
|------|--------|
| **Stop checkpoint + supervisor** | 自动 checkpoint（`focus: false`）会同步 `.supervisor_state.json.task_id` 到刷新任务；**不**移动 `active_task` / `focus_task`。 |
| **`ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK`** | **默认开启**（unset = strict）；仅磁盘 `GOAL_STATE` **不再**满足 pre-goal。宽松 legacy：`0` / `false` / `off` / `no`。 |
| **`ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1`** | hook-state 写失败时 beforeSubmit 仍放行（应急）；**默认 fail-closed**。 |
| **`ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`** | **Claude 专用**；默认 **关闭**（缺失 `fork_context` 不清 `REVIEW_GATE`）。**勿**与 Cursor 同名 env 混用。 |
| **Review pending cap** | 达 `ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX` 时 `subagentStart` 返回 `permission: deny`。 |
| **Claude review_gate** | `.claude/hook-state/review_gate_*.json` 写入使用 `flock`（与 Codex 对齐）。 |

升级后若 continuity 仍报 supervisor 分歧：触发一次 **Cursor Stop** 或 `router-rs framework session-artifact-write`（`focus: false`）。

## Cursor：hooks 减法闭集（2026-05-20）

本仓 [`.cursor/hooks.json`](.cursor/hooks.json) 默认仅 **7** 事件：`beforeSubmitPrompt`、`stop`、`sessionStart`、`sessionEnd`、`postToolUse`、`subagentStart`、`subagentStop`。

**已移除的注册**（`router-rs` handler 仍保留，可手动加回 hooks.json）：

| 事件 | 恢复代价 / 说明 |
|------|-----------------|
| `afterAgentResponse` | compact findings 可提前一轮清 `REVIEW_GATE`；默认改 `Stop` tail |
| `beforeShellExecution` / `afterShellExecution` | SessionEnd 终端 PID 账本更全；需 `ROUTER_RS_CURSOR_KILL_STALE_TERMINALS≠0` 才有意义 |
| `afterFileEdit` | Agent 改 `.rs` 后自动 `rustfmt` |
| `preCompact` | compaction 前 RFV/门状态摘要 |

**门控 `timeout`**：`beforeSubmitPrompt` / `stop` / `postToolUse` / `subagentStart` / `subagentStop` 均为 **20s**（`sessionStart` 5s、`sessionEnd` 15s）。`postToolUse` 超时会导致 review multiset / shell 账本不完整 — 见 [`docs/hosts/cursor.md`](docs/hosts/cursor.md)「PostToolUse timeout」。

**模板同步**：[`configs/framework/cursor-hooks.workspace-template.json`](configs/framework/cursor-hooks.workspace-template.json) 须与 [`.cursor/hooks.json`](.cursor/hooks.json) 一致（`bash scripts/ci/check-cursor-hooks-parity.sh`）。

**内存相关**：见 [`docs/hosts/cursor.md`](docs/hosts/cursor.md)「内存 / release」；项目 env [`.cursor/router-rs-hook.env`](.cursor/router-rs-hook.env)。

## Claude Code：hook env 与 Cursor 对齐（2026-05-20）

Claude 宿主**本就**仅 4 个 hook 事件（`PreToolUse` / `UserPromptSubmit` / `PostToolUse` / `Stop`），无需删除 Cursor 侧已移除的 5 个事件。

| 项 | 路径 |
|----|------|
| 项目 env 真源 | [`.claude/router-rs-hook.env`](.claude/router-rs-hook.env) |
| 模板 / 新仓库复制 | [`configs/framework/claude-router-rs-hook.env`](configs/framework/claude-router-rs-hook.env) |
| Launcher | [`configs/framework/claude-router-rs-hook.sh`](configs/framework/claude-router-rs-hook.sh)（release 优先，与 Cursor 同序） |
| 重装 hooks 合并 | `framework host-integration install --to claude --scope project` |

默认 **`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`**（减 PostTool 证据写盘）。**不要**把 `ROUTER_RS_CURSOR_*` 写入 Claude env（无意义）。

## Harness framework hardening（2026-05-20）

| 变更 | 说明 |
|------|------|
| `review_gate` lane 集 | 磁盘 [`configs/framework/RUNTIME_REGISTRY.json`](configs/framework/RUNTIME_REGISTRY.json) + [`registry_loader.rs`](scripts/router-rs/src/registry_loader.rs)（**无** compile-time embed）；改 lane **无需** `cargo build`，重启 hook 子进程即可 |
| 宿主投影 GSD/review 文案 | [`configs/framework/host_projection_narrative.json`](configs/framework/host_projection_narrative.json)；`host-integration install` 读取；勿在 `host_integration.rs` 硬编码 |
| `generated-artifacts-status` | **`framework doctor`** / `--skip-generator-run` / `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS=1` → **metadata-only**（快）。**`update-one-shot`** 仍要求全量 **drift-gate** `ok: true` |
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1` | 全链路关闭审稿并**清除** `.cursor/hook-state` 内 review 字段；非「仅不 nag」 |
| active/focus GOAL 分裂 | 有 `continuity:active_goal_missing_focus_has_goal` 时**不**注入 `GSD_GOAL_CONTINUE`；用 `framework task-state-resolve` 或修正 `active_task.json` |
| Review soft-nag 超 cap | 超过 `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` 后仍可有 REVIEW 提示，但**不再**单独以 `continuity_suppressed=review_soft_nag` 阻断 GSD 续跑 |
| `SKILL_ROUTING_RUNTIME.scope` | `hot_skill_count`/`full_skill_count` = 热表行数；`manifest_skill_count` = 全 manifest 行数 |
| 文档真源 | 硬化叙述见 [`docs/harness_architecture.md`](docs/harness_architecture.md) §2.3、[`docs/framework_operator_primer.md`](docs/framework_operator_primer.md)、[`docs/rust_contracts.md`](docs/rust_contracts.md) |

## 文档与计划卫生（2026-05-20）

| 移除 | 替代真源 |
|------|----------|
| `docs/plans/*.md`（除 [`docs/plans/README.md`](docs/plans/README.md)） | GSD：`artifacts/current/<task_id>/ROADMAP.md`；Cursor Plan：活跃任务 `.cursor/plans/*.plan.md` |
| `docs/history/**` | git 历史；[`MIGRATION.md`](MIGRATION.md) |
| `configs/codex/docs/**` | [`docs/README.md`](docs/README.md)、宿主手册 [`docs/hosts/`](docs/hosts/) |
| `skills/autopilot/`、`skills/_archived/autopilot/` | [`skills/gsd/`](skills/gsd/) + `/gsd-execute-phase` |

勿在 issue/评论中链接已删路径；契约以 [`docs/README.md`](docs/README.md) 索引为准。
