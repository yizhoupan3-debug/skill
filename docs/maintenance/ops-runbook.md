---
last_verified: "2026-06-02"
scope: all-hosts
depends_on:
  - ../hosts/claude.md
  - ../hosts/cursor.md
  - ../hosts/codex.md
  - ../hosts/opencode.md
  - ../hosts/antigravity.md
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
| **Claude Code** | `./scripts/install-claude.sh` | 同左 |
| **Cursor** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to cursor --scope user` | 同左 |
| **Codex CLI** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"` | 同左 |
| **OpenCode** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to opencode --repo-root "$PWD"` | 同左 |
| **Antigravity** | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to antigravity --repo-root "$PWD"` | 同左 |


---

## 二、宿主概览与架构

### 宿主清单

| 宿主 ID | 传输方式 | Hook 事件数 | 配置根 | MCP 支持 |
|---------|---------|-------------|--------|----------|
| `claude-code` | claude-hooks | 4 | `.claude/` + `~/.claude/` | 否（hook 级集成） |
| `cursor` | cursor-hooks | 7 | `.cursor/` + `~/.cursor/` | 否（hook 级集成） |
| `codex` | codex-hooks | 4 | `.codex/` | 否（hook 级集成） |
| `opencode` | opencode-native | 0（纯 MCP） | `.opencode/` + `~/.config/opencode/` | 是 |
| `antigravity` | MCP stdio | 0（纯 MCP） | `.gemini/` | 是 |

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
# Claude Code
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

### 3.5 MCP 配置占位符规则（防回归硬约束）

> **历史事故**：2026-06-08 发现 `${workspaceRoot}`、`${CLAUDE_PROJECT_DIR:-.}`、`${ANTIGRAVITY_CLI_PROJECT_ROOT:-.}` 等占位符在多个宿主中**未被展开**，导致 `router-rs` 的 `--repo-root` 收到空值或 `.`，fallback 到根路径 `/`，所有状态文件写入 `/artifacts/current`。macOS 的 `/` 有 SIP 只读保护，触发 `Read-only file system (os error 30)` 致命错误。

#### 禁止使用的占位符

以下占位符在各宿主 MCP 配置中**不可靠或根本不展开**，**禁止在 `--repo-root` 中使用**：

| 占位符 | 问题 |
|--------|------|
| `${workspaceRoot}` | VS Code/Cursor 变量，MCP stdio 不展开 |
| `${workspaceFolder}` | 同上 |
| `${CLAUDE_PROJECT_DIR:-.}` | Claude Desktop 不保证注入该变量；fallback 到 `.` 时 CWD 不一定是项目根 |
| `${ANTIGRAVITY_CLI_PROJECT_ROOT:-.}` | Antigravity CLI 变量，不保证注入 |

#### 强制规则

1. **`--repo-root` 必须使用绝对硬编码路径**，如 `/Users/joe/Developer/skill`
2. **每个 MCP server 定义必须注入三个 env 变量**（至少包含一个，推荐全部）：
   ```json
   "env": {
     "FRAMEWORK_ROOT": "/Users/joe/Developer/skill",
     "PROJECT_ROOT": "/Users/joe/Developer/skill",
     "SKILL_FRAMEWORK_ROOT": "/Users/joe/Developer/skill"
   }
   ```
3. **`command` 推荐使用绝对路径**（如 `/Users/joe/.local/share/skill-framework/bin/router-rs`），避免 PATH 解析失败

#### 当前配置矩阵（2026-06-08 核查）

| 宿主 | 配置文件 | `--repo-root` | env 注入 |
|------|----------|---------------|----------|
| Claude Code 全局 | `~/.claude/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| Claude Code 项目 | `skill/.claude/mcp.json` | ✅ 硬编码 | ✅ `SKILL_FRAMEWORK_ROOT` |
| Claude Desktop | `Claude-3p/claude_desktop_config.json` | ✅ 硬编码 | ✅ 三组 |
| Gemini CLI 全局 | `~/.gemini/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| Gemini 项目级 | `skill/.gemini/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| OpenCode | `skill/.opencode/opencode.json` | ✅ 硬编码 | ✅ `SKILL_FRAMEWORK_ROOT` |

#### 校验命令

```bash
# 一键校验所有 MCP 配置的 --repo-root 是否为绝对路径
python3 -c "
import json, glob, os

configs = [
    (os.path.expanduser('~/.claude/mcp.json'), 'mcpServers'),
    (os.path.expanduser('~/.gemini/mcp.json'), 'mcpServers'),
    ('/Users/joe/Developer/skill/.claude/mcp.json', 'mcpServers'),
    ('/Users/joe/Developer/skill/.gemini/mcp.json', 'mcpServers'),
    (os.path.expanduser('~/Library/Application Support/Claude-3p/claude_desktop_config.json'), 'mcpServers'),
]
for path, key in configs:
    try:
        d = json.load(open(path))
        for name, srv in d.get(key, {}).items():
            args = srv.get('args', [])
            if '--repo-root' in args:
                root = args[args.index('--repo-root') + 1]
                status = '✅' if root.startswith('/') else '❌'
                print(f'{status} {os.path.basename(path)}: {name} → {root}')
    except Exception as e:
        print(f'⚠️  {path}: {e}')
"
```

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
| Stop 出现 REVIEW_GATE nudge | 全局 advisory-only（不硬拦 Stop）；my-light suppress；可用 `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE=1` 完全关闭 |
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
| Stop 出现 CODEX_REVIEW_GATE nudge | 全局 advisory-only（不硬拦 Stop）；`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 关闭 |
| Paper prose hook 干扰 | `ROUTER_RS_CODEX_PAPER_PROSE_HOOK=0` 关闭 |

---

## 八、OpenCode / Antigravity 专项运维

### 8.1 OpenCode

- 配置根：`.opencode/`（project）+ `~/.config/opencode/`（user）
- `OPENCODE_HOME` 可覆盖默认路径
- 权限模型：Allow / Ask / Deny（read, write, run, browser）
- 无 shell hook，门控通过 MCP 工具层实现

### 8.2 Antigravity

- 配置根：`.gemini/`（MCP + Planning Mode）
- 安装：`framework host-integration install --to antigravity --scope project`（全宿主刷新见 `./scripts/install-all-hosts.sh`）
- Closeout 分层：review 缺口为 MCP **ADVISORY**；**非 my-light** 时 `closeout_gate` / `goal_state_manage(complete)` 可 hard-block（见 [`host_adapter_contract.md`](../host_adapter_contract.md) §0.1）

### 8.3 已退役：`antigravity-cli`

> **2026-06**：宿主 id **`antigravity-cli`** 已合并为 canonical **`antigravity`**。勿再 `install --to antigravity-cli`；见 [`MIGRATION.md`](../MIGRATION.md) 与 [`antigravity-cli.md`](../hosts/antigravity-cli.md)。

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
| Claude Code | `sandbox.enabled: false` | 有意关闭，Bash 不受 Seatbelt 限制 |
| Cursor | 由 Cursor 自身管理 | 框架不干预 |

**域名白名单**（`sandbox.network.allowedDomains`，声明性）：
github.com、*.githubusercontent.com、gitlab.com、*.npmjs.org、pypi.org、arxiv.org、*.wikipedia.org、stackoverflow.com、docs.rs、crates.io、api.semanticscholar.org 等 30+ 域名。

### 9.3 MCP 工具安全（hook_policy/）

- `session_launch` 的 host 参数禁止 0.0.0.0/169.254/metadata.google 等元数据端点
- `browser_get_network` 参数检测凭证关键词（password/token/secret/cookie/authorization）
- Shell 注入模式检测（`curl|wget ... | sh|bash`、`sh|bash <(curl|wget ...)`)
- MCP 参数中的 `git reset --hard`/`git push --force` 拦截

---

## 十、备份 / 恢复 / 卸载

### 10.1 备份清单

| 文件/目录 | 宿主 | 重要性 |
|-----------|------|--------|
| `<repo>/.claude/` | Claude Code | 高（Git 管理） |
| `<repo>/.cursor/` | Cursor | 高（Git 管理） |
| `<repo>/.codex/` | Codex CLI | 高（Git 管理） |
| `~/.local/share/skill-framework/bin/router-rs` | 所有 | 低（可重编译） |

### 10.2 一键备份

```bash
BACKUP_DIR="$HOME/Desktop/claude-framework-backup-$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"
echo "Backup to $BACKUP_DIR"
```

### 10.3 卸载框架投影

```bash
# Claude Code
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
rm -rf .gemini/

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
```

---

## 十二、多端配置同步

| 类别 | 同步方式 | 说明 |
|------|----------|------|
| Git 真源 | `configs/framework/RUNTIME_REGISTRY.json`、`skills/SKILL_ROUTING_RUNTIME.json`、共用 `docs/`、`AGENTS.md` 随仓库同步 | 所有投影文件随代码同步 |
| 宿主投影 | 仅同步各宿主目录模板（`.cursor/`、`.codex/`、`.claude/` 等 **非** gitignore 的模板与 `hooks.json` 结构） | 机器本地 `settings.local.json`、token、路径 **不要** 提交 |
| dotfiles 可选层 | 用户级 `~/.cursor/rules/framework.mdc` 由 `framework maint` / install 生成 | 多台机器用同一 dotfiles 仓或 `chezmoi` 管理用户级规则 |
| 避免双注册 | Codex 勿同时启用 `~/.codex/hooks.json` 与项目 `.codex/hooks.json` 重复调用 `router-rs codex hook` | 见 operator primer「双注册」 |

## 十三、日常检查清单

- [ ] `cargo test -p router-rs` 与仓库 policy 测试通过
- [ ] `framework doctor` 无 P0 项
- [ ] `artifacts/current/<task_id>/` 任务结束后 `/verifyx` purge（见 `skills/verifyx/SKILL.md`）
- [ ] Dependabot PR：合并前跑 CI，Cargo.lock 与宿主 hook 路径无漂移

---

## 十四、文件路径速查

### 框架产物

| 用途 | 路径 |
|------|------|
| 跨宿主内核 | `AGENTS.md` |
| 任务物化 | `artifacts/current/<task_id>/` |
| Skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 稳定二进制 | `~/.local/share/skill-framework/bin/router-rs` |

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
| Claude Code | [`docs/hosts/claude.md`](../hosts/claude.md) |
| Cursor | [`docs/hosts/cursor.md`](../hosts/cursor.md) |
| Codex CLI | [`docs/hosts/codex.md`](../hosts/codex.md) |
| OpenCode | [`docs/hosts/opencode.md`](../hosts/opencode.md) |
| Antigravity | [`docs/hosts/antigravity.md`](../hosts/antigravity.md) |

---

## 2026-05 紧急补丁历史（已删，git 可恢复）

| 已删脚本 | git ref（最近修改 commit） | 用途 |
|---|---|---|
| `scripts/fix-hook-critical-event.sh` | `dff7444e1f392547d61e6c6bbcbb737ee832ce6e` | 一键修复 `claude-router-rs-hook.sh` 的 `critical_event` block 行为（router-rs 不可用时统一 allow） |
| `scripts/unblock-now.sh` | `dff7444e1f392547d61e6c6bbcbb737ee832ce6e` | 解除 Stop hook 死锁（重置 `hook_state_*.json` 中 `settings_validated=false` 与 `framework_tested=false`，并清理 `*.lock`） |
| `scripts/execute-audit-recommendations.py` | `6b3fbcfe61c73b93c43878c5bd2224ef21e31673` | 一次性 P0/P1 执行器（清 7 个 slug、统一 `routing_layer`、同步 update triggers） |

如需复用：`git show <sha>^:scripts/<name>` 即可拿回（注意：当前 HEAD 已删除这些文件，`<sha>^` 仍指向其上一次存在状态）。

## Self-test (manual)

- `node scripts/test-workflow-merge.mjs` — `.claude/workflows/workflow-helpers.js` 中 `conservativeMerge` 的 fixture test（**未接入 CI**；本地手动跑，预期输出含 "OK" 或 `exit 0`）。


---

## 十五、状态管理运维

### TTL 参数表

| 资源 | TTL | 清理机制 |
|------|-----|---------|
| hook-state 文件 | 7 天 | PostToolUse 每 10 次触发 + session_start age sweep |
| closeout 记录 | 30 天 | closeout_record_write 入口 |
| .trash 目录 | 30 天 | 同 closeout |
| 不活跃 task 目录 | 7 天 | session_start 归档 |
| TASK_LEDGER.jsonl | 保留最近 50 行 | goal 操作后自动压缩 |

### GOAL_STATE 状态机

GOAL_STATE 的状态转换是**惯例约束**（非硬约束）——任何 → 任何转换技术上合法。

推荐路径：`[无] → running → paused/blocked/completed/superseded → .trash (clear)`

`drive_until_done=true` + `status=running` = 应续跑（hook 注入 AG_FOLLOWUP）。

### TASK_STATE.json 聚合

TASK_STATE.json 是只读投影（schema v2），聚合：GOAL_STATE + RFV_LOOP_STATE + EVIDENCE_INDEX + STEP_LEDGER + SESSION_SUMMARY + NEXT_ACTIONS + TRACE_METADATA。

通过 `ROUTER_RS_TASK_STATE_AGGREGATE_AUTO=1` 启用。

### auto_multi_phase 限制

auto_multi_phase 的语义判断完全靠 LLM，Rust 层只做显式关键词匹配。存在误触发风险。

### closeout 防护

`goal_state_manage(operation=complete)` 不经过 `enforce_closeout_for_session_payload`。closeout_record 缺失时输出 eprintln advisory（非硬拦）。
