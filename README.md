# Skill System — 四宿主共用框架

这份仓库是一整套给 Claude、Codex、Cursor 和 OpenCode 共用的 skill 系统：包含 `skills/` 技能库、路由运行表、维护脚本、CI 校验和项目级 `AGENTS.md` 规则。把这个仓库通过 GitHub 分享给别人后，对方可以在 Windows 上克隆、验证，并按本机的宿主全局路径或工作区路径启用；不要依赖某台机器的绝对路径。

**使用者一页纸**（宿主差异、`REVIEW_GATE` 快查、真源阅读顺序、自检命令）：[`docs/hosts/`](docs/hosts/) + [`AGENTS.md`](AGENTS.md)。

**近期变更（2026-06）**：闭集宿主收敛为 **四宿主**（`codex`、`claude`、`cursor`、`opencode`）；`claude-desktop`、`codex-app`、`codex-cli` 已退役 — 见 [`MIGRATION.md`](MIGRATION.md)。历史变更详见 [`MIGRATION.md`](MIGRATION.md)。文档地图：[`docs/README.md`](docs/README.md)。**想贡献代码？** 见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 我该怎么入门（两条路径）

- **路径 A — 只消费 skill 路由（无 hook）**  
  适合：只想让 Codex/Cursor **按 `AGENTS.md` 查 [`skills/SKILL_ROUTING_RUNTIME.json`](skills/SKILL_ROUTING_RUNTIME.json) 再读命中 `skill_path`**，不需要连续性目录、门控或 `router-rs`。  
  最小准备：本仓库根、`AGENTS.md`、`skills/`；变更 skill 后运行 **`router-rs framework skills validate`**（可选 **`refresh --write`**）刷新路由产物（见下文「修改或新增 skill」）。**不要求**安装 `router-rs` hook 面或配置 `.cursor/hooks.json`。

- **路径 B — 全量 harness（router-rs + hooks）**  
  适合：需要 **Cursor/Codex/Claude hooks**、`.cursor/hook-state` 门控、连续性 `artifacts/current/`、证据索引等。必须先 **构建并安装 `router-rs`**，再按宿主配置 hooks；关键事件在二进制缺失时常 **fail-closed**（见下文 Codex hooks 解析顺序）。  
  **维护注意**：若修改根目录 `AGENTS.md` 且依赖 Codex 投影，改完后须重新 **`cargo build` + `router-rs framework sync-entrypoints --repo-root "$PWD"`**（首选；与 `codex sync --repo-root "$PWD"` 为同一实现之兼容别名）；策略正文以**编译期嵌入**形式进二进制。  
  Windows 首次全量验证见下文 **「第一次验证」**；装好后可在仓库根执行：  
  `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"` 做人读自检（生成物为 **metadata-only** 快探针；全量 drift 见 `framework maint update-one-shot`）。

**Office 文档阅读（PDF / DOCX / XLSX / PPTX）**：skill 路由默认 Rust-first，但 CLI **需单独安装**到 `~/.local/bin`（不随 `router-rs` 下发）。在仓库根执行 `bash scripts/install-pdf-tool.sh` 等，或 `just install-office-tools`。

## 这套系统包含什么

- `AGENTS.md`：四宿主（Claude、Codex、Cursor、OpenCode）进入本仓库时共同遵守的项目规则（含宿主行为差异附录）。
  - **维护**：若修改 `AGENTS.md` 且依赖 `router-rs` 生成的 Codex hook 投影，优先直接用本仓源码重新执行 `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"`（或与其实现相同的 `codex sync --repo-root "$PWD"`）；策略正文在二进制内为**编译期嵌入**，不要直接假设 PATH 里的 `router-rs` 已同步到最新构建。
- `docs/README.md`：文档索引（阅读顺序、主题表）。
- `docs/adr/010-ideal-architecture-v10.md`：六层架构规约、DAG、L4 拆分计划（当前权威架构文档）。
- `skills/`：全部 skill 源文件，每个 skill 通常在 `skills/<name>/SKILL.md`。
- `skills/SKILL_ROUTING_RUNTIME.json`：运行时路由入口。Codex 应先查这个文件，再按命中结果读取对应 skill。
- `skills/SKILL_MANIFEST.json`、`skills/SKILL_ROUTING_INDEX.md` 等：路由索引与 manifest（**热表** `SKILL_ROUTING_RUNTIME.json` 与 manifest 为手维护真源；`refresh --write-companions` 只再生 tiers/companion stubs）。
- `core/router-rs/`：`framework skills validate|refresh` 刷新 skill 路由产物。
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
git add AGENTS.md README.md skills core scripts tests Cargo.toml Cargo.lock .github .githooks docs
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

## 第一次验证（路径 B：全量 harness）

路径 A 用户可只跑 **`framework skills validate`** 与 `cargo test --test policy_contracts`（见上节）；本节面向路径 B（还要 hooks / `router-rs`）。

进入仓库后运行：

```powershell
cargo run --release --manifest-path core/router-rs/Cargo.toml -- `
  framework skills refresh --framework-root . --write
```

`--skills-root` 的父目录必须是含 `configs/framework/RUNTIME_REGISTRY.json` 的仓库根；`framework_command` 运行时行**只**从该 registry 生成，缺失文件会直接报错（无静默回退）。

再运行测试：

```powershell
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework skills validate --framework-root .
cargo test --test policy_contracts
```

如果上面都通过，说明 skill 编译器、路由产物和策略测试在她的 Windows 环境里可用。需要启用 hook 时，还要先构建 `router-rs`，因为 Codex/Cursor hook 都通过这个 Rust 二进制执行。

## 在 Codex / Cursor / Claude 中使用

各宿主的 hooks 安装、事件矩阵、fail-closed/fail-open 行为、跨仓库接入方式等完整说明见 [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md)。

**快速接入（路径 A 无 hook）**：只需本仓库根 + `AGENTS.md` + `skills/`，不需要安装 `router-rs`。

**全量 harness（路径 B 有 hook）**：必须先构建 `router-rs`（`cargo build --release --manifest-path core/router-rs/Cargo.toml`），再按 [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md) 对应宿主节配置 hooks。

**日常更新**：修改 skill 后运行 `router-rs framework skills refresh --write` + `router-rs framework sync-entrypoints --repo-root "$PWD"`。

## 日常更新方式

**全量维护（推荐，等同 `/update`）**：优先直接走框架源码入口；只有当 `router-rs framework --help` 明确出现 `maint` 时，才直接用已安装二进制。

```bash
export SKILL_FRAMEWORK_ROOT=/abs/path/to/framework-repo   # 或与 Cursor 单根一致的 CURSOR_WORKSPACE_ROOT
cargo run --release --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/core/router-rs/Cargo.toml" -- framework maint update-one-shot
```

```bash
router-rs framework maint update-one-shot
```

你更新 skill 后若只需最小验证，可拆步：

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework skills refresh --framework-root "$PWD" --write
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
cargo test --test policy_contracts
git status --short
git add skills core scripts tests AGENTS.md README.md
git commit -m "Update skill system"
git push
```

她同步更新：

```powershell
cd $HOME\Documents\codex-skill-system
git pull
cargo run --release --manifest-path core/router-rs/Cargo.toml -- `
  framework skills refresh --framework-root . --write
```

## 修改或新增 skill

与 [`skills/SKILL_MAINTENANCE_GUIDE.md`](skills/SKILL_MAINTENANCE_GUIDE.md) 一致；摘要如下：

1. 创建 `skills/<skill-name>/SKILL.md`（frontmatter + `## When to use` / `## Do not use`）。
2. **手改**热路由真源：`skills/SKILL_ROUTING_RUNTIME.json`、`skills/SKILL_MANIFEST.json`（slug、trigger、path 与 frontmatter 对齐）。
3. 再生 companion（**不**代替手改热表）：

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- \
  framework skills refresh --framework-root "$PWD" --write --write-companions
```

4. 验证：`framework skills validate`；`cargo test --test policy_contracts`。
5. 提交并推送。

**勿指望 `refresh` 写入 runtime/manifest**。下列多为 companion / 生成物，仅在校验失败或明确修生成器输出时手改：

- `skills/SKILL_ROUTING_INDEX.md`、`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`、`skills/SKILL_ROUTING_METADATA.json`
- `skills/SKILL_PLUGIN_CATALOG.json`、`skills/SKILL_HEALTH_MANIFEST.json`（健康分已退役，见 MAINTENANCE）
- `configs/framework/FRAMEWORK_SURFACE_POLICY.json`

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

## Hook integration quickstart

宿主 hook 的安装、事件矩阵与门控行为完整说明见 [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md)。

```bash
# Cursor hooks 自检
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-cursor-hooks

# Codex 用户级 hooks 安装
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint install-codex-user-hooks

# 全局安装 router-rs（推荐每台机器一次）
cargo install --path core/router-rs --locked --force
```
