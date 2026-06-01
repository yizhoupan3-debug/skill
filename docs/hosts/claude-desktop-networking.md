---
last_verified: "2026-06-02"
depends_on:
  - claude-desktop.md
---

# Claude Desktop 联网与权限操作手册

**适用**：macOS Claude Desktop（含 3P / CC Switch + MiniMax gateway）。

**代理必读**：项目 `.claude/CLAUDE.md`（install 写入）声明联网 tool 顺序；本文是**给人看的运维步骤**（联网 + Allow/Deny 权限）。

---

## 日常维护清单（换机 / 升级 / CC Switch 重配后）

在 skill 仓库根目录，按序执行（**权限脚本须先 Cmd+Q 完全退出 Desktop**）：

```bash
./scripts/install-claude-desktop.sh
./scripts/patch-claude-desktop-3p-cowork-egress.sh
# Desktop 已 Cmd+Q 后：
HOME="$HOME" ./scripts/patch-claude-desktop-permission-mode.sh
```

然后 **重开 Desktop** → Connectors 确认 MCP 已连接 → **新开 Cowork 会话**验证。

---

## 联网：一句话结论

| 标签 | 外网怎么查 |
|------|------------|
| **Chat** | MCP `web_fetch` → `browser-mcp` |
| **Cowork** | **只用 `browser-mcp`**（`browser_open` / `browser_get_text`） |

Cowork 下 **`WebSearch` / `mcp__workspace__web_fetch` 在本环境不可靠**；勿用 Bash `curl` 绕过。

---

## 首次安装

```bash
./scripts/install-claude-desktop.sh
```

写入 project `.claude/*`、user/3P `claude_desktop_config.json`、稳定二进制 `~/.local/share/skill-framework/bin/router-rs`。

自检：

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration status
```

---

## 3P Cowork：放行 VM 出站

默认仅允许 inference gateway（`127.0.0.1`），报 `cowork-egress-blocked` 时：

```bash
./scripts/patch-claude-desktop-3p-cowork-egress.sh
```

手动：编辑 `~/Library/Application Support/Claude-3p/configLibrary/<appliedId>.json`，加入 `"coworkEgressAllowedHosts": ["*"]`（`<appliedId>` 见 `_meta.json`）。详见 [Cowork 3P Configuration](https://claude.com/docs/cowork/3p/configuration)。

---

## 权限：持久化减少 Allow/Deny 弹窗

**保留** AskUserQuestion（多选题澄清）；**减少** 工具/MCP 的 Allow / Deny / Allow once。

### 为何 ~/.claude/settings.json 无效？

Desktop **不读** CLI 的 `permissions.defaultMode`。实际来源两层：

| 层 | 路径 | 作用 |
|----|------|------|
| JSON 偏好 | `Claude-3p/claude_desktop_config.json` → `preferences.epitaxyPrefs.cc-landing-draft-permission-mode` | 默认 composer/Cowork 模式 |
| Electron 缓存 | `Claude-3p/Local Storage/leveldb/` 内 `*permission-mode*` 键 | **运行时真源**（覆盖 JSON 若不同步） |

本机曾出现：JSON 为 `plan`、Cowork audit 为 `permissionMode: default`、Code 会话为 `bypassPermissions` — **Cowork 与 Code 不同步**。

### 一键补丁（须 Desktop 已退出）

```bash
# Cmd+Q 完全退出 Claude Desktop 后：
HOME="$HOME" ./scripts/patch-claude-desktop-permission-mode.sh
# 折中（自动放行编辑，MCP 仍可能偶发弹窗）：
HOME="$HOME" ./scripts/patch-claude-desktop-permission-mode.sh acceptEdits
```

脚本会：

1. 写 `claude_desktop_config.json`：`cc-landing-draft-permission-mode` → `bypassPermissions`
2. 同步工作区目录 permission（含 `~/Claude`、`~/Developer/skill` 等）
3. 修补 LevelDB：`draft-permission-mode` + `folder-permission-mode`（folder 为路径→模式 **映射**，不会误写成字符串）

若 LevelDB 被锁，先只完成 JSON；**必须 quit 后再跑一遍** 才持久生效。

### 验证（重开 Desktop + 新开 Cowork）

1. UI：Cowork 输入框旁权限下拉应显示 **Bypass permissions**（或你选的模式）
2. 行为：跑 `browser-mcp` 联网测试，应**不再**逐步弹 Allow/Deny
3. 日志（可选）— 新会话 audit init 应含 `bypassPermissions`：

```bash
# 找最新 Cowork 会话 audit
ls -t ~/Library/Application\ Support/Claude-3p/local-agent-mode-sessions/*/*/*/audit.jsonl 2>/dev/null | head -1 | \
  xargs rg '"permissionMode"' | head -3
```

期望：`"permissionMode":"bypassPermissions"`。**旧会话不会 retroactive 更新**，请新开 Cowork。

### 模式对照

| 模式 | Allow/Deny | AskUserQuestion |
|------|------------|-----------------|
| `bypassPermissions` | 基本不弹（推荐个人 VM） | 保留 |
| `acceptEdits` | 读写自动，MCP 可能仍弹 | 保留 |
| `plan` / `default` | 频繁弹窗 | 保留 |

---

## 联网自检话术

**Cowork**：

```text
联网测试：依次试 WebSearch、mcp__workspace__web_fetch https://example.com、browser-mcp 打开 https://www.google.com 并简述页面标题。只报告每项成败。
```

**Chat**：

```text
联网测试：依次试 web_fetch https://example.com、browser-mcp 打开 https://www.google.com。只报告每项成败。
```

---

## 故障排查

| 现象 | 处理 |
|------|------|
| MCP 断开 | `./scripts/install-claude-desktop.sh` |
| `cowork-egress-blocked` | `./scripts/patch-claude-desktop-3p-cowork-egress.sh` |
| 频繁 Allow/Deny | Cmd+Q → `HOME="$HOME" ./scripts/patch-claude-desktop-permission-mode.sh` → 新开 Cowork |
| 补丁后仍弹窗 | 确认 LevelDB 步骤未报 locked；旧会话请新开 |
| CC Switch 重配 | 重跑 egress + permission 两个脚本 |
| WebSearch 429 | MiniMax / gateway 限流，与框架无关 |

日志：`tail -f ~/Library/Logs/Claude-3p/main.log`

---

## 文件速查

| 用途 | 路径 |
|------|------|
| 代理声明 | `<repo>/.claude/CLAUDE.md` |
| MCP | `<repo>/.claude/mcp.json`；3P：`~/Library/Application Support/Claude-3p/claude_desktop_config.json` |
| Cowork egress | `~/Library/Application Support/Claude-3p/configLibrary/<appliedId>.json` |
| 权限 JSON | `…/Claude-3p/claude_desktop_config.json` → `preferences.epitaxyPrefs` |
| 权限 LevelDB | `…/Claude-3p/Local Storage/leveldb/` |
| 安装脚本 | `./scripts/install-claude-desktop.sh` |
| egress 脚本 | `./scripts/patch-claude-desktop-3p-cowork-egress.sh` |
| 权限脚本 | `./scripts/patch-claude-desktop-permission-mode.sh` |
| 总览 | [`claude-desktop.md`](claude-desktop.md) |
