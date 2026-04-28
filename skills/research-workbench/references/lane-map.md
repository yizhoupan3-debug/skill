# Research Workbench Lane Map

Use this when the request is research-shaped but the first active lane is unclear.

## Fast Decision Rules

| User intent | Active lane | Why |
|---|---|---|
| "这个课题下一步怎么做" | `research-workbench` | The lane choice is part of the job |
| "帮我 brainstorm 研究点" | `research-workbench` | Divergent early ideation only |
| "帮我搜论文 / 文献梳理 / related work" | `literature-synthesis` | Academic corpus building and synthesis |
| "这个 idea 别人做过没有" | `literature-synthesis` | Novelty check needs evidence |
| "跑多假设实验循环" | `autoresearch` | Repeated hypothesis-control-reflection loop |
| "实现训练/评测 pipeline" | `ai-research` | AI/ML research engineering |
| "这个算法站不站得住" | `research-engineer` | Correctness and defensibility critique |
| "怎么保证实验可复现" | `experiment-reproducibility` | Environment, seeds, configs, data versions |
| "用什么统计检验 / p 值 / 效应量" | `statistical-analysis` | Statistics and uncertainty |
| "科研出图 / matplotlib 论文图" | `scientific-figure-plotting` | Code-generated publication figures |
| "参考文献核查 / BibTeX / DOI" | `citation-management` | Citation truth and formatting |
| "审这篇 paper / 按 reviewer comments 改" | `paper-workbench` | Manuscript-level workflow |

## Minimum Handoff Contract

When leaving `research-workbench` for a narrow lane, carry four fields:

- `phase`: idea, literature, novelty, experiment, rigor, statistics, figure, citation, or paper
- `blocker`: the one reason progress is blocked
- `decision`: why this lane is next
- `next_action`: the smallest concrete action that can change the project state
