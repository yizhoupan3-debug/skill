---
last_verified: "2026-06-09"
scope: modular-ops
depends_on:
  - ../README.md
  - ../../configs/framework/RUNTIME_REGISTRY.json
  - ../../artifacts/current/roadmap-v5.md
---

# 运维手册（按功能模块）

**入口真源**：本目录按 Roadmap v5 **板块架构**（B0–B11）组织运维内容，替代历史上按宿主分章的 [`../maintenance/claude-desktop-runbook.md`](../maintenance/claude-desktop-runbook.md)。

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

## 模块索引（Roadmap v5 板块）

| 板块 | 文档 | 职责摘要 |
|------|------|----------|
| **B0** framework-kernel | [b0-framework-kernel.md](b0-framework-kernel.md) | core-state / core-policy / core-math、TelemetryWriter、registry 快照 |
| **B1** routing-engine | [b1-routing-engine.md](b1-routing-engine.md) | 技能路由、tokenize、评分与热路由 JSON |
| **B3** runtime-core | [b3-runtime-core.md](b3-runtime-core.md) | stdio ops、task ledger、live_execute、session_supervisor |
| **B4** host-projection | [b4-host-projection.md](b4-host-projection.md) | host-integration 安装、投影、生成物 drift |
| **B5** browser-mcp | [b5-browser-mcp.md](b5-browser-mcp.md) | browser-mcp MCP 服务、session_launch、URL 防护 |
| **B7** CLI | [b7-cli.md](b7-cli.md) | `router-rs` 命令面、薄壳与向后兼容 |
| **B8** research-engine | [b8-research-engine.md](b8-research-engine.md) | RFV loop、外研 harness、autoresearch-rs |
| **B10** codegraph | [b10-codegraph.md](b10-codegraph.md) | 代码图谱索引、MCP 六工具、sync/watcher |
| **B11** evolution-engine | [b11-evolution-engine.md](b11-evolution-engine.md) | 遥测 journal、evolution-rs analyze/audit |

---

## 横切主题

| 主题 | 文档 |
|------|------|
| 安装 / 升级 / 多机同步 | [getting-started.md](getting-started.md) |
| 安全（SSRF、MCP 策略、沙箱） | [security.md](security.md) |
| 备份 / 恢复 / 卸载 | [backup-restore.md](backup-restore.md) |
| 运维开关组合（profile） | [`../operator_profiles.md`](../operator_profiles.md) → harness §5 为裁判 |
| 使用者一页纸（术语、REVIEW_GATE 快查） | [`../framework_operator_primer.md`](../framework_operator_primer.md) |

---

## 与旧文档的关系

| 旧路径 | 处理 |
|--------|------|
| `docs/maintenance/ops-runbook.md` | **stub**：重定向至本目录；历史 URL 保留 |
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
| `${ANTIGRAVITY_CLI_PROJECT_ROOT:-.}` | Antigravity CLI 变量，不保证注入 |

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
| Claude Code 全局 | `~/.claude/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| Claude Code 项目 | `skill/.claude/mcp.json` | ✅ 硬编码 | ✅ `SKILL_FRAMEWORK_ROOT` |
| Claude Desktop | `Claude-3p/claude_desktop_config.json` | ✅ 硬编码 | ✅ 三组 |
| Gemini CLI 全局 | `~/.gemini/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| Gemini 项目级 | `skill/.gemini/mcp.json` | ✅ 硬编码 | ✅ 三组 |
| OpenCode | `skill/.opencode/opencode.json` | ✅ 硬编码 | ✅ `SKILL_FRAMEWORK_ROOT` |

### 校验命令

```bash
# 一键校验所有 MCP 配置的 --repo-root 是否为绝对路径
python3 -c "
import json, os
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
