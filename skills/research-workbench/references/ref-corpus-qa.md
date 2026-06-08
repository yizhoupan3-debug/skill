# Ref corpus Q&A（本地语料问答）

吸收 **Paper RAG** 精华：在 ref-first 语料之上做**可验收**的本地问答，不替代 `$citation-management` 元数据 gate。

## 何时启用

- 用户已有 `paper_ref/pdf/` 或等价本地 ref 目录。
- 问题针对「这几篇 ref 里怎么写的」而非全网检索。
- 需要页码/段落锚点支撑 story norm 或 related-work 对比。

## 最小 artifact

```text
paper_ref/
  retained.tsv          # path, title, year, venue
  pdf/                  # 本地 PDF
  ref_qa_log.md         # 可选：问答记录与锚点
```

## 工作流（五宿主相同）

1. **清单**：从 `retained.tsv` 列出语料。
2. **索引（推荐）**：本地 FTS（`ref-corpus` CLI，落盘 `artifacts/ref_corpus/index.sqlite`）：

```bash
# 安装（一次）
./scripts/install-ref-corpus-tool.sh

# 全量索引 paper_ref/pdf/
ref-corpus index --corpus paper_ref/pdf --project-root .

# BM25 检索（JSON 便于 agent 解析）
ref-corpus search --query "attention baseline comparison" --json --project-root .

# 增量：仅新改 PDF
ref-corpus index --corpus paper_ref/pdf --resume --project-root .
```

3. **降级路径**：无索引时逐篇 `pdf read`（`$pdf` skill）抽取文本；分块须记录 `doc_id + page/offset` 锚点。
4. **问答**：仅基于检索/抽取片段回答；每条结论附 `doc_path + page_hint + chunk_index`；无法定位则标 `unresolved`，禁止编造页码。
5. **产出**：写入 `ref_qa_log.md` 或并入 `ref_learning_brief.md` 的字段（gap types、evidence order、baseline 惯例）。
6. **Handoff**：故事/改稿 → `$paper-workbench`；引文卫生 → `$citation-management`。

## 硬约束

- 不得把语料问答结论直接当投稿级 claim；须回到 claim-evidence ladder。
- 无本地 PDF 时降级为 `external_research` lane（OpenAlex/arXiv 等），并写明 `retrieval_trace`。
- 当前索引为 **FTS/BM25**（非向量）；若未来加 embedding，须保留 **锚点可追溯**（chunk → source page），与本 workflow 兼容。
