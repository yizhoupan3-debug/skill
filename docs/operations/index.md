---
last_verified: "2026-06-22"
scope: modular-ops
depends_on:
  - ../README.md
  - ../../configs/framework/RUNTIME_REGISTRY.json
---

# 运维手册（按功能模块）

**入口真源**：本目录按 Roadmap v9 **架构治理** 组织运维内容。

**政策与叙事**（生命周期、Closeout、路由规则）仍以仓库根 [`AGENTS.md`](../../AGENTS.md) 为准；本手册只覆盖**操作、配置、排障、路径**。

**宿主闭集与安装矩阵**：仅以 [`configs/framework/RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json) → `host_targets.supported` 为准；各宿主代理行为规范见 [`docs/hosts/`](../hosts/)。

---

## 快速命令

| 场景 | 命令 |
|------|------|
| 健康检查 | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"` |
| 宿主集成状态 | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status` |
| 构建 release | `CARGO_TARGET_DIR="$PWD/core/router-rs/target" cargo build --release --manifest-path core/router-rs/Cargo.toml` |
| 全量测试（常用三 crate） | `cargo test -p router-rs -p codegraph-rs -p evolution-rs` |
| SSRF 防护回归 | `cargo test --manifest-path core/router-rs/Cargo.toml -- web_fetch_guard` |

---

## 首次安装（通用）

```bash
git clone <repo-url> && cd skill
CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
  cargo build --release --manifest-path core/router-rs/Cargo.toml
# 按 RUNTIME_REGISTRY 中目标宿主 id 安装投影：
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to <host_id> --repo-root "$PWD"
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework doctor --repo-root "$PWD"
```

版本升级：`git pull` 后重编 `router-rs`，再对所用宿主重跑 `host-integration install`（或各宿主手册中的 sync 等价命令）。

### 跨项目引导

```bash
export SKILL_FRAMEWORK_ROOT=/path/to/skill

./scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
./scripts/install-claude.sh --scope user

./scripts/cursor-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to cursor --scope user
```

### Python 环境（macOS）

**uv-only**、Python 3.12、禁止全局 `pip`。详见 `skills/python-env-management/SKILL.md`。

### Office CLI（可选）

```bash
bash scripts/install-pdf-tool.sh
bash scripts/install-ooxml-tool.sh
bash scripts/install-ppt-tool.sh
export PATH="$HOME/.local/bin:$PATH"
```

### 性能基准（hook）

```bash
./scripts/bench-hooks.sh
```

---

## 模块索引（功能模块）

### B5 — browser-mcp

浏览器自动化 MCP：`browser-mcp` stdio 服务、页面 attach、URL 策略。实现：`core/browser-mcp/`。

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  browser mcp-stdio --repo-root "$PWD"
```

排障：`no browser-mcp runtime attach artifact` → 检查 `browser-mcp` 启动参数；MCP 路径陈旧 → 重跑 `host-integration install`。

相关路径：`core/browser-mcp/` · 安全策略见 § 安全运维 · `RUNTIME_REGISTRY.json` → `managed_mcp_servers.browser-mcp`

### B10 — codegraph

代码图谱索引（`tools/codegraph-rs/`），MCP 八工具：search / callers / callees / impact / node / status / dead_code / goto_definition。

```bash
cargo build --release --manifest-path core/router-rs/Cargo.toml --features codegraph
cargo run --release --manifest-path core/router-rs/Cargo.toml --features codegraph -- \
  codegraph mcp-stdio --repo-root "$PWD"
```

排障：工具不可用 → 确认 `--features codegraph` 构建；索引空 → 跑 sync + watcher。

相关路径：`tools/codegraph-rs/` · `configs/framework/RUNTIME_REGISTRY.json` → `mcp-codegraph`

### B11 — evolution-engine

自进化离线分析（`tools/evolution-rs/`）：零 crate 级依赖，通过 mmap 读取遥测 JSONL。

```bash
cargo run --manifest-path tools/evolution-rs/Cargo.toml -- analyze --help
cargo run --manifest-path tools/evolution-rs/Cargo.toml -- audit --config configs/evolution/evolution.toml
```

排障：找不到 journal → 确认 telemetry 目录；idle analyze 未触发 → 检查仍有 running worker。

相关路径：`tools/evolution-rs/` · `configs/evolution/evolution.toml` · `core/session-supervisor/src/evolution_idle.rs`

---

## 横切主题

| 主题 | 文档 |
|------|------|
| 安装 / 升级 / 多机同步 | § 首次安装 + § 多机同步（本文件） |
| 安全（SSRF、MCP 策略、沙箱） | § 安全运维（本文件） |
| 备份 / 恢复 / 卸载 | § 备份、恢复与卸载（本文件） |
| 运维开关组合（profile） | [架构规约](../adr/010-ideal-architecture-v10.md)（架构原则） |
| 使用者入门 | [`../README.md`](../../README.md) + [`AGENTS.md`](../../AGENTS.md) |

---

## 与旧文档的关系

| 旧路径 | 处理 |
|--------|------|
| `docs/maintenance/` | 已删除（redirect stub） |
| `docs/hosts/*.md` | 各宿主 hook 事件、Stop 行为、env 快查（**非**本手册重复） |
| `AGENTS.md` | 跨宿主政策真源，本手册不复制 |

---

## MCP 配置占位符规则（防回归硬约束）

> **历史事故**：2026-06-08 发现 `${workspaceRoot}`、`${CLAUDE_PROJECT_DIR:-.}` 等占位符在多个宿主中**未被展开**，导致 `--repo-root` 收到空值或 `.`，fallback 到根路径 `/`，触发 `Read-only file system (os error 30)` 致命错误。

### 禁止使用的占位符

| 占位符 | 问题 |
|--------|------|
| `${workspaceRoot}` | VS Code/Cursor 变量，MCP stdio 不展开 |
| `${workspaceFolder}` | 同上 |
| `${CLAUDE_PROJECT_DIR:-.}` | Claude Desktop 不保证注入该变量；fallback 到 `.` 时 CWD 不一定是项目根 |

### 强制规则

1. **`--repo-root` 必须使用绝对硬编码路径**，如 `/Users/joe/Developer/skill`
2. **每个 MCP server 定义必须注入 env 变量**（至少包含一个，推荐全部）：
   ```json
   "env": {
     "FRAMEWORK_ROOT": "/Users/joe/Developer/skill",
     "PROJECT_ROOT": "/Users/joe/Developer/skill",
     "SKILL_FRAMEWORK_ROOT": "/Users/joe/Developer/skill"
   }
   ```
3. **`command` 推荐使用绝对路径**（如 `/Users/joe/.local/share/skill-framework/bin/router-rs`），避免 PATH 解析失败

### 当前配置矩阵

| 宿主 | 配置文件 | `--repo-root` | env 注入 |
|------|----------|---------------|----------|
| Claude 全局 | `~/.claude/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| Claude 项目 | `.mcp.json` | ✅ 硬编码 | ✅ `SKILL_FRAMEWORK_ROOT` |
| Claude Desktop | `Claude-3p/claude_desktop_config.json` | ✅ 硬编码 | ✅ 三组 |
| Gemini CLI 全局 | `~/.gemini/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| Gemini 项目级 | `.gemini/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| OpenCode | `.opencode/opencode.json` | ✅ 硬编码 | ✅ `SKILL_FRAMEWORK_ROOT` |

### 校验命令

```bash
# 一键校验所有 MCP 配置的 --repo-root 是否为绝对路径
python3 -c "
import json, os
configs = [
    (os.path.expanduser('~/.claude/mcp.json'), 'mcpServers'),
    (os.path.expanduser('~/.gemini/mcp.json'), 'mcpServers'),
    ('.mcp.json', 'mcpServers'),
    ('.gemini/mcp.json', 'mcpServers'),
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

## 状态管理运维

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

TASK_STATE.json 是只读投影（schema v2），聚合：GOAL_STATE + QUALITY_GATE_STATE + EVIDENCE_INDEX + STEP_LEDGER + SESSION_SUMMARY + NEXT_ACTIONS + TRACE_METADATA。

通过 `ROUTER_RS_TASK_STATE_AGGREGATE_AUTO=1` 启用。

### auto_multi_phase 限制

auto_multi_phase 的语义判断完全靠 LLM，Rust 层只做显式关键词匹配。存在误触发风险。

### closeout 防护

`goal_state_manage(operation=complete)` 不经过 `enforce_closeout_for_session_payload`。closeout_record 缺失时输出 eprintln advisory（非硬拦）。

---

## 多机同步

| 类别 | 同步方式 | 说明 |
|------|----------|------|
| 仓库代码（含 `.claude/`、`.cursor/` 等） | Git | 所有投影文件随代码同步 |
| `~/.claude/rules/framework.md` | 手动安装 | 每台机器运行 `install-claude.sh --scope user` |
| `~/.cursor/rules/framework.mdc` | 手动安装 | 每台机器运行 host-integration install --scope user |
| 稳定二进制 | **不随 Git** | 每台机器单独编译安装 |

**新机器上线流程**：`git clone` → `cargo build --release` → 安装需要的宿主（见速查卡）。

---

## 日常检查清单

- [ ] `cargo test -p router-rs` 与仓库 policy 测试通过
- [ ] `framework doctor` 无 P0 项
- [ ] `artifacts/current/<task_id>/` 任务结束后 `/verifyx` purge
- [ ] Dependabot PR：合并前跑 CI，Cargo.lock 与宿主 hook 路径无漂移

---

## 备份、恢复与卸载

### 备份优先级

| 路径 | 重要性 | 说明 |
|------|--------|------|
| 仓库内宿主投影（`.claude/`、`.cursor/`、`.codex/`、`.opencode/`） | 高 | 建议 Git 管理 |
| `artifacts/current/<task_id>/` | 中 | 进行中的 goal / RFV / wave |
| `~/.local/share/skill-framework/bin/router-rs` | 低 | 可重编译 |
| `artifacts/telemetry/` | 中 | evolution 分析输入 |

### 恢复

1. `git clone` / `git pull` 恢复仓库与投影文件
2. `cargo build --release` 重建 `router-rs`
3. `framework host-integration install --to <host_id>` 刷新 MCP / hooks
4. `framework doctor` 确认无 drift WARN

### 卸载框架投影

按所用宿主删除对应投影目录与 hook 配置（**不**要删除整个仓库）。示例：`rm -rf .cursor/hooks.json .cursor/router-rs-hook.env .cursor/hook-state/`。卸载前备份 `artifacts/current/` 与未提交的宿主 `settings.local.json`。

## 安全运维

### SSRF 与 URL 策略

| 工具 | 防护层 | 覆盖 |
|------|--------|------|
| `web_fetch`（MCP） | `web_fetch_guard.rs` | HTTP(S)、IP 黑名单（loopback/private/link-local/CGNAT/metadata）、host 后缀黑名单（`.localhost/.local/.internal`）、DNS pinning、重定向逐跳校验 |
| `browser_open`（MCP） | `validate_browser_open_url` | 阻断非 http(s) scheme（`file://`/`data:`/`javascript:`）、复用 IP/host 黑名单 |
| Bash `curl`/`wget` | 宿主 `excludedCommands` / 沙箱 | 沙箱开启时不自动放行 |

**browser_open 已知限制**：`browser_click`/`browser_fill` 可绕过 SSRF guard；CDP 重定向目标未经校验；无 DNS pinning（Chrome 自行解析）。回归：`cargo test --manifest-path core/router-rs/Cargo.toml -- web_fetch_guard`。

### MCP 工具策略

- `session_launch` 的 host 参数禁止元数据端点
- `browser_get_network` 检测凭证关键词
- Shell 注入模式检测；危险 git 命令拦截

Smoke：`cargo test -p router-rs smoke_p0_hook_policy`。

### 安全注意事项

- 勿将 `.env`、密钥提交 Git
- `framework doctor` 不替代渗透测试
- 详细安全 env 开关：见 [`AGENTS.md`](../../AGENTS.md) § Coding First Principles

## 文件路径速查

| 用途 | 路径 |
|------|------|
| 跨宿主内核 | `AGENTS.md` |
| 任务物化 | `artifacts/current/<task_id>/` |
| Skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 稳定二进制 | `~/.local/share/skill-framework/bin/router-rs` |

---

## Goal 生命周期（v2 — 2026-06）

### 核心变更

| 能力 | 说明 | 对应代码路径 |
|------|------|-------------|
| 复杂度自动检测 | `UserPromptSubmit` hook 使用 `analyze_complexity()` 检测复杂任务并注入 goal 建议上下文 | `core/core-policy/src/goal_auto_detect.rs` |
| Goal amend | `goal_state_manage(operation="amend")` 更新 goal/non_goals/done_when，保留 checkpoints | `core/core-state/src/state_manager/goal_ops.rs` |
| 严格退出验证 | Stop 管线从磁盘读取 `done_when` 与响应内容比对，列出未完成项（advisory，不 hard block） | `core/host-projection/src/hosts/stop_dispatch.rs` |
| 完成后自动归档 | `complete` 操作标记 `archived: true`，不再物理删除 GOAL_STATE.json | `core/core-state/src/state_manager/goal_ops.rs:554` |
| 单 session 管理 | goal 仅在创建它的 session 中活跃；跨 session 目标标记为 stale，不自动恢复 | `core/core-state/src/state_manager/mod.rs:105` |

### 完整生命周期

```
用户输入 → 复杂度检测 → goal 确认
    → start (drive_until_done=true)
    → 工作中 → checkpoint × N（PostToolUse 自动触发）
    → scope change detected → amend（保留进度）
    → done_when 全覆盖 + EVIDENCE 通过 → complete → archived
    → 可开启下一个 goal
```

### 复杂度检测指标（≥2 项命中 → complex）

1. 实现动词（实现/重构/添加/modify/implement 等）
2. 文件路径引用（≥2 处）
3. 任务描述长度（中文>80字符，英文>150字符）
4. 结构化 markers（Goal:/Non-goals:/Done when:）
5. 多步骤描述（≥3 个 bullet/编号）
6. 跨文件/跨 crate 引用

### Scope Change 检测

当已有活跃 goal 且用户消息包含以下关键词时触发 `[Goal Amendment]` 上下文注入：

- 中文：增加/修改/补充/额外/调整/还要/但是/不过/另外/顺便/追加/变更/改动
- English：apart from/also need/additionally/one more thing/actually/instead/change/update/modify

### 退出条件验证（Advisory）

Stop 管线在 goal 未满足时：
1. 读取磁盘 GOAL_STATE.json 的 `done_when` 数组
2. 与 response_text 做子串匹配（每个 done_when 项独立匹配）
3. 计算覆盖率
4. 生成精确的 followup 消息，列出未完成项

**示例 followup**：
```
Goal progress: 2/4 done_when completed, 1 checkpoint. Still missing:
- 已修复 P0 并提交
- cargo test 全部通过
Continue working.
```

### Amend 操作

`goal_state_manage(operation="amend")` 接受以下可选字段：
- `goal` — 更新目标描述
- `non_goals` — 更新非目标列表
- `done_when` — 更新退出条件列表（完全替换，非追加）
- `validation_commands` — 更新验证命令
- `keep_progress` — `true`（默认）保留现有 checkpoints；`false` 清空 checkpoints

**状态要求**：goal 必须处于 running/paused/blocked 状态，不能 amend completed/stale 的 goal。
