# Research Harness

Unified research harness crate for the skill framework. Integrates paper revision loop, literature search, claims management, AIGC detection/reduction, verification pipelines, research logging, and **LaTeX math formula parsing/rendering**.

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
| `latex` | **LaTeX math formula parser and SVG renderer (based on RaTeX)** |
| `render` | Markdown rendering pipeline |
| `state` | Research state persistence: load/save/migrate/hydrate from YAML/JSON |
| `workspace` | Workspace initialization, file sync, ledger events |
| `text` | Text processing: slugification, XML parsing, content word extraction |
| `provenance` | Git provenance and environment fingerprint capture |
| `smoke` | Smoke tests for academic source freshness |

## LaTeX Module

Based on [RaTeX](https://github.com/erweixin/RaTeX), provides:

- **LaTeX math formula AST parsing** (54 node types)
- **30+ math environments** (equation/align/cases/matrix/CD/prooftree etc.)
- **Macro expansion** (\def/\newcommand/\DeclareMathOperator)
- **Chemical equations** (\ce{})
- **SVG vector rendering**

### Usage

```rust
use research_harness::latex::{parse, render::render_to_svg};

// Parse LaTeX formula to AST
let ast = parse(r"\frac{a^2 + b^2}{c}").unwrap();

// Render to SVG
let svg = render_to_svg(r"\frac{a^2 + b^2}{c}", true).unwrap();
```

### Relationship with Citation Module

The `latex` module **does not replace** existing `citation/audit.rs`, `citation/render.rs`, `citation/doi.rs`. The citation module handles BibTeX literature management and citation auditing, while the latex module handles math formula parsing and rendering. They cover completely different domains and don't interfere with each other.

## Dependencies

- `core-state` (leaf crate, no cycle risk)
- `loop-engine` (generic loop scheduler)
- `ratex-lexer` (LaTeX lexer from RaTeX)
- `ratex-font` (font metrics and symbol tables from RaTeX)
- Common workspace deps (anyhow, chrono, reqwest, rusqlite, serde, regex, ...)

**不依赖** `runtime-core` 或 `host-projection`，避免循环依赖。
`runtime-core` 可通过 trait object 或函数指针调用 `research-harness` 的 hook 接口。

## MCP Tools

通过 `host-projection` 的 `mcp_stdio_harness` 暴露：

- `research_review_dimensions` — 获取审稿维度 prompt + checklist
- `research_aigc_check` — AIGC 检测（0-100 评分 + 信号列表）
- `research_aigc_humanize` — AIGC 降重（句法改写/词汇替换）
- `research_latex_parse` — LaTeX 数学公式 AST 解析
- `research_latex_render_svg` — LaTeX 公式渲染为 SVG（支持内联/独立模式）

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
