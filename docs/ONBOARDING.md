# 安装与入门教程

本文档从 `README.md` 拆分而来，覆盖详细的安装步骤、宿主配置、日常更新和常见问题。

**快速入口**：[README.md](../README.md) · [运维手册](operations/index.md) · [框架操作手册](framework_operator_primer.md) · [文档索引](README.md)

---

## 分享前你要做的事

先确认仓库里没有个人私密信息：

```bash
git status --short --branch
git diff --stat
git grep -n -I -E "OPENAI_API_KEY|api_key|secret|token|password|smtp|cookie|authorization|私钥|密码" -- .
```

建议不要分享这些本地状态文件或目录：

- `.supervisor_state.json`
- `artifacts/`
- `output/`
- `archives/`
- 任何 `.env`、token、账号状态、运行日志

当前 `.gitignore` 已经忽略了大部分临时目录和状态文件，但分享前仍建议再检查一次 `git status --short`。如果你只通过 GitHub 分享，Git 只会上传已经纳入版本控制的文件。

## 上传到 GitHub

如果这是一个新仓库：

```bash
git init
git add AGENTS.md README.md skills core scripts tests Cargo.toml Cargo.lock .github .githooks docs RTK.md
git commit -m "Share Codex skill system"
git branch -M main
git remote add origin https://github.com/<your-name>/<repo-name>.git
git push -u origin main
```

如果这个仓库已经有远端：

```bash
git status --short --branch
git add README.md
git commit -m "Update onboarding docs"
git push
```

如果你不想公开这套系统，请在 GitHub 创建 private repository，再邀请对方账号访问。

## 首次安装

> **详细运维**：完整安装矩阵见 [`operations/getting-started.md`](operations/getting-started.md)，MCP 占位符规则见 [`operations/index.md`](operations/index.md) §MCP 配置占位符规则。

```bash
# 1. 克隆仓库
git clone <repo-url> && cd skill

# 2. 构建 router-rs（所有宿主依赖）
CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
  cargo build --release --manifest-path core/router-rs/Cargo.toml

# 3. 安装目标宿主（选择一个）
#    Claude Code：./scripts/install-claude.sh
#    其他宿主：见 operations/index.md 速查卡
#    全部宿主：./scripts/install-all-hosts.sh

# 4. 健康检查
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration status
```

### 第一次验证

```bash
# Skill 编译器验证
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework skills refresh --framework-root "$PWD" --write

# Skill 路由验证
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework skills validate --framework-root "$PWD"

# 策略测试
cargo test --test policy_contracts
```

如果上面都通过，说明 skill 编译器、路由产物和策略测试在你的环境中可用。

## 在宿主中使用

### Skill 路由（所有宿主通用）

1. 宿主先读取根目录 `AGENTS.md`。
2. 查询 `skills/SKILL_ROUTING_RUNTIME.json`（热路由）。
3. 命中后只读取 runtime 记录里的 `skill_path` 对应文件。
4. 不要一次性预读整个 `skills/` 技能库。

### 宿主专项配置

| 宿主 | 配置说明 | 详细手册 |
|------|----------|----------|
| **Claude Code** | `.claude/settings.json` hook 绑定 + `~/.claude/rules/framework.md` 叙事 | [`docs/hosts/claude.md`](hosts/claude.md) |
| **Cursor** | `.cursor/hooks.json` 7 事件 + `~/.cursor/rules/framework.mdc` | [`docs/hosts/cursor.md`](hosts/cursor.md) |
| **Codex CLI** | `.codex/hooks.json` 4 事件 + `AGENTS_CODEX.md` | [`docs/hosts/codex.md`](hosts/codex.md) |
| **OpenCode** | `.opencode/` MCP 配置（纯 MCP，无 shell hook） | [`docs/hosts/opencode.md`](hosts/opencode.md) |
| **Antigravity** | `.gemini/` MCP 配置（纯 MCP，无 shell hook） | [`docs/hosts/antigravity.md`](hosts/antigravity.md) |

**其它仓库一键接入**：

```bash
# Claude Code
./scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
./scripts/install-claude.sh --scope user

# Cursor
./scripts/cursor-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"

# 全部宿主
./scripts/install-all-hosts.sh
```

## 日常更新方式

**全量维护（推荐）**：

```bash
router-rs framework maint update-one-shot
```

最小验证：

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework skills refresh --framework-root "$PWD" --write
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
cargo test --test policy_contracts
git status --short
git add skills core scripts tests AGENTS.md README.md
git commit -m "Update skill system"
git push
```

## 修改或新增 skill

与 [`skills/SKILL_MAINTENANCE_GUIDE.md`](../skills/SKILL_MAINTENANCE_GUIDE.md) 一致；摘要如下：

1. 创建 `skills/<skill-name>/SKILL.md`（frontmatter + `## When to use` / `## Do not use`）。
2. **手改**热路由真源：`skills/SKILL_ROUTING_RUNTIME.json`、`skills/SKILL_MANIFEST.json`。
3. 再生 companion（**不**代替手改热表）：

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- \
  framework skills refresh --framework-root "$PWD" --write --write-companions
```

4. 验证：`framework skills validate`；`cargo test --test policy_contracts`。
5. 提交并推送。

## 可选：启用 Git Hooks

```bash
git config core.hooksPath .githooks
```

## 常见问题

### Rust 编译很慢怎么办？

第一次运行 `cargo` 会下载依赖和编译，慢是正常的。后续会复用缓存。

### Skill 没有按路由触发怎么办？

先确认宿主的工作目录就是仓库根目录，然后确认 `skills/SKILL_ROUTING_RUNTIME.json` 存在且非空。

### 可以只复制 `skills/` 吗？

不推荐。`skills/` 是核心，但完整系统还包括 `AGENTS.md`、编译器、测试、CI 和维护约定。通过 GitHub 克隆整个仓库最稳。
