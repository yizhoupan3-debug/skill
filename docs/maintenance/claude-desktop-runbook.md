---
last_verified: "2026-06-02"
scope: all-hosts
depends_on:
  - ../hosts/claude-desktop.md
  - ../hosts/claude-desktop-networking.md
  - ../hosts/claude.md
  - ../hosts/cursor.md
  - ../hosts/codex-cli.md
  - ../hosts/opencode.md
  - ../hosts/antigravity.md
  - ../hosts/antigravity-app.md
  - ../hosts/antigravity-cli.md
  - ../host_adapter_contract.md
---

# 统一运维手册

**适用范围**：全部 7 个宿主的安装、维护、故障恢复、安全配置。本文是运维操作的**唯一真源**，各宿主操作手册保留代理行为规范和 hook 契约细节。

---

## 一、快速参考卡

### 通用命令

| 场景 | 命令 |
|------|------|
| 健康检查 | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status` |
| 诊断全量 | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor` |
| 构建 release 二进制 | `CARGO_TARGET_DIR="$PWD/core/router-rs/target" cargo build --release --manifest-path core/router-rs/Cargo.toml` |
| SSRF 防护测试 | `cargo test --manifest-path core/router-rs/Cargo.toml -- web_fetch_guard` |

### 按宿主速查

| 宿主 | 安装命令 | 同步命令 |
|------|----------|----------|
| **Claude Desktop** | `./scripts/install-claude-desktop.sh` | 同左（含 egress 补丁） |
| **Claude Code** | `./scripts/install-claude.sh` | 同左 |
| **Cursor** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to cursor --scope user` | 同左 |
| **Codex CLI** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"` | 同左 |
| **OpenCode** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to opencode --repo-root "$PWD"` | 同左 |
| **Antigravity App** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to antigravity-app --repo-root "$PWD"` | 同左 |
| **Antigravity CLI** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to antigravity-cli --repo-root "$PWD"` | 同左 |

### Claude Desktop 专项

| 场景 | 命令 |
|------|------|
| Cowork 出站修复 | `./scripts/patch-claude-desktop-3p-cowork-egress.sh` |
| 权限持久化（bypass） | `HOME="$HOME" ./scripts/patch-claude-desktop-permission-mode.sh` |
| 权限持久化（acceptEdits） | `HOME="$HOME" ./scripts/patch-claude-desktop-permission-mode.sh acceptEdits` |
| 日志查看 | `tail -f ~/Library/Logs/Claude-3p/main.log` |

---

## 二、宿主概览与架构

### 宿主清单

| 宿主 ID | 传输方式 | Hook 事件数 | 配置根 | MCP 支持 |
|---------|---------|-------------|--------|----------|
| `claude-desktop` | MCP stdio | 0（纯 MCP） | `.claude/` + `~/Library/Application Support/Claude-3p/` | 是（router-rs-framework + browser-mcp） |
| `claude-code` | claude-hooks | 4 | `.claude/` + `~/.claude/` | 否（hook 级集成） |
| `cursor` | cursor-hooks | 7 | `.cursor/` + `~/.cursor/` | 否（hook 级集成） |
| `codex-cli` | codex-hooks | 4 | `.codex/` | 否（hook 级集成） |
| `opencode` | opencode-native | 0（纯 MCP） | `.opencode/` + `~/.config/opencode/` | 是 |
| `antigravity-app` | MCP stdio | 0（纯 MCP） | `.gemini/` | 是 |
| `antigravity-cli` | JSON hooks | 5 | `.antigravitycli/` | 否 |

### 共同架构

所有宿主共享：
- **Skill 路由**：`skills/SKILL_ROUTING_RUNTIME.json`（热路由）+ `skills/SKILL_MANIFEST.json`（冷表）
- **任务物化**：`artifacts/current/<task_id>/GOAL_STATE.json`
- **生命周期**：Discuss -> Plan -> Implement -> Verify（`/discussx` -> `/planx` -> `/implementx` -> `/verifyx`）
- **默认 profile**：`my-light`（advisory closeout，不硬拦 Stop）
- **Python 环境**：uv-only，Python 3.12，`uv.lock` 锁存

---

## 三、跨宿主通用运维

### 3.1 首次安装（通用流程）

```bash
# 1. 克隆仓库
git clone <repo-url> && cd skill

# 2. 构建 router-rs（所有宿主依赖）
CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
  cargo build --release --manifest-path core/router-rs/Cargo.toml

# 3. 安装目标宿主（见速查卡选择命令）

# 4. 健康检查
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration status
```

### 3.2 版本升级

```bash
# 1. 拉取最新代码
git pull

# 2. 重新构建
CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
  cargo build --release --manifest-path core/router-rs/Cargo.toml

# 3. 重跑目标宿主的安装/同步命令（见速查卡）
# 4. 健康检查
```

### 3.3 跨项目引导

将框架注入到其他项目：

```bash
# Claude Code / Desktop
./scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
./scripts/install-claude.sh --scope user

# Cursor
./scripts/cursor-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
```

### 3.4 自检诊断

```bash
# 全量诊断（含 metadata-only artifacts-status、flock 检查）
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor

# Cursor hooks 校验
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-cursor-hooks

# 集成测试
cargo test --manifest-path core/router-rs/Cargo.toml host_integration
```

---

## 四、Claude Desktop 专项运维

### 4.1 CC Switch 重配后恢复

CC Switch 会重导出配置并清除自定义字段（`coworkEgressAllowedHosts`、MCP 注册等）。

**必须完全退出 Desktop（Cmd+Q）后再执行**：

```bash
# 1. 安装（MCP 投影 + 二进制 + egress 补丁）
./scripts/install-claude-desktop.sh

# 2. 权限补丁（Desktop 退出后执行）
HOME="$HOME" ./scripts/patch-claude-desktop-permission-mode.sh

# 3. 重开 Desktop
```

**验证清单**：
- [ ] Connectors 面板：`router-rs-framework` + `browser-mcp` 已连接
- [ ] Chat 标签：`web_fetch` MCP 可用
- [ ] Cowork 标签：`browser-mcp` 可访问外网（无 `cowork-egress-blocked`）
- [ ] 权限模式：输入框旁显示 `Bypass permissions`

### 4.2 联网架构

| 标签 | 外网查询顺序 | 禁用 |
|------|-------------|------|
| **Chat** | MCP `web_fetch` -> `browser-mcp` -> 宿主 WebFetch | Bash `curl` |
| **Cowork** | **仅 `browser-mcp`** | `WebSearch`（429）、`mcp__workspace__web_fetch`（易 reset） |

**3P Cowork egress**：
- 默认仅允许 inference gateway（`127.0.0.1`），外网被阻断
- 补丁脚本：`./scripts/patch-claude-desktop-3p-cowork-egress.sh`（写入 `coworkEgressAllowedHosts: ["*"]`）
- 配置路径：`~/Library/Application Support/Claude-3p/configLibrary/<appliedId>.json`
- CC Switch 重配后需重跑

**联网自检话术**：

Cowork：
```
联网测试：依次试 WebSearch、mcp__workspace__web_fetch https://example.com、browser-mcp 打开 https://www.google.com 并简述页面标题。只报告每项成败。
```

Chat：
```
联网测试：依次试 web_fetch https://example.com、browser-mcp 打开 https://www.google.com。只报告每项成败。
```

### 4.3 权限模式管理

Desktop 不读 CLI 的 `permissions.defaultMode`。实际来源两层：

| 层 | 路径 | 作用 |
|----|------|------|
| JSON 偏好 | `Claude-3p/claude_desktop_config.json` -> `preferences.epitaxyPrefs` | 默认模式 |
| Electron LevelDB | `Claude-3p/Local Storage/leveldb/` | **运行时真源**（覆盖 JSON） |

**模式对照**：

| 模式 | Allow/Deny | 适用场景 |
|------|------------|----------|
| `bypassPermissions` | 基本不弹 | 个人 VM（推荐） |
| `acceptEdits` | 读写自动，MCP 可能仍弹 | 折中 |
| `auto` | 自动决策 | 通用 |
| `plan` | 只读自动，写入需确认 | 审慎 |
| `default` | 频繁弹窗 | Claude Code 默认 |

**切换**：退出 Desktop -> 运行权限脚本 -> 重开 -> 新开 Cowork 验证。

**LevelDB 故障恢复**：

| 症状 | 处理 |
|------|------|
| exit code 2（locked） | Desktop 未退出 -> 完全退出后重跑 |
| LevelDB 损坏（断电等） | 关闭 Desktop -> 删除 `leveldb/` 目录 -> 重开 -> 重跑权限脚本 |
| JSON 与 LevelDB 不同步 | 运行权限脚本（同时写两层） |

### 4.4 故障排查

| 现象 | 处理 |
|------|------|
| MCP 断开 | `./scripts/install-claude-desktop.sh` -> 重启 Desktop |
| `cowork-egress-blocked` | `./scripts/patch-claude-desktop-3p-cowork-egress.sh` -> 重启 Desktop |
| `SSRF_BLOCKED` | browser_open 目标为内网或非 http(s)，改用公网 URL |
| 频繁 Allow/Deny | Cmd+Q -> 权限脚本 -> 新开 Cowork |
| WebSearch 429 | MiniMax/gateway 限流，改用 browser-mcp |
| PostTool 卡 ~20s | 门控争用；检查 hook-state 体积与锁 |

---

## 五、Claude Code 专项运维

### 5.1 Hook 事件

4 事件减法闭集：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`。

配置文件：
- `.claude/settings.json` — hook 绑定
- `.claude/router-rs-hook.env` — 项目环境变量

### 5.2 故障排查

| 现象 | 处理 |
|------|------|
| Stop 后任务未完成 | `/implementx` + `framework_goal_drive` stdio |
| hook-state 不可读 | `.claude/hook-state/` 文件损坏 -> 删除后重开会话 |
| REVIEW_GATE 硬拦 | my-light 不应触发；非 my-light 用 `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE=1` |
| Paper prose hook 干扰 | `ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK=0` 关闭 |

---

## 六、Cursor 专项运维

### 6.1 Hook 事件

7 事件减法闭集：`beforeSubmitPrompt`、`stop`、`sessionStart`、`sessionEnd`、`postToolUse`、`subagentStart`、`subagentStop`。

配置文件：
- `.cursor/hooks.json` — hook 绑定（project）
- `.cursor/router-rs-hook.env` — 环境变量
- `~/.cursor/rules/framework.mdc` — 框架叙事（user）

### 6.2 故障排查

| 现象 | 处理 |
|------|------|
| Stop 后出现 REVIEW_GATE | 非 my-light 或 review 未清门 -> `rg_clear` 或 `ROUTER_RS_CURSOR_REVIEW_GATE_MODE=lite` |
| 子代理 `permission: deny` | 重复 `subagentStart` 或 session 分片 -> 检查 `active_subagent_count` |
| beforeSubmit 无法继续 | hook-state 锁/持久化失败 -> 检查 `.cursor/hook-state/` 权限 |
| PostTool 卡 ~20s | L1/L3 争用 -> 提升 gate timeout（`.cursor/hooks.json`） |
| 双聊天互相影响 | 设 `ROUTER_RS_CURSOR_SESSION_NAMESPACE` |
| router-rs 缺失 | critical 事件 fail-closed -> 确保二进制存在或 `ROUTER_RS_BIN` 指向 |
| Hooks 校验 | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-cursor-hooks` |

### 6.3 性能基准

```bash
# Hook 延迟基准测试
./scripts/bench-hooks.sh
```

---

## 七、Codex CLI 专项运维

### 7.1 安装与同步

```bash
# 同步 hook 表 + AGENTS_CODEX.md
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  codex sync --repo-root "$PWD"
```

### 7.2 故障排查

| 现象 | 处理 |
|------|------|
| Stop 后任务未完成 | `/implementx` + `framework_goal_drive` stdio |
| hook-state 不可读 | `.codex/hook-state/` 文件损坏 -> 删除后重开 |
| REVIEW_GATE 硬拦 | `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 关闭 |
| Paper prose hook 干扰 | `ROUTER_RS_CODEX_PAPER_PROSE_HOOK=0` 关闭 |

---

## 八、OpenCode / Antigravity 专项运维

### 8.1 OpenCode

- 配置根：`.opencode/`（project）+ `~/.config/opencode/`（user）
- `OPENCODE_HOME` 可覆盖默认路径
- 权限模型：Allow / Ask / Deny（read, write, run, browser）
- 无 shell hook，门控通过 MCP 工具层实现

### 8.2 Antigravity App

- 配置根：`.gemini/`（MCP + Planning Mode）
- 安装：`framework host-integration install --to antigravity-app`

### 8.3 Antigravity CLI

- 配置根：`.antigravitycli/`（JSON hooks）
- 5 事件 hook：`SessionStart`、`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`
- 关闭 REVIEW 硬门：`ROUTER_RS_ANTIGRAVITY_CLI_REVIEW_GATE_DISABLE=1`

---

## 九、安全

### 9.1 SSRF 防护架构

| 工具 | 防护层 | 覆盖范围 |
|------|--------|----------|
| `web_fetch`（MCP） | `web_fetch_guard.rs` | HTTP(S) only、IP 黑名单（loopback/private/link-local/CGNAT/metadata）、host 后缀黑名单（`.localhost/.local/.internal`）、DNS pinning、重定向逐跳校验 |
| `browser_open`（MCP） | `web_fetch_guard.rs::validate_browser_open_url` | 阻断非 http(s) scheme（`file://`/`data:`/`javascript:`）、复用 IP/host 黑名单 |
| Bash `curl`/`wget` | `settings.json` `excludedCommands` | 沙箱开启时排除在自动放行之外 |

**已知限制**（browser_open）：
- `browser_click`/`browser_fill` 可绕过 SSRF guard（通过页面内链接导航到内网）
- CDP `Page.navigate` 后的 3xx 重定向目标未经校验
- 无 DNS pinning（Chrome 自行解析，存在 TOCTOU 窗口）

### 9.2 沙箱配置

| 宿主 | 沙箱状态 | 说明 |
|------|---------|------|
| Claude Desktop | `sandbox.enabled: false` | 有意关闭，Bash 不受 Seatbelt 限制 |
| Claude Code | 同上 | 同上 |
| Cursor | 由 Cursor 自身管理 | 框架不干预 |

**域名白名单**（`sandbox.network.allowedDomains`，声明性）：
github.com、*.githubusercontent.com、gitlab.com、*.npmjs.org、pypi.org、arxiv.org、*.wikipedia.org、stackoverflow.com、docs.rs、crates.io、api.semanticscholar.org 等 30+ 域名。

### 9.3 MCP 工具安全（hook_policy.rs）

- `session_launch` 的 host 参数禁止 0.0.0.0/169.254/metadata.google 等元数据端点
- `browser_get_network` 参数检测凭证关键词（password/token/secret/cookie/authorization）
- Shell 注入模式检测（`curl|wget ... | sh|bash`、`sh|bash <(curl|wget ...)`)
- MCP 参数中的 `git reset --hard`/`git push --force` 拦截

---

## 十、备份 / 恢复 / 卸载

### 10.1 备份清单

| 文件/目录 | 宿主 | 重要性 |
|-----------|------|--------|
| `<repo>/.claude/` | Claude Desktop/Code | 高（Git 管理） |
| `<repo>/.cursor/` | Cursor | 高（Git 管理） |
| `<repo>/.codex/` | Codex CLI | 高（Git 管理） |
| `~/Library/Application Support/Claude-3p/claude_desktop_config.json` | Claude Desktop | 高 |
| `~/Library/Application Support/Claude-3p/configLibrary/` | Claude Desktop | 高 |
| `~/Library/Application Support/Claude-3p/Local Storage/leveldb/` | Claude Desktop | 中（须 Desktop 退出） |
| `~/.local/share/skill-framework/bin/router-rs` | 所有 | 低（可重编译） |

### 10.2 一键备份

```bash
BACKUP_DIR="$HOME/Desktop/claude-framework-backup-$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"
cp ~/Library/Application\ Support/Claude-3p/claude_desktop_config.json "$BACKUP_DIR/" 2>/dev/null
cp -r ~/Library/Application\ Support/Claude-3p/configLibrary/ "$BACKUP_DIR/configLibrary/" 2>/dev/null
echo "Backup to $BACKUP_DIR"
```

### 10.3 卸载框架投影

```bash
# Claude Desktop / Code
rm -f .claude/CLAUDE.md .claude/settings.json .claude/settings.local.json
rm -f .claude/mcp.json .claude/router-rs-hook.env
rm -rf .claude/rules/ .claude/workflows/ .claude/.framework-projection*.json

# Cursor
rm -rf .cursor/hooks.json .cursor/router-rs-hook.env .cursor/hook-state/
rm -f ~/.cursor/rules/framework.mdc

# Codex CLI
rm -rf .codex/hooks.json .codex/AGENTS_CODEX.md .codex/README.md .codex/hook-state/

# OpenCode
rm -rf .opencode/

# Antigravity
rm -rf .gemini/ .antigravitycli/

# 可选：移除稳定二进制
rm -f ~/.local/share/skill-framework/bin/router-rs
```

---

## 十一、多机同步

| 类别 | 同步方式 | 说明 |
|------|----------|------|
| 仓库代码（含 `.claude/`、`.cursor/` 等） | Git | 所有投影文件随代码同步 |
| `~/.claude/rules/framework.md` | 手动安装 | 每台机器运行 `install-claude.sh --scope user` |
| `~/.cursor/rules/framework.mdc` | 手动安装 | 每台机器运行 host-integration install --scope user |
| 3P configLibrary | **不随 Git** | 每台机器单独运行 egress 补丁 |
| LevelDB 权限 | **不随 Git** | 每台机器单独运行权限补丁 |
| 稳定二进制 | **不随 Git** | 每台机器单独编译安装 |

**新机器上线流程**：
```bash
git clone <repo-url> && cd skill
# 构建
CARGO_TARGET_DIR="$PWD/core/router-rs/target" cargo build --release --manifest-path core/router-rs/Cargo.toml
# 安装需要的宿主（见速查卡）
# Claude Desktop 额外步骤：
./scripts/patch-claude-desktop-3p-cowork-egress.sh
HOME="$HOME" ./scripts/patch-claude-desktop-permission-mode.sh
```

---

## 十二、文件路径速查

### 框架产物

| 用途 | 路径 |
|------|------|
| 跨宿主内核 | `AGENTS.md` |
| 任务物化 | `artifacts/current/<task_id>/` |
| Skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 稳定二进制 | `~/.local/share/skill-framework/bin/router-rs` |

### Claude Desktop 专项

| 用途 | 路径 |
|------|------|
| 代理声明 | `<repo>/.claude/CLAUDE.md` |
| MCP 配置 | `<repo>/.claude/mcp.json` |
| 3P 主配置 | `~/Library/Application Support/Claude-3p/claude_desktop_config.json` |
| Cowork egress | `~/Library/Application Support/Claude-3p/configLibrary/<appliedId>.json` |
| 权限 LevelDB | `~/Library/Application Support/Claude-3p/Local Storage/leveldb/` |
| 日志 | `~/Library/Logs/Claude-3p/main.log` |
| 安装脚本 | `./scripts/install-claude-desktop.sh` |
| egress 脚本 | `./scripts/patch-claude-desktop-3p-cowork-egress.sh` |
| 权限脚本 | `./scripts/patch-claude-desktop-permission-mode.sh` |
| Hook 解除脚本 | `./scripts/unblock-now.sh`、`./scripts/unblock-and-fix.sh` |

### Cursor 专项

| 用途 | 路径 |
|------|------|
| Hook 表 | `<repo>/.cursor/hooks.json` |
| 环境变量 | `<repo>/.cursor/router-rs-hook.env` |
| Hook 状态 | `<repo>/.cursor/hook-state/` |
| 框架叙事 | `~/.cursor/rules/framework.mdc` |
| 性能基准 | `./scripts/bench-hooks.sh` |
| CI 校验 | `./scripts/ci/check-cursor-hooks-parity.sh` |

### 各宿主操作手册（代理行为规范）

| 宿主 | 手册路径 |
|------|----------|
| Claude Desktop | [`docs/hosts/claude-desktop.md`](../hosts/claude-desktop.md) |
| Claude Desktop 联网 | [`docs/hosts/claude-desktop-networking.md`](../hosts/claude-desktop-networking.md) |
| Claude Code | [`docs/hosts/claude.md`](../hosts/claude.md) |
| Cursor | [`docs/hosts/cursor.md`](../hosts/cursor.md) |
| Codex CLI | [`docs/hosts/codex-cli.md`](../hosts/codex-cli.md) |
| OpenCode | [`docs/hosts/opencode.md`](../hosts/opencode.md) |
| Antigravity | [`docs/hosts/antigravity.md`](../hosts/antigravity.md) |
| Antigravity App | [`docs/hosts/antigravity-app.md`](../hosts/antigravity-app.md) |
| Antigravity CLI | [`docs/hosts/antigravity-cli.md`](../hosts/antigravity-cli.md) |
