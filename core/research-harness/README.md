# Research Harness

Unified research harness crate for the skill framework. Integrates paper revision loop, literature search, claims management, AIGC detection/reduction, verification pipelines, and research logging.

## Modules

| Module | Description |
|--------|-------------|
| `search` | Literature search (Semantic Scholar, arXiv, paperplain MCP bridge) |
| `claims` | Claim ledger management, drift detection, ceiling computation |
| `log` | Research activity logging (SQLite FTS5), knowledge graph |
| `citation` | Citation audit, BibTeX rendering, DOI validation |
| `review` | Multi-round adversarial review orchestration |
| `hooks` | Paper prose/adversarial hooks, research activity log hooks |
| `aigc` | AIGC detection (n-gram, burstiness, syntactic patterns), humanization |
| `verification` | Literature, statistical, prose QC, structure, formal verification |
| `render` | Markdown rendering pipeline |
| `state` | Research state persistence: load/save/migrate/hydrate from YAML/JSON |
| `workspace` | Workspace initialization, file sync, ledger events |
| `text` | Text processing: slugification, XML parsing, content word extraction |
| `provenance` | Git provenance and environment fingerprint capture |
| `smoke` | Smoke tests for academic source freshness |

## Dependencies

- `core-state` (leaf crate, no cycle risk)
- `loop-engine` (generic loop scheduler)
- Common workspace deps (anyhow, chrono, reqwest, rusqlite, serde, regex, ...)

**不依赖** `runtime-core`，避免循环依赖。
`runtime-core` 可通过函数指针调用 `research-harness` 的 hook 接口。

## MCP Tools

通过 `host-projection` 的 `mcp_stdio_harness` 暴露：

- `research_review_dimensions` — 获取审稿维度 prompt + checklist
- `research_aigc_check` — AIGC 检测（0-100 评分 + 信号列表）
- `research_aigc_humanize` — AIGC 降重（句法改写/词汇替换）
- `research_claim_drift` — Claim 漂移检测（原始 vs 当前声明）
- `research_review_loop` — 多轮对抗审稿循环

## 向后兼容

- `research-harness` 保留为独立 binary（thin CLI wrapper 待完成）
- `host-projection` 的 hook 注册可渐进迁移为调用 `research_harness::hooks`
- 所有现有 MCP tool 名称不变，调用方无感知

## Building

```bash
cargo build -p research-harness
```

## Testing

```bash
cargo test -p research-harness
```

## License

MIT
