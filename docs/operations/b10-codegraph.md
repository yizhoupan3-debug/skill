---
last_verified: "2026-06-09"
plate: B10
---

# B10 — codegraph

## 职责

代码图谱索引与查询：独立 crate `core/codegraph-rs/`，经 `router-rs` 薄壳 `codegraph_mcp` 暴露 **六工具** MCP（`mcp-codegraph`）。能力：search / callers / callees / impact / node / status。

## 启动 / 配置

```bash
# 构建（router-rs 需 codegraph feature）
cargo build --release --manifest-path core/router-rs/Cargo.toml --features codegraph

# MCP stdio（调试）
cargo run --release --manifest-path core/router-rs/Cargo.toml --features codegraph -- \
  codegraph mcp-stdio --repo-root "$PWD"

# 索引：prepare_index / sync / watcher（见 codegraph-rs 集成测试与 CG-3 交付）
cargo test -p codegraph-rs
```

宿主 MCP 注册：`host-integration install` 写入 `mcp-codegraph`；键路径见 `RUNTIME_REGISTRY.json` → `mcp_servers.mcp-codegraph`。

Skill 集成（CG-5）：`planx` / `implementx` / `verifyx` / `code-review-deep` 的 `allowed_tools` 与 `SKILL_MANIFEST.json` `allowedTools` 均含六工具 `mcp__mcp-codegraph__*`；各 SKILL.md **CodeGraph 场景** 节见 `skills/{planx,implementx,verifyx,code-review-deep}/SKILL.md`。

## 排障

| 现象 | 处理 |
|------|------|
| MCP 工具不可用 | 确认 `--features codegraph` 构建；重装 host projection |
| 索引空 / stale | 跑 sync + watcher；查 DB schema v1→v2 迁移日志 |
| 性能问题 | W4：rayon 并行 parse + prepared stmt（见 roadmap CG-4） |
| 测试失败 | `cargo test -p codegraph-rs`（当前基线 25 passed） |

## 相关路径

- `core/codegraph-rs/`
- `core/router-rs/src/codegraph_mcp/`
- `configs/framework/RUNTIME_REGISTRY.json` → `mcp-codegraph`
- `artifacts/current/roadmap-v5-exec/lane-notes/phase-cg-w*.json`
