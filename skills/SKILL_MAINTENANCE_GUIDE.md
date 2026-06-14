# Skill 库维护约定

## 单一事实来源

- `skills/` 是唯一可写的 skill 源目录。默认维护、校验、生成都直接围绕仓库本身进行，不再把 `~/.claude/commands` 当成主路径前提。
- system skill 放 `skills/.system/`。不要同时保留两份 live source。
- `~/.claude/commands` 是 Claude Code 共用的轻量安装面，用于 CLI 可见的薄别名。**框架命令的真源在仓库内**：个人默认 **`discussx` / `planx` / `implementx` / `verifyx`**（legacy `/gsd-*` 已移除）。`team` → `skills/agent-swarm-orchestration/SKILL.md`。改 routing 后执行 `just publish` + `host-integration install`。治理与路由边界仍见 `skills/skill-framework-developer/SKILL.md`；不要在 `~/.claude/commands` 手修 canonical body。
- 不再维护 skill health 分。路由真源只保留 source manifest、skill frontmatter、generated manifest/runtime，以及真实回归用例；不要新增健康快照或把健康分写回 schema。
- 过期计划/历史文档（`docs/plans/*` stub、`docs/history/`、`configs/codex/docs/`）已删除；勿恢复为「第二真源」。索引见 [`docs/plans/README.md`](../docs/plans/README.md)、[`MIGRATION.md`](../MIGRATION.md)。

## 新增 Skill 最小清单

1. 创建 `skills/<skill-name>/SKILL.md`，frontmatter 必填：`name`, `description`, `routing_layer`, `routing_owner`, `routing_gate`, `session_start`
2. Body 必含：`## When to use` + `## Do not use`
3. **Manifest 新增字段**（SKILL_MANIFEST.json 位置索引 13–16）：

   | 字段 | 索引 | 类型 | 必填 | 说明 |
   |------|------|------|------|------|
   | `allowedTools` | 13 | `string[] \| null` | 否 | 该 skill 运行时允许使用的工具列表，如 `["Read","Bash","Agent"]`。`null` 表示不限制。 |
   | `model` | 14 | `string \| null` | 否 | 推荐运行模型。可选值：`haiku`（轻量路由/讨论）、`sonnet`（规划/中等任务）、`opus`（深度分析）。`null` 表示使用宿主默认模型。 |
   | `disableModelInvocations` | 15 | `boolean \| null` | 否 | 设为 `true` 禁止模型自动触发此 skill（仅允许用户显式调用）。`null` 等效于 `false`。 |
   | `context` | 16 | `string \| null` | 否 | 补充上下文说明，用于在路由匹配时为模型提供额外背景信息。一般留 `null`。 |

   填写原则：
   - 轻量路由/讨论类 skill（如 `discussx`）建议设 `model: "haiku"`
   - 规划类 skill（如 `planx`）建议设 `model: "sonnet"`
   - 需要严格工具管控的 skill（如 `code-review-deep`）显式列出 `allowedTools`
   - 大多数 skill 只需填 `allowedTools` 和 `model`，其余留 `null`
3. 更新手维护路由真源（**必须**）：
   - 编辑 `skills/SKILL_ROUTING_RUNTIME.json` 与 `skills/SKILL_MANIFEST.json`（slug、trigger、path 与 frontmatter 对齐）。
   - 运行 companion 再生（**不**改 runtime/manifest）：
   ```bash
   cargo run --manifest-path core/router-rs/Cargo.toml -- \
     framework skills refresh --framework-root "$PWD" --write --write-companions
   ```
   这一步刷新 `skills/SKILL_TIERS.json` 与 routing companion stubs（`SKILL_PLUGIN_CATALOG.json` 等）；**不要**指望 `refresh` 代替手改热表。
4. 运行验证：
   ```bash
   cargo run --manifest-path core/router-rs/Cargo.toml -- framework skills validate --framework-root "$PWD"
   cargo test --test policy_contracts
   ```
   本地人工执行这些高输出命令时，可按 [`RTK.md`](../RTK.md) 改用 `rtk ...` 包装形式。
5. 提交后 CI 自动验证（`.github/workflows/skill-ci.yml`）

## 改 Skill 必查

- 触发词是否变化 → 更新 description
- 边界是否变化 → 同步改 `SKILL_ROUTING_RUNTIME.json` / `SKILL_MANIFEST.json`，再 `framework skills refresh --write --write-companions`
- 是否引入第二份 live source → 删除多余副本
- 是否需要刷新 Claude Code 可见入口 → 运行 `cargo run --manifest-path core/router-rs/Cargo.toml -- codex host-integration install-skills --repo-root \"$PWD\" install`（或使用已安装的 `router-rs` 等价命令），不要手动改 `~/.claude/commands`

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

已提供 `.githooks/`（`pre-commit` 自动校验 + 评分，`post-commit` 仅在显式 opt-in 时自动 push）。首次安装：

Hooks are generated through the Rust host-entrypoint sync path.

默认不会在 commit 后自动 push。只有显式设置 `SKILL_SYNC_AUTO_PUSH=1` 时，`post-commit` 才会触发自动 push。

## Git 安全基线

这个仓库高频变更且可能同时存在多个 worktree。做清理、切分提交、rebase、stash 前，先运行：

```bash
git status --short --branch
git diff --stat
git worktree list --porcelain
```

如需 checkpoint，直接用 `git diff`、`git diff --staged` 和必要的手动备份写入 `artifacts/ops/`；不要依赖已移除的 Python git helper。
