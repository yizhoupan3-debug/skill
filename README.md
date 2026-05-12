# Codex + Cursor Skill System Handoff Guide

这份仓库是一整套给 Codex 和 Cursor 共用的 skill 系统：包含 `skills/` 技能库、路由运行表、维护脚本、CI 校验和项目级 `AGENTS.md` 规则。把这个仓库通过 GitHub 分享给别人后，对方可以在 Windows 上克隆、验证，并按本机的 `CODEX_HOME` / `CURSOR_HOME` 或工作区路径启用；不要依赖某台机器的绝对路径。

## 这套系统包含什么

- `AGENTS.md`：Codex 和 Cursor 进入本仓库时共同遵守的项目规则。
  - **维护**：若修改 `AGENTS.md` 且依赖 `router-rs` 生成的 Codex hook 投影，优先直接用本仓源码重新执行 `cargo run --manifest-path scripts/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"`；策略正文在二进制内为**编译期嵌入**，不要直接假设 PATH 里的 `router-rs` 已同步到最新构建（见 `AGENTS.md` → **权威分层** → **Codex：`AGENTS.md` 构建快照（策略 A）**）。
- `docs/README.md`：契约与分层文档索引（阅读顺序、主题表、`target-dir`/hook 清理边界）。
- `docs/harness_architecture.md`：连续性控制面 **L1–L5** 上层设计（证据流、续跑流、扩展规则）。
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
  --apply
```

再运行测试：

```powershell
cargo test --manifest-path scripts/skill-compiler-rs/Cargo.toml
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
- 本仓库在 `.cursor/hooks.json` 通过 `configs/framework/cursor-router-rs-hook.sh` 调用 `router-rs cursor hook --event=…`。launcher 只做 release/debug/PATH 探测和缺 binary 策略：关键门控事件 fail-closed，session/format/telemetry 类事件 fail-open；业务语义仍全部在 Rust hook 内。`.cursor/hook-state/` 存门控临时状态。
- 若使用 Codex CLI hooks，状态文件在 `.codex/hook-state/`，与 Cursor 独立。
- Codex `.codex/hooks.json` 包装脚本解析 `router-rs` 的顺序为：环境变量 **`ROUTER_RS_BIN`**（可执行绝对路径）→ 仓库 `scripts/router-rs/target/{release,debug}` → 仓库根 `target/{release,debug}` → **`command -v router-rs`**（最后手段；生产环境建议固定前两档之一）。缺少二进制时各生命周期事件一律 fail-closed（单行 JSON `decision:block`）。`.codex/hook-state/` 跨事件串联依赖 stdin 常见字段（`session_id` 等，含 camelCase）或 **`CODEX_SESSION_ID`** / **`CODEX_CONVERSATION_ID`**；需要硬前置时可设 **`ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY=1`**，在无稳定键时阻断 `UserPromptSubmit`/`PostToolUse`/`Stop`（详见 `docs/harness_architecture.md` 环境变量表）。
- 策略强度：Codex Stop 可 `decision: block`；Cursor 侧为 **followup_message / continue** 语义（见 `scripts/router-rs/src/cursor_hooks/` 内 `dispatch.rs`/handlers），与 Codex 不完全相同。
- Cursor 技能分为两层：仓库路由技能走 `skills/`（由 `SKILL_ROUTING_RUNTIME.json` 管理）；用户侧/内置技能由 Cursor 自身加载（如 `~/.cursor/skills/` 与 `~/.cursor/skills-cursor/`），不写回本仓库 runtime。

**其它仓库一键接入（跨工作区）**

- 在目标项目根运行：`/path/to/skill/scripts/cursor-bootstrap-framework.sh --framework-root /path/to/skill`（或先 `export SKILL_FRAMEWORK_ROOT=/path/to/skill`）。若脚本不可执行，先：`chmod +x /path/to/skill/scripts/cursor-bootstrap-framework.sh`。
- 脚本写入 `.cursor/hooks.json`，模板真源为 `configs/framework/cursor-hooks.workspace-template.json`（通过 `configs/framework/cursor-router-rs-hook.sh` 探测 `router-rs`，`--repo-root` 用当前 Cursor 工作区）。
- 将 `skills/` 与 `AGENTS.md` 符号链接到框架仓库；需要与框架根目录等价的托管规则时加 `--with-cursor-rules`；需要与框架根目录共享 `configs/framework/*`（如 `HARNESS_OPERATOR_NUDGES.json`、`PAPER_ADVERSARIAL_HOOK.txt` 等磁盘真源）时加 **`--with-configs`**（否则相关 hooks 仍可用，但会回落到编译期内置默认，不等价于「改 JSON/txt 即生效」）。
- 安装二进制：`cargo install --path /path/to/skill/scripts/router-rs`；若可执行文件名不是默认，在环境里设 `ROUTER_RS_BIN`（hooks 内展开）。
- 与「本仓库 embedded」模式对照：本仓库 `.cursor/hooks.json` 与跨仓模板都走同一个 launcher；跨仓通常依赖 PATH / `ROUTER_RS_BIN` 或 `SKILL_FRAMEWORK_ROOT`。
- **`router-rs framework …` 维护命令**：在目标仓库目录执行时，若当前目录不是框架检出根，需设置 **`SKILL_FRAMEWORK_ROOT`**（或传 `--framework-root`），否则会报无法解析 `framework_root`（实现会尝试从已安装二进制路径、`CURSOR_WORKSPACE_ROOT` 等推断，不可靠时以环境变量为准）。
- 科研向 skill、hook 真源与跨工作区核对清单索引：`docs/plans/research_skills_hooks_survey.md`、`docs/plans/cursor_cross_workspace_operator_checklist.md`。

**别的目录验收清单（Cursor 工作区 = 目标项目根）**

1. **PATH**：`which router-rs` 能解析到已安装的 `router-rs`（或 hooks 环境内 `ROUTER_RS_BIN` 指向绝对路径）。
2. **bootstrap**：已在目标根执行过上述脚本；`ls -l skills AGENTS.md .cursor/hooks.json` 显示 `skills`/`AGENTS.md` 为指向框架的符号链接，`hooks.json` 为普通文件（由模板复制）。
3. **可选符号链接**：按需存在 `.cursor/rules`、`configs` 分别指向框架（`--with-cursor-rules`、`--with-configs`）。
4. **打开方式**：在 Cursor 中「打开文件夹」选**目标项目根**（含 `.cursor/hooks.json` 的那一层），不要只打开子目录，否则可能找不到 hooks 或 `repo-root` 解析偏离预期。
5. **常见失败**：hooks 未触发（工作区根不对、或 `.cursor/hooks.json` 缺失）；`router-rs` 未安装或不在 PATH（关键门控事件 fail-closed，telemetry 事件 fail-open）；与 embedded 模式混用（目标仓仍手写 `.../target/release/router-rs` 但从未在该路径构建）。
6. **（可选）强制技能策略根**：仅在从子目录启动、且父级探测不符合预期时，设置 `CURSOR_PROJECT_ROOT` 或 `SKILL_REPO_ROOT` 指向含 `skills/SKILL_ROUTING_RUNTIME.json` 与 `AGENTS.md` 的目录（实现见 `scripts/router-rs/src/skill_repo.rs`）。

**建议自检命令序列（可复制）**

```bash
# 0) 框架路径
export FW=/abs/path/to/skill   # 改成你的框架仓库根

# 1) 安装/确认 router-rs
command -v router-rs && router-rs --help | head -n 1
# 若未安装：cargo install --path "$FW/scripts/router-rs"
# 若 `router-rs framework --help` 看不到 `maint`，说明本机安装的二进制偏旧；
# 维护类命令请改用下文的 `cargo run --manifest-path ... -- framework maint ...`
# 或先重新安装/重建 router-rs。

# 2) 在「目标项目根」执行 bootstrap（按需加规则与 configs）
cd /abs/path/to/your-other-repo
"$FW/scripts/cursor-bootstrap-framework.sh" --framework-root "$FW" --with-cursor-rules --with-configs

# 3) JSON / 符号链接粗检
python3 -m json.tool .cursor/hooks.json > /dev/null
test -L skills && test -L AGENTS.md && echo "symlinks ok"

# 4) 模拟 hook（stdin 空 JSON；repo-root 用目标根）
cd /abs/path/to/your-other-repo
printf '{}' | router-rs cursor hook --event=SessionStart --repo-root "$(pwd)"

# 5) 在「非框架 cwd」下跑维护类命令须显式走框架源码入口（示例）
cargo run --manifest-path "$FW/scripts/router-rs/Cargo.toml" -- framework maint verify-cursor-hooks
# 注意：上条校验的是框架仓 $FW 内的 .cursor/hooks.json（本仓库多为 embedded 路径）。
# 若要确认「目标仓」hooks 与跨仓模板一致：
cmp .cursor/hooks.json "$FW/configs/framework/cursor-hooks.workspace-template.json" && echo "hooks match workspace template"
```

## 日常更新方式

**全量维护（推荐，等同 `/update`）**：优先直接走框架源码入口；只有当 `router-rs framework --help` 明确出现 `maint` 时，才直接用已安装二进制。

```bash
export SKILL_FRAMEWORK_ROOT=/abs/path/to/framework-repo   # 或与 Cursor 单根一致的 CURSOR_WORKSPACE_ROOT
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint update-one-shot
```

```bash
router-rs framework maint update-one-shot
```

你更新 skill 后若只需最小验证，可拆步：

```bash
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
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
- `skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`
- `skills/SKILL_ROUTING_METADATA.json`
- `skills/SKILL_PLUGIN_CATALOG.json`
- `skills/SKILL_HEALTH_MANIFEST.json`
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

此仓库使用 Rust `router-rs`（`scripts/router-rs`）承接 Codex/Cursor/Claude hooks、连续性扩展与 **`router-rs browser mcp-stdio`**。宿主编排以 **Rust 入口为真源**：`.cursor/hooks.json` 只经 `configs/framework/cursor-router-rs-hook.sh` 做二进制发现与 fail-open/fail-closed 分层，业务分支不得写进 shell。

### Cursor

`.cursor/hooks.json` 由 Cursor 自动读取；自检可用：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint verify-cursor-hooks
```

### Codex CLI

写入 `~/.codex/{config.toml,hooks.json}` 的用户级安装（替代已移除的 bash 包装脚本）：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint install-codex-user-hooks
```

快速检查：

```bash
cargo build --release --manifest-path scripts/router-rs/Cargo.toml
cargo run --release --manifest-path scripts/router-rs/Cargo.toml -- codex install-hooks --check --codex-home "$HOME/.codex"
```

Global install (recommended once per machine):

```bash
cargo install --path scripts/router-rs --locked --force
# Or `router-rs self install` from any freshly built workspace binary（需已在 PATH）。
```

### Cross-host CLI cheatsheet

| Action | Cursor | Codex |
|---|---|---|
| Run review gate | `cursor hook --event=<event>` | `codex hook --event=<name>` (or positional) |
| Install user-level hooks | (none; in-repo) | `codex install-hooks --apply` |
