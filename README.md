# Codex + Cursor Skill System Handoff Guide

这份仓库是一整套给 Codex 和 Cursor 共用的 skill 系统：包含 `skills/` 技能库、路由运行表、维护脚本、CI 校验和项目级 `AGENTS.md` 规则。把这个仓库通过 GitHub 分享给别人后，对方可以在 Windows 上克隆、验证，并按本机的 `CODEX_HOME` / `CURSOR_HOME` 或工作区路径启用；不要依赖某台机器的绝对路径。

**使用者一页纸**（宿主差异、`REVIEW_GATE` 快查、真源阅读顺序、自检命令）：[`docs/hosts/`](docs/hosts/) + [`AGENTS.md`](AGENTS.md)。

**近期变更（2026-06）**：闭集宿主收敛为 **四宿主**（`codex`、`claude-code`、`cursor`、`opencode`）；`claude-desktop`、`codex-app`、`codex-cli` 已退役 — 见 [`MIGRATION.md`](MIGRATION.md) §闭集宿主收敛（2026-06）。

**近期变更（2026-05）**：`/autopilot` 已退役（用 `/implementx`）；Cursor hooks 默认 **7 事件**减法闭集；`docs/plans/` 与 `docs/history/` 过期 stub 已移除（索引见 [`docs/plans/README.md`](docs/plans/README.md)）；控制面硬化（registry 磁盘 loader、`host_projection_narrative.json`、生成物 metadata-only doctor）见 [`MIGRATION.md`](MIGRATION.md) 与 [`docs/spec.md`](docs/spec.md)。文档地图：[`docs/README.md`](docs/README.md)。

## 我该怎么入门（两条路径）

- **路径 A — 只消费 skill 路由（无 hook）**  
  适合：只想让 Codex/Cursor **按 `AGENTS.md` 查 [`skills/SKILL_ROUTING_RUNTIME.json`](skills/SKILL_ROUTING_RUNTIME.json) 再读命中 `skill_path`**，不需要连续性目录、门控或 `router-rs`。  
  最小准备：本仓库根、`AGENTS.md`、`skills/`；变更 skill 后运行 **`router-rs framework skills validate`**（可选 **`refresh --write`**）刷新路由产物（见下文「修改或新增 skill」）。**不要求**安装 `router-rs` hook 面或配置 `.cursor/hooks.json`。

- **路径 B — 全量 harness（router-rs + hooks）**  
  适合：需要 **Cursor/Codex/Claude hooks**、`.cursor/hook-state` 门控、连续性 `artifacts/current/`、证据索引等。必须先 **构建并安装 `router-rs`**，再按宿主配置 hooks；关键事件在二进制缺失时常 **fail-closed**（见下文 Codex hooks 解析顺序）。  
  **维护注意**：若修改根目录 `AGENTS.md` 且依赖 Codex 投影，改完后须重新 **`cargo build` + `router-rs framework sync-entrypoints --repo-root "$PWD"`**（首选；与 `codex sync --repo-root "$PWD"` 为同一实现之兼容别名）；策略正文可能以**编译期嵌入**形式进二进制，详见 [`AGENTS_CODEX.md`](AGENTS_CODEX.md) → **Codex：`AGENTS.md` 构建快照（策略 A）**。  
  Windows 首次全量验证见下文 **「第一次验证」**；装好后可在仓库根执行：  
  `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"` 做人读自检（生成物为 **metadata-only** 快探针；全量 drift 见 `framework maint update-one-shot`）。

**Office 文档阅读（PDF / DOCX / XLSX / PPTX）**：skill 路由默认 Rust-first，但 CLI **需单独安装**到 `~/.local/bin`（不随 `router-rs` 下发）。在仓库根执行 `bash scripts/install-pdf-tool.sh` 等，或 `just install-office-tools`；详见 [`docs/references/office-document-clis.md`](docs/references/office-document-clis.md)。

## 这套系统包含什么

- `AGENTS.md`：Codex 和 Cursor 进入本仓库时共同遵守的项目规则。
  - **维护**：若修改 `AGENTS.md` 且依赖 `router-rs` 生成的 Codex hook 投影，优先直接用本仓源码重新执行 `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"`（或与其实现相同的 `codex sync --repo-root "$PWD"`）；策略正文在二进制内为**编译期嵌入**，不要直接假设 PATH 里的 `router-rs` 已同步到最新构建（见 [`AGENTS_CODEX.md`](AGENTS_CODEX.md) → **Codex：`AGENTS.md` 构建快照（策略 A）**）。
- `docs/README.md`：文档索引（阅读顺序、主题表）。
- `docs/spec.md`：统一规约（架构、五层模型、沙箱、路由、Closeout）。
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

## 在 Codex / Cursor 里使用

先让 Codex 打开这个仓库目录：

```powershell
cd $HOME\Documents\codex-skill-system
codex
```

或在 Codex 桌面版中选择这个文件夹作为工作区。

### Codex 侧（仓库级）

1. Codex 先读取根目录 `AGENTS.md`。
2. 仓库开发态先查 `skills/SKILL_ROUTING_RUNTIME.json`；全局安装态先查 `$CODEX_HOME/skills/SKILL_ROUTING_RUNTIME.json`。
3. 命中后只读取 runtime 记录里的 `skill_path` 对应文件。
4. 不要让 Codex 一次性预读整个 `skills/` 技能库。

可以用这句话测试是否生效：

```text
请根据本仓库 AGENTS.md 的规则，先查 skills/SKILL_ROUTING_RUNTIME.json，再选择合适 skill 回答：我想新增一个 Codex skill。
```

### Cursor 侧（工作区级）

- Cursor 规则来自 `.cursor/rules/`，对当前工作区（本仓库根目录）生效。
- Cursor hooks 来自 `.cursor/hooks.json`，对当前工作区会话生效，不是跨所有仓库的全局策略。
- 本仓库在 `.cursor/hooks.json` 注册 **7 个** hook 事件（减法闭集，见 [`docs/hosts/cursor.md`](docs/hosts/cursor.md)），经 [`configs/framework/cursor-router-rs-hook.sh`](configs/framework/cursor-router-rs-hook.sh) 调用 `router-rs cursor hook`；launcher **优先仓库 release**（~8MB），并 `source` [`.cursor/router-rs-hook.env`](.cursor/router-rs-hook.env)。关键门控 fail-closed；`postToolUse` 对 `Read` 等走 Rust fast-path。`.cursor/hook-state/` 存门控状态。
- 若使用 Codex CLI hooks，状态文件在 `.codex/hook-state/`，与 Cursor 独立。
- Codex `.codex/hooks.json` 包装脚本解析 `router-rs` 的顺序为：环境变量 **`ROUTER_RS_BIN`**（可执行绝对路径）→ 仓库 `core/router-rs/target/{release,debug}` → 仓库根 `target/{release,debug}` → **`command -v router-rs`**（最后手段；生产环境建议固定前两档之一）。缺少二进制时各生命周期事件一律 fail-closed（单行 JSON `decision:block`）。`.codex/hook-state/` 跨事件串联依赖 stdin 常见字段（`session_id` 等，含 camelCase）或 **`CODEX_SESSION_ID`** / **`CODEX_CONVERSATION_ID`**；需要硬前置时可设 **`ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY=1`**，在无稳定键时阻断 `UserPromptSubmit`/`PostToolUse`/`Stop`。
- 策略强度：Codex Stop 可 `decision: block`；Cursor 侧为 **followup_message / continue** 语义（见 `core/runtime-core/src/hosts/cursor_hooks/` 内 handlers），与 Codex 不完全相同。
- Cursor 技能分为两层：仓库路由技能走 `skills/`（由 `SKILL_ROUTING_RUNTIME.json` 管理）；用户侧/内置技能由 Cursor 自身加载（如 `~/.cursor/skills/` 与 `~/.cursor/skills-cursor/`），不写回本仓库 runtime。

**其它仓库一键接入（跨工作区）**

- 在目标项目根运行：`/path/to/skill/scripts/cursor-bootstrap-framework.sh --framework-root /path/to/skill`（或先 `export SKILL_FRAMEWORK_ROOT=/path/to/skill`）。若脚本不可执行，先：`chmod +x /path/to/skill/scripts/cursor-bootstrap-framework.sh`。
- 脚本写入 `.cursor/hooks.json`，模板真源为 `configs/framework/cursor-hooks.workspace-template.json`（通过 `configs/framework/cursor-router-rs-hook.sh` 探测 `router-rs`，`--repo-root` 用当前 Cursor 工作区）。
- 将 `skills/` 与 `AGENTS.md` 符号链接到框架仓库；需要与框架根目录等价的托管规则时加 `--with-cursor-rules`；需要与框架根目录共享 `configs/framework/*`（如 `HARNESS_OPERATOR_NUDGES.json`、`PAPER_ADVERSARIAL_HOOK.txt` 等磁盘真源）时加 **`--with-configs`**（否则相关 hooks 仍可用，但会回落到编译期内置默认，不等价于「改 JSON/txt 即生效」）。
- 安装二进制：`cargo install --path /path/to/skill/core/router-rs`；若可执行文件名不是默认，在环境里设 `ROUTER_RS_BIN`（hooks 内展开）。
- 与「本仓库 embedded」模式对照：本仓库 `.cursor/hooks.json` 与跨仓模板都走同一个 launcher；跨仓通常依赖 PATH / `ROUTER_RS_BIN` 或 `SKILL_FRAMEWORK_ROOT`。
- **`router-rs framework …` 维护命令**：在目标仓库目录执行时，若当前目录不是框架检出根，需设置 **`SKILL_FRAMEWORK_ROOT`**（或传 `--framework-root`），否则会报无法解析 `framework_root`（实现会尝试从已安装二进制路径、`CURSOR_WORKSPACE_ROOT` 等推断，不可靠时以环境变量为准）。
- Hook 减法与内存：[`docs/hosts/cursor.md`](docs/hosts/cursor.md)；恢复已删 Cursor 事件见 [`MIGRATION.md`](MIGRATION.md)。

### Claude Code（项目级 + 用户级）

- **My 生命周期**（与 Cursor 一致）：`/discussx` → `/planx` → `/implementx` → `/verifyx`；全局叙事在 **`~/.claude/rules/framework.md`**（对齐 `~/.cursor/rules/framework.mdc`）。
- Hooks：`.claude/settings.json` 合并 **4 事件** → [`claude-router-rs-hook.sh`](configs/framework/claude-router-rs-hook.sh)；env [`.claude/router-rs-hook.env`](.claude/router-rs-hook.env)。
- **安装（推荐）**：`./scripts/install-claude.sh`（project + user）。详见 [`docs/hosts/claude.md`](docs/hosts/claude.md)。
- **其它仓库**：`./scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"`，再 `install-claude.sh --scope user`。

**别的目录验收清单（Cursor 工作区 = 目标项目根）**

1. **PATH**：`which router-rs` 能解析到已安装的 `router-rs`（或 hooks 环境内 `ROUTER_RS_BIN` 指向绝对路径）。
2. **bootstrap**：已在目标根执行过上述脚本；`ls -l skills AGENTS.md .cursor/hooks.json` 显示 `skills`/`AGENTS.md` 为指向框架的符号链接，`hooks.json` 为普通文件（由模板复制）。
3. **可选符号链接**：按需存在 `.cursor/rules`、`configs` 分别指向框架（`--with-cursor-rules`、`--with-configs`）。
4. **打开方式**：在 Cursor 中「打开文件夹」选**目标项目根**（含 `.cursor/hooks.json` 的那一层），不要只打开子目录，否则可能找不到 hooks 或 `repo-root` 解析偏离预期。
5. **常见失败**：hooks 未触发（工作区根不对、或 `.cursor/hooks.json` 缺失）；`router-rs` 未安装或不在 PATH（关键门控事件 fail-closed，telemetry 事件 fail-open）；与 embedded 模式混用（目标仓仍手写 `.../target/release/router-rs` 但从未在该路径构建）。
6. **（可选）强制技能策略根**：仅在从子目录启动、且父级探测不符合预期时，设置 `CURSOR_PROJECT_ROOT` 或 `SKILL_REPO_ROOT` 指向含 `skills/SKILL_ROUTING_RUNTIME.json` 与 `AGENTS.md` 的目录（实现见 `core/runtime-core/src/skill_repo.rs`）。

**建议自检命令序列（可复制）**

```bash
# 0) 框架路径
export FW=/abs/path/to/skill   # 改成你的框架仓库根

# 1) 安装/确认 router-rs
command -v router-rs && router-rs --help | head -n 1
# 若未安装：cargo install --path "$FW/core/router-rs"
# 若 `router-rs framework --help` 看不到 `maint`，说明本机安装的二进制偏旧；
# 维护类命令请改用下文的 `cargo run --release --manifest-path ... -- framework maint ...`
# 或先重新安装/重建 router-rs。

# 2) 在「目标项目根」执行 bootstrap（按需加规则与 configs）
cd /abs/path/to/your-other-repo
"$FW/scripts/cursor-bootstrap-framework.sh" --framework-root "$FW" --with-cursor-rules --with-configs

# 3) JSON / 符号链接粗检
uv run python -m json.tool .cursor/hooks.json > /dev/null
test -L skills && test -L AGENTS.md && echo "symlinks ok"

# 4) 模拟 hook（stdin 空 JSON；repo-root 用目标根）
cd /abs/path/to/your-other-repo
printf '{}' | router-rs cursor hook --event=SessionStart --repo-root "$(pwd)"

# 5) 在「非框架 cwd」下跑维护类命令须显式走框架源码入口（示例）
cargo run --release --manifest-path "$FW/core/router-rs/Cargo.toml" -- framework maint verify-cursor-hooks
# 注意：上条校验的是框架仓 $FW 内的 .cursor/hooks.json（本仓库多为 embedded 路径）。
# 若要确认「目标仓」hooks 与跨仓模板一致：
cmp .cursor/hooks.json "$FW/configs/framework/cursor-hooks.workspace-template.json" && echo "hooks match workspace template"
```

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

此仓库使用 Rust `router-rs`（`core/router-rs`）承接 Codex/Cursor/Claude hooks、连续性扩展与 **`router-rs browser mcp-stdio`**。宿主编排以 **Rust 入口为真源**：`.cursor/hooks.json` 只经 `configs/framework/cursor-router-rs-hook.sh` 做二进制发现与 fail-open/fail-closed 分层，业务分支不得写进 shell。

### Cursor

`.cursor/hooks.json` 由 Cursor 自动读取；自检可用：

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-cursor-hooks
```

### Codex CLI

写入 `~/.codex/{config.toml,hooks.json}` 的用户级安装（替代已移除的 bash 包装脚本）：

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint install-codex-user-hooks
```

快速检查：

```bash
cargo build --release --manifest-path core/router-rs/Cargo.toml
cargo run --release --manifest-path core/router-rs/Cargo.toml -- codex install-hooks --check --codex-home "$HOME/.codex"
```

Global install (recommended once per machine):

```bash
cargo install --path core/router-rs --locked --force
# Or `router-rs self install` from any freshly built workspace binary（需已在 PATH）。
```

### Cross-host CLI cheatsheet

| Action | Cursor | Codex |
|---|---|---|
| Run review gate | `cursor hook --event=<event>` | `codex hook --event=<name>` (or positional) |
| Install user-level hooks | (none; in-repo) | `codex install-hooks --apply` |
