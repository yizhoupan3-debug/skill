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
| `docs/maintenance/claude-desktop-runbook.md` | **stub**：重定向至本目录；历史 URL 保留 |
| `docs/hosts/*.md` | 各宿主 hook 事件、Stop 行为、env 快查（**非**本手册重复） |
| `AGENTS.md` | 跨宿主政策真源，本手册不复制 |
