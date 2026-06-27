---

description: Verify and format academic citations and bibliographies.
metadata:
  platforms:
  - supported
  tags:
  - citation
  - bibliography
  - bibtex
  - reference
  - doi
  - academic
  version: '2.4.0'
name: citation-management
scene: research
risk: low
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P2
session_start: preferred
source: project
trigger_hints:
- ACM
- APA
- BibTeX
- BibTeX 格式核查
- DOI 格式验证
- GB/T 7714
- IEEE
- PMID
- Zotero-style cleanup
- 文中引用与参考文献表一致性检查
- 文献引用管理
---
# citation-management

This skill owns reference correctness and style consistency. It makes
citations verifiable, complete, deduplicated, and aligned with the manuscript.

Manuscript workflow context: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md).

## When to Use

- The main object is references, a bibliography, `.bib`, DOI list, or citation style.
- The user wants citation verification, de-duplication, metadata completion, or formatting.
- In-text citations need to match the reference list.
- Claims need a quick source-support check at citation level.

## Do Not Use

- Searching and synthesizing a topic literature corpus -> keep this skill only for citation truth; broader source synthesis belongs to `$paper-workbench` for manuscripts or `$research-discovery` or `$research-execution` for non-manuscript research.
- Writing or polishing manuscript prose -> use `@lane:writer`.
- Checking paper logic beyond citations -> use `@lane:reviewer` logic mode.
- Formatting non-academic documents without citations.

## Truth Rules

- **诚信红线**（不可核验主张、图像诚信、自我剽窃）：[`references/integrity-redlines.md`](references/integrity-redlines.md)；与 `@lane:reviewer` **P0** 口径对齐（致命问题先报、不粉饰）。
- When manuscript context is available, keep citation keys and bibliography
  titles aligned with the frozen terminology in
  [`../paper-workbench/references/research-language-norms.md`](../paper-workbench/references/research-language-norms.md)
  (preferred names for methods/datasets/metrics); do not introduce a second
  naming system in `.bib` that conflicts with in-text terms unless the user
  asked for a rename pass.
- Never invent missing author, title, venue, year, DOI, PMID, or pages.
- **软件 / 数据 / 预印本**：在 `.bib` 或参考表里写清版本、修订、仓库 commit、数据 DOI 或访问条款；不得用模糊「某工具/某数据集」顶替可核对字段。
- Mark unverifiable fields instead of guessing.
- Preserve citation keys unless the user asks to rename them.
- Keep style formatting separate from factual metadata.
- Use current external lookup when citation metadata may be incomplete or stale.
- Treat publisher metadata and DOI records as stronger than copied reference text.
- Keep unresolved ambiguity visible in the output.

## Hard constraints

- 不得发明缺失的作者、标题、年份、DOI 等字段——必须标记为 unverifiable
- 疑似重复条目必须显式列出并由用户确认合并，不得自动丢弃
- 风格格式化与事实元数据必须分开处理——不得在验证阶段修改格式
- paperplain MCP 不可用时，必须降级到 Crossref/PMID fallback 并声明覆盖范围受限
- 诚信红线违规（伪造引用、篡改数据来源）为 P0 blocker，不得豁免

## Tools

**Rust CLI（本仓库真源，无 Python 脚本）**

```bash
cargo run -p citation_tool_rs --manifest-path rust_tools/citation_tool_rs/Cargo.toml -- audit --bib refs.bib
cargo run -p citation_tool_rs --manifest-path rust_tools/citation_tool_rs/Cargo.toml -- render --bib refs.bib --style ieee
```

实现路径：`rust_tools/citation_tool_rs`（`cargo run` / `cargo test -p citation_tool_rs`）。

**首选：paperplain MCP**（论文元数据验证与发现）

| 场景 | 工具 | 说明 |
|------|------|------|
| 有 DOI 的条目 | `mcp__paperplain__fetch_paper(doi)` | 替代手工 Crossref API 调用，返回完整摘要与元数据 |
| 仅有标题的条目 | `mcp__paperplain__find_paper_by_title(title)` | Semantic Scholar 标题匹配，返回 DOI、作者、摘要 |
| 领域文献发现 | `mcp__paperplain__search_research(query)` | 跨 PubMed/ArXiv/Semantic Scholar 搜索 |

**Fallback**（paperplain 未覆盖时）：
- DOAJ（开放获取期刊目录）
- 手工 Crossref API（`https://api.crossref.org/works/{doi}`）
- PubMed/PMC API
- 出版商官方页面

## Workflow

1. Identify the citation style and input format.
2. Parse all entries and detect duplicates, missing fields, and malformed records.
3. Verify high-risk records: **优先**用 `mcp__paperplain__fetch_paper(doi)` 验证有 DOI 的条目；仅有标题时用 `mcp__paperplain__find_paper_by_title(title)` 补全元数据；对 paperplain 未覆盖的条目（如 DOAJ 特定字段、非学术出版物），fallback 到 Crossref/PMID/出版商页面。
4. Normalize names, titles, venues, years, pages, identifiers, and capitalization.
5. Check in-text citations against the reference list when manuscript text is available.
6. Return cleaned entries plus unresolved items.

## Output Defaults

- For `.bib`: corrected BibTeX and a short unresolved list.
- For reference lists: formatted references in the requested style.
- For issue reviews: issue table with severity, entry, problem, and fix.
- For manuscript consistency: missing-in-text and missing-in-reference lists.
- For verification gaps: unresolved entries with the lookup source attempted.

## References

- [references/style-policy.md](./references/style-policy.md)
- [references/integrity-redlines.md](./references/integrity-redlines.md)
