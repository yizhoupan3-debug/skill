---
last_verified: "2026-06-26"
scope: modular-ops
depends_on:
  - ../README.md
  - ../../configs/framework/RUNTIME_REGISTRY.json
---

# 运维手册（按功能模块）

**政策与叙事**（生命周期、Closeout、路由规则）以 [`AGENTS.md`](../../AGENTS.md) 为准；本手册只覆盖**操作、配置、排障、路径**。宿主闭集与安装矩阵以 [`RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json) → `host_targets.supported` 为准。

---

## 快速命令

| 场景 | 命令 |
|------|------|
| 健康检查 | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"` |
| 宿主集成状态 | `framework host-integration status` |
| 构建 release | `CARGO_TARGET_DIR="$PWD/core/router-rs/target" cargo build --release --manifest-path core/router-rs/Cargo.toml` |
| 全量测试（常用三 crate） | `cargo test -p router-rs -p codegraph-rs -p observer-rs` |
| SSRF 防护回归 | `cargo test --manifest-path core/router-rs/Cargo.toml -- web_fetch_guard` |

---

## 首次安装

```bash
git clone <repo-url> && cd skill
CARGO_TARGET_DIR="$PWD/core/router-rs/target" cargo build --release --manifest-path core/router-rs/Cargo.toml
framework host-integration install --to <host_id> --repo-root "$PWD"
framework doctor --repo-root "$PWD"
```

### 跨项目引导

```bash
export SKILL_FRAMEWORK_ROOT=/path/to/skill
# Claude
./scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
./scripts/install-claude.sh --scope user
# Cursor
./scripts/cursor-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
framework host-integration install --to cursor --scope user
```

### 附加设置

| 事项 | 操作 |
|------|------|
| Python 环境 | uv-only、Python 3.12、禁全局 `pip`（见 [`AGENTS.md`](../../AGENTS.md) §个人使用） |
| Office CLI | `bash scripts/install-pdf-tool.sh && bash scripts/install-ooxml-tool.sh && bash scripts/install-ppt-tool.sh` |
| 性能基准 | `./scripts/bench-hooks.sh` |

---

## 模块索引

### B5 — browser-mcp

浏览器自动化 MCP 服务（`core/browser-mcp/`）。

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- browser mcp-stdio --repo-root "$PWD"
```

排障：`no browser-mcp runtime attach artifact` → 检查启动参数；MCP 路径陈旧 → 重跑 `host-integration install`。安全策略见 § 安全运维。

### B10 — codegraph

代码图谱索引（`core/codegraph-rs/`），MCP 八工具：search / callers / callees / impact / node / status / dead_code / goto_definition。

```bash
cargo build --release --manifest-path core/router-rs/Cargo.toml --features codegraph
cargo run --release --manifest-path core/router-rs/Cargo.toml --features codegraph -- codegraph mcp-stdio --repo-root "$PWD"
```

排障：工具不可用 → 确认 `--features codegraph` 构建；索引空 → 跑 sync + watcher。

### B11 — observatory-engine

遥测观测离线分析（`tools/observer-rs/`），零 crate 依赖，mmap 读取遥测 JSONL。

```bash
cargo run --manifest-path tools/observer-rs/Cargo.toml -- analyze --help
cargo run --manifest-path tools/observer-rs/Cargo.toml -- audit --config configs/observer/observer.toml
```

排障：找不到 journal → 确认 telemetry 目录；idle analyze 未触发 → 检查仍有 running worker。

---

## MCP 配置占位符规则

> 2026-06-08 事故：`${workspaceRoot}` 等占位符未被展开 → `--repo-root` 收到空值 → `Read-only file system (os error 30)`。

| 禁用占位符 | 问题 |
|-----------|------|
| `${workspaceRoot}`、`${workspaceFolder}` | VS Code/Cursor 变量，MCP stdio 不展开 |
| `${CLAUDE_PROJECT_DIR:-.}` | Claude Desktop 不保证注入；fallback `.` 时 CWD 不一定为项目根 |

**强制规则**：`--repo-root` 须用绝对硬编码路径；每个 MCP server 定义须注入 `FRAMEWORK_ROOT` / `PROJECT_ROOT` / `SKILL_FRAMEWORK_ROOT`；`command` 推荐绝对路径。

**当前配置矩阵**（所有宿主均已 ✅ 硬编码 + ✅ 三组 env 注入）：`~/.claude/mcp.json`、`.mcp.json`、`Claude-3p/claude_desktop_config.json`、`~/.gemini/mcp.json`、`.gemini/mcp.json`、`.opencode/opencode.json`。

校验：`python3 scripts/check-mcp-configs.py`（遍历所有配置文件并检查 `--repo-root` 是否为绝对路径）。

---

## 状态管理运维

| 资源 | TTL | 清理机制 |
|------|-----|---------|
| hook-state 文件 | 7 天 | PostToolUse 每 10 次 + session_start age sweep |
| closeout 记录 | 30 天 | closeout_record_write 入口 |
| .trash 目录 | 30 天 | 同 closeout |
| 不活跃 task 目录 | 7 天 | session_start 归档 |
| TASK_LEDGER.jsonl | 保留最近 50 行 | goal 操作后自动压缩 |

**GOAL_STATE 状态机**：推荐路径 `[无] → running → paused/blocked/completed/superseded → .trash (clear)`。惯例约束（非硬约束）。`drive_until_done=true` + `status=running` = 应续跑。

**TASK_STATE 聚合**：只读投影（schema v2），聚合 GOAL_STATE + QUALITY_GATE_STATE + EVIDENCE_INDEX + STEP_LEDGER + SESSION_SUMMARY + NEXT_ACTIONS + TRACE_METADATA。通过 `ROUTER_RS_TASK_STATE_AGGREGATE_AUTO=1` 启用。

**closeout 防护**：`goal_state_manage(operation=complete)` 不经过 `enforce_closeout_for_session_payload`。closeout_record 缺失时 advisory（非硬拦）。

---

## 多机同步

| 类别 | 同步方式 |
|------|----------|
| 仓库代码（含 `.claude/`、`.cursor/` 等投影） | Git |
| `~/.claude/rules/framework.md`、`~/.cursor/rules/framework.mdc` | 每台机器手动安装（`install-claude.sh --scope user` / `host-integration install --scope user`）|
| 稳定二进制 | **不随 Git**，每台机器单独编译 |

---

## 日常检查清单

- [ ] `cargo test -p router-rs` 与仓库 policy 测试通过
- [ ] `framework doctor` 无 P0 项
- [ ] `artifacts/current/<task_id>/` 任务结束后清理
- [ ] Dependabot PR：合并前跑 CI，Cargo.lock 与 hook 路径无漂移

---

## 备份、恢复与卸载

**备份优先级**：仓库内宿主投影（高，建议 Git 管理）→ `artifacts/current/`（中）→ `~/.local/share/skill-framework/bin/router-rs`（低，可重编译）→ `artifacts/telemetry/`（中）。

**恢复**：`git clone/pull` → `cargo build --release` → `host-integration install` → `framework doctor`。

**卸载框架投影**：`rm -rf .cursor/hooks.json .cursor/router-rs-hook.env .cursor/hook-state/` 等宿主目录。卸载前备份 `artifacts/current/` 与未提交的 `settings.local.json`。

---

## 安全运维

| 工具 | 防护层 |
|------|--------|
| `web_fetch`（MCP） | `web_fetch_guard.rs`：HTTP(S)、IP 黑名单（loopback/private/CGNAT/metadata）、host 后缀黑名单、DNS pinning、重定向逐跳校验 |
| `browser_open`（MCP） | `validate_browser_open_url`：阻断非 http(s) scheme，复用 IP/host 黑名单 |
| Bash `curl`/`wget` | 宿主 `excludedCommands` / 沙箱 |

**browser_open 已知限制**：`browser_click`/`browser_fill` 可绕过 SSRF guard；CDP 重定向目标未经校验；无 DNS pinning。回归：`cargo test --manifest-path core/router-rs/Cargo.toml -- web_fetch_guard`。

**MCP 策略**：`session_launch` 的 host 参数禁止元数据端点；`browser_get_network` 检测凭证关键词；Shell 注入模式检测。Smoke：`cargo test -p router-rs smoke_p0_hook_policy`。

**安全注意事项**：勿提交 `.env`/密钥；`framework doctor` 不替代渗透测试。

---

## Goal 生命周期（v2 — 2026-06）

| 能力 | 操作/命令 | 代码路径 |
|------|----------|---------|
| 复杂度自动检测 | `UserPromptSubmit` hook 自动触发 | `core/core-policy/src/goal_auto_detect.rs` |
| Goal amend | `goal_state_manage(operation="amend")` | `core/core-state/src/state_manager/goal_ops.rs` |
| 严格退出验证 | Stop 管线 advisory check | `core/host-projection/src/hosts/stop_dispatch.rs` |
| 完成归档 | `goal_state_manage(operation="complete")` | `goal_ops.rs:554` |
| 单 session 管理 | 自动（goal 仅创建它的 session 活跃） | `core/core-state/src/state_manager/mod.rs:105` |

完整原理与状态机详见 [`docs/architecture.md §1.3`](../architecture.md#13-无固定阶段-lifecycle)。

---

## 文件路径速查

| 用途 | 路径 |
|------|------|
| 跨宿主内核 | `AGENTS.md` |
| 任务物化 | `artifacts/current/<task_id>/` |
| Skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 稳定二进制 | `~/.local/share/skill-framework/bin/router-rs` |
