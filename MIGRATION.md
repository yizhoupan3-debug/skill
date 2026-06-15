# Skill 真源迁移（2026-05-19）

## 闭集宿主收敛（2026-06）

**权威闭集**（仅此 4 个 id）：`codex`、`claude-code`、`cursor`、`opencode` — `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`。

| 退役 id | 替代 / 说明 |
|---------|-------------|
| `codex-cli`、`codex-app` | **`codex`**（`install --to codex`） |
| `claude-desktop` | **`claude-code`**（`install --to claude-code` / `./scripts/install-claude.sh`）；勿再 `install-claude-desktop.sh` |
| `antigravity`、`antigravity-app`、`antigravity-cli` | 早期实验性宿主 id，产品线已终止（2026-06），无替代宿主 |

**文档**：宿主手册见 [`docs/hosts/`](docs/hosts/)。运维见 [`docs/operations/index.md`](docs/operations/index.md)（按功能模块 B0–B11）。

**操作**：升级后对各仍用宿主重跑 `framework host-integration install --to <id>`；`just doctor` 确认 `host_targets.supported` 与本地投影一致。

## Rust crate 路径（2026-05-28）

| 旧路径 | 新路径 |
|--------|--------|
| `scripts/router-rs/` | `core/router-rs/` |
| `scripts/evolution-rs/` | `core/evolution-rs/` |
| `scripts/autoresearch-rs/` | `core/autoresearch-rs/` |

Hook launchers、CI、`Cargo.toml` workspace 已对齐 `core/*`；勿再引用 `scripts/*-rs`。

- **唯一可写 skill 源**：`$SKILL_FRAMEWORK_ROOT/skills/`（示例：`$SKILL_FRAMEWORK_ROOT/skills/`）
- **Codex 全局**：与其他宿主统一使用运行时路由（`skills/SKILL_ROUTING_RUNTIME.json`），不再需要文件投影
- **Agents 全局**：同 Codex，使用运行时路由
- **已删除（2026-05-19 激进清理）**：`~/Documents/skill`、`~/Documents/skill.nosync` 及同批空壳/无关目录；勿再引用。`~/skills_backup` 此前已删。
- **生成物漂移检测**：`router-rs` 仍把历史路径 `/Users/joe/Documents/skill` 当作 forbidden marker，用于拒绝陈旧 bootstrap/投影。

## Codex 多账户示例（2026-05）

- **`configs/codex/sub_accounts.example.json` 已移除**（2026-05）。勿再在文档或脚本中引用该路径。
- 多账户 / 子账户配置见 Codex 宿主文档 [`docs/hosts/codex.md`](docs/hosts/codex.md)；机读状态仍使用 `configs/codex/sub_accounts.state.json`（gitignore，本机生成）。

## 日常维护

**Steady-state 操作面**（宿主差异、`REVIEW_GATE` / review-lite、fork_context、自检）：[`docs/hosts/`](docs/hosts/) + [`AGENTS.md`](AGENTS.md)。实现 crate 路径见上表 **`core/router-rs`**（勿再引用 `scripts/router-rs`）。

```bash
export SKILL_FRAMEWORK_ROOT="${SKILL_FRAMEWORK_ROOT:-/path/to/Developer/skill}"
cd "$SKILL_FRAMEWORK_ROOT"
just publish    # 或：ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1 … update-one-shot
just doctor
```

## 投影同步清单（改 AGENTS / 叙事 / My lifecycle 文案后）

1. 仓库根改 `AGENTS.md` 且依赖 Codex：`cargo build --manifest-path core/router-rs/Cargo.toml` + `router-rs framework sync-entrypoints --repo-root "$SKILL_FRAMEWORK_ROOT"`（或 `codex sync --repo-root "$SKILL_FRAMEWORK_ROOT"`）。
2. Cursor 用户级 framework：`router-rs framework host-integration install --to cursor --scope user --framework-root "$SKILL_FRAMEWORK_ROOT" --project-root "$SKILL_FRAMEWORK_ROOT"`.
3. 业务仓 harness gate 规则（含 `subagent-model-inherit.mdc`）：`cursor-bootstrap-framework.sh --with-cursor-rules` 或手动 symlink `.cursor/rules/*.mdc`。
4. 改 `configs/framework/host_projection_narrative.json` 或 `RUNTIME_REGISTRY.json` **review_gate**：**无需** rebuild；重启 hook 子进程。
5. 发布前：`router-rs framework maint update-one-shot`（全量 drift-gate）；日常仅 `framework doctor` **不等于** drift-gate 通过。

### Claude Code（framework 源码仓）

- **`.claude/settings.json`** 由 `install --to claude-code` 材料化（四事件 hook）。
- **Claude / Codex 退役面**：`claude-desktop`、`codex-app` 已移除；Codex 用 **`codex`**，Claude 用 **`claude-code`**。
- 勿再依赖 **`.claude/hooks/router-rs-hook.sh`**（deprecated shim）；真源为 `configs/framework/claude-router-rs-hook.sh` + settings hooks。

## 默认工作流（全宿主）

- **个人默认生命周期（2026-05-21）**：`/discussx` → `/planx` → `/implementx` → `/verifyx`（verify 含 ship）。热路由见 `skills/SKILL_ROUTING_RUNTIME.json`；全宿主 Stop 上 `REVIEW_GATE` 为 advisory-only（见 [`docs/spec.md`](docs/spec.md) §6.2）；`lifecycle_profile: my-light` 另 suppress review nudge 与 spawn-first。
- **改 routing 后必做**（否则新对话仍见旧斜杠）：`just publish`（已刷新 Cursor user `framework.mdc` 与 Claude user/project `.claude/*`）；**重启 Cursor**。GSD 整树与 `/gsd-*` runtime 识别已于 **2026-05 彻底移除**。
- **legacy-gsd / `/gsd-*`**：**已删除**（非冷表、非 CI stub）；hook 与 registry **不再识别**。个人入口仅 My 四命令（下表）。
- `/autopilot` 已退役；连续执行请用 `/implementx`（一口气跑完 `WAVE_STATE` 全部 wave；goal drive 经 `GOAL_STATE.json`）。

| 退役（个人） | 替代 |
|--------------|------|
| `/gsd-new-project` + `/gsd-discuss-phase` | `/discussx` |
| `/gsd-plan-phase` | `/planx` |
| `/gsd-execute-phase` | `/implementx` |
| `/gsd-verify-work` + `/gsd-ship` | `/verifyx` |
| `/discuss-phase`、`/plan-phase`、`/execute-phase`、`/verify-work`、`/ship`、`/new-project`（无前缀 GSD 残留） | 已归档，**无**个人斜杠；用上表四命令 |

## Cursor：framework 规则仅用户级

- **`framework.mdc`** 只安装到 **`$CURSOR_HOME/rules/`**（默认 `~/.cursor/rules/framework.mdc`），**不要**在业务仓库维护项目级副本。
- 一次性（或升级后）在 framework 仓执行：

```bash
cd "$SKILL_FRAMEWORK_ROOT"
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --framework-root "$PWD" --project-root "$PWD" \
  --artifact-root "$PWD/artifacts" --scope user --to cursor
```

## Claude Code：与 Cursor 对齐 My 生命周期（2026-05-29）

- **症状**：`~/.claude/rules/framework.md` 仍写 `/gsd-*` 或 `GOAL_CONTINUE` → 与 Cursor `framework.mdc` 的 `/discussx`→`/verifyx` 不一致；路由仍走仓库 `skills/SKILL_ROUTING_RUNTIME.json`，但**入口叙事过时**。
- **真源**：`configs/framework/host_projection_narrative.json` + `install --to claude-code`。
- **推荐一键**（framework 仓或业务仓）：

```bash
cd "$SKILL_FRAMEWORK_ROOT"
./scripts/install-claude.sh
# 或仅刷新全局（对齐 ~/.cursor/rules/framework.mdc）：
./scripts/install-claude.sh --scope user
```

- **`just publish`** / `update-one-shot` 对 **claude-code** 执行 **project + user** 双 scope（与 Cursor user-only 不同：Claude Code 仍需项目级 `.claude/settings.json` hooks）。
- **其它仓库接入**：`scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"`（symlink `skills/`、`AGENTS.md` + project 投影）；全局规则再跑 `install-claude.sh --scope user`。
- **业务仓注意**：在 framework 仓执行 `just publish` **不会**自动更新其它项目目录下的 `.claude/*`；每个消费仓库须在本机重跑 `install-claude.sh --project-root <业务仓根>`（或先 `claude-bootstrap-framework.sh`）。

## 其它 Cursor 项目接入

```bash
cd /path/to/project
"$SKILL_FRAMEWORK_ROOT/scripts/cursor-bootstrap-framework.sh" \
  --framework-root "$SKILL_FRAMEWORK_ROOT" --with-configs
# 可选：--with-cursor-rules 仅 symlink harness gate 规则（不含 framework.mdc）
```

## router-rs 门控（findings-remediation-2026-05）

| 变更 | 操作面 |
|------|--------|
| **Stop 自动 checkpoint** | **已拔除**（2026-05）；`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT` 无操作。显式 `session-artifact-write` / Desktop MCP `session_checkpoint` 仍可用。 |
| **`ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK`** | **默认开启**（unset = strict）；仅磁盘 `GOAL_STATE` **不再**满足 pre-goal。宽松 legacy：`0` / `false` / `off` / `no`。 |
| **`ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1`** | hook-state 写失败时 beforeSubmit 仍放行（应急）；**默认 fail-closed**。 |
| **`ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`** | **Claude 专用**；默认 **关闭**（缺失 `fork_context` 不清 `REVIEW_GATE`）。**勿**与 Cursor 同名 env 混用。 |
| **Review pending cap** | 达 `ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX` 时 `subagentStart` 返回 `permission: deny`。 |
| **Claude review_gate** | `.claude/hook-state/review_gate_*.json` 写入使用 `flock`（与 Codex 对齐）。 |
| **Codex stable session + Stop review**（2026-05 wave-1） | `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY` **默认 on**；legacy `=0`。无稳定键时 hook-state 用确定性 fallback（非 per-invocation 随机）。Stop 在 review 已武装且无独立子代理证据时注入 **advisory nudge**（**不** `decision:block`）；**含**无 hook-state 文件路径；Stop 载荷 review 措辞 alone 不能清门。 |
| **Codex wave-2 P1-4..P1-7**（2026-05） | PostTool hook-state 锁失败 **fail-closed**（与 UserPromptSubmit 同形）。`stop_hook_active` 默认仍执行 review/closeout；仅 `ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS=1` 跳过门控。Stop closeout：`closeout_stop_followup_for_completion_text`。Codex fork 推断用 **`ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`**（**不**读 Cursor env）。 |

手动画板分歧：用 `router-rs framework session-artifact-write` 或 Desktop MCP `session_checkpoint` 显式对齐 `artifacts/current/<task_id>/`。

## Spawn-first 配对审稿（deep-review-multiagent-compact-2026-05，2026-05-21）

| 项 | 说明 |
|----|------|
| **Registry** | `review_gate.spawn_first_enabled`（默认 true）、`spawn_first_nudge`（一行文案）、`spawn_first_includes_model_inherit_by_host.cursor`（去重 model inherit nudge）、`subagent_model_inherit_nudge_by_host` |
| **`ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE`** | `0`/`false`/`off`/`no` 关闭 Cursor beforeSubmit model inherit 单行（默认开；与 my-light / REVIEW_GATE 无关） |
| **Cursor UPS re-arm** | fresh deep-review cycle 调用 `reset_review_cycle_progress(preserve_session_guards=true)`；保留 `review_pending_cap_refused` 与 open subagent 计数；见 [`docs/hosts/cursor.md`](docs/hosts/cursor.md) |
| **`ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE`** | `0`/`false`/`off`/`no` **关闭** beforeSubmit/UPS spawn-first 单行 nudge（**零注入**，无 fallback）；**不** 改变 REVIEW_GATE 清门阈值 |
| **窄范围** | `review ./path`、`small_task`、不用子代理 → **不武装** `review_required`（五宿主 `is_narrow_review_prompt`） |
| **禁止** | `start_count≥2` 清门、缺 `review-lanes` 文件即 Stop block |
| **细则** | [`skills/code-review-deep/SKILL.md`](skills/code-review-deep/SKILL.md)、[`AGENTS.md`](AGENTS.md) |

## Cursor / Codex wave-2 review gate（2026-05）

| 主题 | 行为 |
|------|------|
| **主线程 compact 清门（Cursor）** | 无可数 `reviewer_lanes` + `fork_context=false` 子代理证据时，**不得**仅凭 compact findings 清 `REVIEW_GATE`（须 qualifying PostTool/subagentStart 后再与 compact bump 配合）。Stop 满足走 **`independent_reviewer_seen`**（与 Claude 对齐）。 |
| **`fork_context` 缺省推断** | 统一 **`ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`**（unset=**关**）；legacy 宿主 env 仅显式 enable 时生效。 |
| **hook-state fail-closed** | Cursor review 武装路径与 Codex PostTool（wave-2 P1-4..）在锁不可用时 deny / `continue:false`；**非** review gate Stop 硬拦（review 缺口仍为 advisory nudge）。 |
| **Codex Stop 清门** | 与 Claude 对齐：`independent_reviewer_seen` 或 bounded **`rg_clear`** / reject token（**无** phase≥3 硬要求）。 |

## REVIEW_GATE 核心去宿主化（2026-06）

`review_gate.deep_gate_lanes` / `claude_reviewer_lanes` 已合并为单一 **`reviewer_lanes`**（Claude Code canonical 闭集）。lane 判定、fork 证据、Stop 满足规则统一在 **`core-policy`**（`review_gate_engine.rs`、`hook_common::is_reviewer_lane_normalized`）；各宿主 hook 仅保留 transport 差异（Stop **advisory** nudge、PostTool/subagentStart 观测路径；**不**硬拦 Stop）。维护：只改 `RUNTIME_REGISTRY.json` → `review_gate.reviewer_lanes`。

**细则**：[`.cursor/rules/review-subagent-gate.mdc`](.cursor/rules/review-subagent-gate.mdc)、[`docs/hosts/cursor.md`](docs/hosts/cursor.md)、[`docs/hosts/codex.md`](docs/hosts/codex.md)

## Cursor：hooks 减法闭集（2026-05-20）

本仓 [`.cursor/hooks.json`](.cursor/hooks.json) 默认仅 **7** 事件：`beforeSubmitPrompt`、`stop`、`sessionStart`、`sessionEnd`、`postToolUse`、`subagentStart`、`subagentStop`。

**已移除的注册**（`router-rs` handler 仍保留；**默认 dispatch 为 no-op**，不跑账本/rustfmt/compact 清门）：

| 事件 | 恢复代价 / 说明 |
|------|-----------------|
| `afterAgentResponse` | compact findings 可提前一轮清 `REVIEW_GATE`；默认改 `Stop` tail |
| `beforeShellExecution` / `afterShellExecution` | SessionEnd 终端 PID 账本更全；需 `ROUTER_RS_CURSOR_KILL_STALE_TERMINALS≠0` 才有意义 |
| `afterFileEdit` | Agent 改 `.rs` 后自动 `rustfmt` |
| `preCompact` | compaction 前 RFV/门状态摘要 |

手动把事件加回 `hooks.json` 后，dispatch **自动**走真实 handler（无需 env）。仅当**未**注册但仍想跑 handler（单测/对照）时：`ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS=1`（见 [`.cursor/router-rs-hook.env`](.cursor/router-rs-hook.env) 注释）。

**门控 `timeout`**：`beforeSubmitPrompt` / `stop` / `postToolUse` / `subagentStart` / `subagentStop` 均为 **20s**（`sessionStart` 5s、`sessionEnd` 15s）。`postToolUse` 超时会导致 review multiset / shell 账本不完整 — 见 [`docs/hosts/cursor.md`](docs/hosts/cursor.md)「对话中断排障」。

**模板同步**：[`configs/framework/cursor-hooks.workspace-template.json`](configs/framework/cursor-hooks.workspace-template.json) 须与 [`.cursor/hooks.json`](.cursor/hooks.json) 一致（`bash scripts/ci/check-cursor-hooks-parity.sh`；事件列表真源：`router-rs schema-drift contract` ↔ [`subtraction.rs`](core/host-projection/src/hosts/cursor_hooks/subtraction.rs)）。

**内存相关**：见 [`docs/hosts/cursor.md`](docs/hosts/cursor.md)「内存 / release」；项目 env [`.cursor/router-rs-hook.env`](.cursor/router-rs-hook.env)。

## Schema drift CLI（2026-05-20）

| 子命令 | 作用 |
|--------|------|
| `router-rs schema-drift contract` | 打印契约（7 必需 / 5 禁止事件、基线路径、嵌入 schema 版本） |
| `router-rs schema-drift baseline --repo-root …` | 捕获 `artifacts/current/<task_id>/SCHEMA_DRIFT_BASELINE.json` |
| `router-rs schema-drift check --repo-root …` | 对比基线；hooks parity、gate timeout、REQUIREMENTS↔ROADMAP 标题、`EVIDENCE_INDEX.artifacts[]` |

验收入口：[`skills/verifyx/SKILL.md`](skills/verifyx/SKILL.md)（verify 后 purge `artifacts/current/<task_id>/`）。CI 探针：`skill-ci` 跑 `schema-drift contract`。

## Claude Code：hook env 与 Cursor 对齐（2026-05-20）

Claude 宿主**本就**仅 4 个 hook 事件（`PreToolUse` / `UserPromptSubmit` / `PostToolUse` / `Stop`），无需删除 Cursor 侧已移除的 5 个事件。

| 项 | 路径 |
|----|------|
| 项目 env 真源 | [`.claude/router-rs-hook.env`](.claude/router-rs-hook.env) |
| 模板 / 新仓库复制 | [`configs/framework/claude-router-rs-hook.env`](configs/framework/claude-router-rs-hook.env) |
| Launcher | [`configs/framework/claude-router-rs-hook.sh`](configs/framework/claude-router-rs-hook.sh)（release 优先，与 Cursor 同序） |
| 重装 hooks 合并 | `framework host-integration install --to claude-code --scope project` |

默认 **`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`**（减 PostTool 证据写盘）。**不要**把 `ROUTER_RS_CURSOR_*` 写入 Claude env（无意义）。

## Harness framework hardening（2026-05-20）

| 变更 | 说明 |
|------|------|
| `review_gate` lane 集 | 磁盘 [`configs/framework/RUNTIME_REGISTRY.json`](configs/framework/RUNTIME_REGISTRY.json) + [`runtime_registry.rs`](core/framework-kernel/src/runtime_registry.rs)（re-export shim；**无** compile-time embed）；改 lane **无需** `cargo build`，重启 hook 子进程即可 |
| 宿主投影 My/review 文案 | [`configs/framework/host_projection_narrative.json`](configs/framework/host_projection_narrative.json)；`host-integration install` 读取；勿在 `host_integration.rs` 硬编码 |
| `generated-artifacts-status` | **`framework doctor`** / `--skip-generator-run` / `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS=1` → **metadata-only**（快）。**`update-one-shot`** 仍要求全量 **drift-gate** `ok: true` |
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1` | 全链路关闭审稿并**清除** `.cursor/hook-state` 内 review 字段；非「仅不 nag」 |
| active/focus GOAL 分裂 | 有 `continuity:active_goal_missing_focus_has_goal` 时 stdio/任务视图可能拒载错误 focus；用 `framework task-state-resolve` 或修正 `active_task.json`（**无** hook `GOAL_CONTINUE`） |
| Review soft-nag 超 cap | 超过 `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` 后 `followup_message` 降频；细节进 `additional_context`（**无** goal/RFV hook 续跑可合并） |
| `SKILL_ROUTING_RUNTIME.scope` | `hot_skill_count`/`full_skill_count` = 热表行数；`manifest_skill_count` = 全 manifest 行数 |
| 文档真源 | [`docs/spec.md`](docs/spec.md)、[`docs/hosts/`](docs/hosts/)、[`AGENTS.md`](AGENTS.md) |

## 文档与计划卫生（2026-05-20）

| 移除 | 替代真源 |
|------|----------|
| `docs/plans/*.md`（除 [`docs/plans/README.md`](docs/plans/README.md)） | GSD：`artifacts/current/<task_id>/ROADMAP.md`；Cursor Plan：活跃任务 `.cursor/plans/*.plan.md` |
| `docs/history/**` | git 历史；[`MIGRATION.md`](MIGRATION.md) |
| `configs/codex/docs/**` | [`docs/README.md`](docs/README.md)、宿主手册 [`docs/hosts/`](docs/hosts/) |
| `skills/autopilot/`、`skills/_archived/autopilot/`、`skills/legacy-gsd-ci-stub/`（**仅迁移对照**；磁盘路径已删） | `/implementx` + My 四命令 |

勿在 issue/评论中链接已删路径；契约以 [`docs/README.md`](docs/README.md) 索引为准。
