---
last_verified: "2026-06-22"
scope: modular-ops
depends_on:
  - ../README.md
  - ../../configs/framework/RUNTIME_REGISTRY.json
  - ../../artifacts/current/roadmap-v7.md
---

# 运维手册（按功能模块）

**入口真源**：本目录按 Roadmap v7 **架构治理**（K1–K16）组织运维内容。

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

跨项目引导脚本：`scripts/claude-bootstrap-framework.sh`、`scripts/cursor-bootstrap-framework.sh`（见 [`getting-started.md`](getting-started.md)）。

---

## 模块索引（功能模块）

### B5 — browser-mcp

浏览器自动化 MCP：`browser-mcp` stdio 服务、页面 attach、URL 策略。实现：`core/browser-mcp/`。

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  browser mcp-stdio --repo-root "$PWD"
```

排障：`no browser-mcp runtime attach artifact` → 检查 `browser-mcp` 启动参数；MCP 路径陈旧 → 重跑 `host-integration install`。

相关路径：`core/browser-mcp/` · `docs/operations/security.md` §SSRF · `RUNTIME_REGISTRY.json` → `managed_mcp_servers.browser-mcp`

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
| 安装 / 升级 / 多机同步 | [getting-started.md](getting-started.md) |
| 安全（SSRF、MCP 策略、沙箱） | [security.md](security.md) |
| 备份 / 恢复 / 卸载 | [backup-restore.md](backup-restore.md) |
| 运维开关组合（profile） | [`spec.md`](../spec.md) §5 + [`framework_profile_contract.md`](../framework_profile_contract.md) |
| 使用者入门 | [`getting-started.md`](getting-started.md) + [`AGENTS.md`](../../AGENTS.md) |

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

TASK_STATE.json 是只读投影（schema v2），聚合：GOAL_STATE + RFV_LOOP_STATE + EVIDENCE_INDEX + STEP_LEDGER + SESSION_SUMMARY + NEXT_ACTIONS + TRACE_METADATA。

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

## 文件路径速查

| 用途 | 路径 |
|------|------|
| 跨宿主内核 | `AGENTS.md` |
| 任务物化 | `artifacts/current/<task_id>/` |
| Skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 稳定二进制 | `~/.local/share/skill-framework/bin/router-rs` |
