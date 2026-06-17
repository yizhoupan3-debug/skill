---
last_verified: "2026-06-17"
plate: B10
---

# B10 — codegraph

## 职责

代码图谱索引与查询：独立 crate `tools/codegraph-rs/`，经 `router-rs` 薄壳 `codegraph_mcp` 暴露 **七工具** MCP（`mcp-codegraph`）。能力：search / callers / callees / impact / node / status / dead_code。

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

Skill 集成（CG-5）：`planx` / `implementx` / `verifyx` / `code-review-deep` 的 `allowed_tools` 与 `SKILL_MANIFEST.json` `allowedTools` 均含七工具 `mcp__mcp-codegraph__*`；各 SKILL.md **CodeGraph 场景** 节见 `skills/{planx,implementx,verifyx,code-review-deep}/SKILL.md`。

## 排障

| 现象 | 处理 |
|------|------|
| MCP 工具不可用 | 确认 `--features codegraph` 构建；重装 host projection |
| 索引空 / stale | 跑 sync + watcher；查 DB schema v1→v2 迁移日志 |
| 性能问题 | W4：rayon 并行 parse + prepared stmt（见 roadmap CG-4） |
| 测试失败 | `cargo test -p codegraph-rs`（当前基线 74 passed） |
| 启动报 UNIQUE constraint | 检查 SKILL_MANIFEST.json slug/hint 是否重名（已修复） |

## 索引失效与 symbol 消歧（v3）

- **content hash**：`files.content_hash` 存 SHA256（hex）；`incremental_sync` 以 hash 判定文件是否 current（不再仅靠 `mtime_ns`）。v2 库经 `migrate_schema` 自动 `ALTER TABLE` 补列；首次 sync 会按新 hash 重索引变更文件。
- **symbol 消歧**：`codegraph_node` 多匹配时返回 `candidates`（含 `file_path` + `kind`）；单匹配仍返回 `node`。`codegraph_callers` / `codegraph_callees` / `codegraph_impact` 在歧义且无过滤时返回 tool error，可传可选 `file_path` 或 `node_id` 限定范围。图查询按 `file_path`/`node_id` 过滤，避免跨文件同名 symbol 串边。

## 相关路径

- `tools/codegraph-rs/`
- `core/runtime-core/src/codegraph_mcp/`
- `configs/framework/RUNTIME_REGISTRY.json` → `mcp-codegraph`
