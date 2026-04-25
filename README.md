# Codex Skill System Handoff Guide

这份仓库是一整套给 Codex 使用的 skill 系统：包含 `skills/` 技能库、路由运行表、维护脚本、CI 校验和项目级 `AGENTS.md` 规则。把这个仓库通过 GitHub 分享给别人后，对方可以在 Windows 上克隆下来，直接把它当作 Codex 的工作目录使用。

## 这套系统包含什么

- `AGENTS.md`：Codex 每次进入本仓库时最先遵守的项目规则。
- `skills/`：全部 skill 源文件，每个 skill 通常在 `skills/<name>/SKILL.md`。
- `skills/SKILL_ROUTING_RUNTIME.json`：运行时路由入口。Codex 应先查这个文件，再按命中结果读取对应 skill。
- `skills/SKILL_MANIFEST.json`、`skills/SKILL_ROUTING_INDEX.md`、`skills/SKILL_ROUTING_REGISTRY.md` 等：由编译器生成的路由/索引产物。
- `scripts/skill-compiler-rs/`：刷新 skill 路由产物的 Rust 工具。
- `tests/`：skill 策略和路由约束测试。
- `.github/workflows/`：GitHub 上的自动校验。

## 分享前你要做的事

先确认仓库里没有个人私密信息：

```bash
git status --short --branch
git diff --stat
git grep -n -I -E "OPENAI_API_KEY|api_key|secret|token|password|smtp|cookie|authorization|私钥|密码" -- .
```

建议不要分享这些本地状态文件或目录：

- `.codex/memory/`
- `.supervisor_state.json`
- `artifacts/`
- `output/`
- `archives/`
- `memory/*.db`
- `memory/*.sqlite3`
- 任何 `.env`、token、账号状态、运行日志

当前 `.gitignore` 已经忽略了大部分临时目录和状态文件，但分享前仍建议再检查一次 `git status --short`。如果你只通过 GitHub 分享，Git 只会上传已经纳入版本控制的文件。

## 上传到 GitHub

如果这是一个新仓库：

```bash
git init
git add AGENTS.md README.md skills scripts tests Cargo.toml Cargo.lock .github .githooks docs RTK.md
git commit -m "Share Codex skill system"
git branch -M main
git remote add origin https://github.com/<your-name>/<repo-name>.git
git push -u origin main
```

如果这个仓库已经有远端：

```bash
git status --short --branch
git add README.md
git commit -m "Add Windows handoff guide"
git push
```

如果你不想公开这套系统，请在 GitHub 创建 private repository，再邀请对方账号访问。

## Windows 用户安装准备

对方 Windows 机器建议安装：

1. Git for Windows: https://git-scm.com/download/win
2. Rust stable: https://rustup.rs/
3. Codex CLI 或 Codex 桌面版，按她当前使用的 Codex 安装方式完成登录。
4. 推荐使用 PowerShell 或 Windows Terminal。

安装后在 PowerShell 检查：

```powershell
git --version
rustc --version
cargo --version
codex --version
```

如果 `codex --version` 不可用，但她使用的是 Codex 桌面版，也可以直接用 Codex 打开这个仓库目录。

## Windows 上获取这套系统

在 PowerShell 中执行：

```powershell
cd $HOME\Documents
git clone https://github.com/<your-name>/<repo-name>.git codex-skill-system
cd codex-skill-system
```

如果是 private repository，她需要先登录 GitHub，或使用 GitHub Desktop / SSH key / personal access token 完成克隆。

## 第一次验证

进入仓库后运行：

```powershell
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- `
  --skills-root skills `
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json `
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json `
  --apply
```

再运行测试：

```powershell
cargo test --manifest-path scripts/skill-compiler-rs/Cargo.toml
cargo test --test policy_contracts
```

如果上面都通过，说明 skill 编译器、路由产物和策略测试在她的 Windows 环境里可用。

## 在 Codex 里使用

让 Codex 打开这个仓库目录：

```powershell
cd $HOME\Documents\codex-skill-system
codex
```

或在 Codex 桌面版中选择这个文件夹作为工作区。

使用时的核心规则是：

1. Codex 先读取根目录 `AGENTS.md`。
2. Codex 再查 `skills/SKILL_ROUTING_RUNTIME.json`。
3. 命中后只读取对应的 `skills/<name>/SKILL.md`。
4. 不要让 Codex 一次性预读整个 `skills/` 技能库。

可以用这句话测试是否生效：

```text
请根据本仓库 AGENTS.md 的规则，先查 skills/SKILL_ROUTING_RUNTIME.json，再选择合适 skill 回答：我想新增一个 Codex skill。
```

## 日常更新方式

你更新 skill 后：

```bash
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json \
  --apply
cargo test --test policy_contracts
git status --short
git add skills scripts tests AGENTS.md README.md
git commit -m "Update skill system"
git push
```

她同步更新：

```powershell
cd $HOME\Documents\codex-skill-system
git pull
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- `
  --skills-root skills `
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json `
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json `
  --apply
```

## 修改或新增 skill

新增 skill 的最小流程：

1. 创建 `skills/<skill-name>/SKILL.md`。
2. frontmatter 至少包含 `name`、`description`、`routing_layer`、`routing_owner`、`routing_gate`、`session_start`。
3. 正文至少包含 `## When to use` 和 `## Do not use`。
4. 运行 skill compiler 的 `--apply` 命令刷新路由产物。
5. 运行测试。
6. 提交并推送。

不要手动改这些生成文件，除非你明确知道自己在修编译器输出：

- `skills/SKILL_ROUTING_RUNTIME.json`
- `skills/SKILL_MANIFEST.json`
- `skills/SKILL_ROUTING_INDEX.md`
- `skills/SKILL_ROUTING_REGISTRY.md`
- `skills/SKILL_SHADOW_MAP.json`
- `skills/SKILL_LOADOUTS.json`
- `skills/SKILL_APPROVAL_POLICY.json`

## 可选：启用 Git Hooks

仓库里有 `.githooks/`，可以让提交前自动跑 skill 同步/校验。Windows PowerShell 中执行：

```powershell
git config core.hooksPath .githooks
```

如果遇到 shell 兼容问题，可以先不启用 hooks，手动运行上面的验证命令即可。

## 常见问题

### PowerShell 里的换行符怎么写？

PowerShell 用反引号 `` ` `` 续行；Git Bash 用反斜杠 `\` 续行。README 里 Windows 命令默认写 PowerShell 版本。

### Rust 编译很慢怎么办？

第一次运行 `cargo` 会下载依赖和编译，慢是正常的。后续会复用缓存。

### Codex 没有按 skill 路由怎么办？

先确认 Codex 的工作目录就是这个仓库根目录，并把下面这句话发给 Codex：

```text
请遵守本仓库 AGENTS.md：先查 skills/SKILL_ROUTING_RUNTIME.json，命中后只读对应 skills/<name>/SKILL.md。
```

### 可以只复制 `skills/` 吗？

不推荐。`skills/` 是核心，但完整系统还包括 `AGENTS.md`、编译器、测试、CI 和维护约定。通过 GitHub 克隆整个仓库最稳。

