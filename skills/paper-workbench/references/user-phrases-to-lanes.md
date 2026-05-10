# 用户话术 → 推荐 lane（维护者最小对照表）

目的：减少误命中与“用户被迫选 skill”。本表只做**语义绑定**，不引入新用户入口。

## 1) 手稿前门（默认）

- **`$paper-workbench`**：整篇手稿级请求、先审再改、改到能投、顶刊顶会标准、ref-first 写作、workflow 抱怨（“review 不好用/写作不好用”）。

## 2) 审稿 vs 改稿 vs 局部写作

- **`$paper-reviewer`**：审稿/严审/能不能投/投稿前把关/只做判断不改稿/单维度审（claim、图表、引用、语言、数学等）。
- **`$paper-reviser`**：按审稿意见改、rebuttal 驱动改稿、已知 blocker 要“现在就动稿”，允许删/缩/挪附录/降主张（受 `edit_scope` 约束）。
- **`$paper-writing`**：只改表达/写某一节（abstract/introduction/related work/caption 等），且 claim 边界已冻结或用户明确“不改 claim”。

## 3) 建议 sidecar lanes（挂到 `PAPER_GATE_PROTOCOL` 的 `lane_kind`）

这些 lane **只在主链 gate 已选定**时并行启用，产出供主线程 merge-back；不独立推进 gate。

- **`citation_verify` → `citation-management`**
  - 话术：`.bib` 清理、DOI/PMID 核查、引用缺失/重复、文中引用与参考文献表一致性。
  - 常见 gate：`G5`（reference support）。

- **`figure_audit` / `table_audit` → `figure-table mode` / plotting owners**
  - 话术：只看图表、caption 不自洽、轴/单位/legend、图表密度与可读性、期刊风格出图。
  - 常见 gate：`G11`/`G12`/`G14`。

- **`notation_audit` → `notation sweep`（paper lanes 内部模式）**
  - 话术：符号不统一、缩写首次出现、单位/公式引用、符号表。
  - 常见 gate：`G10`。

- **`statistical_rigor` → `statistical-analysis`**
  - 话术：用什么检验/显著性/效应量/多重比较/统计功效/不确定性报告、A/B 差异是否显著。
  - 常见 gate：`G2`/`G3`/`G5`（证据与主张匹配、比较公平、统计口径）。

- **`reproducibility_check` → `experiment-reproducibility`**
  - 话术：怎么保证可复现、环境/依赖/随机种子、数据版本、实验配置记录、结果复核流程。
  - 常见 gate：`G2`/`G5`/`G14`（证据闭合与报告/披露规范）。

## 4) 反例（避免误路由）

- 用户说“运行/跑一下/execute”但上下文是代码或 CLI：不要因为词面包含“running”而误路由到统计 lane。
- 用户贴一段文字说“润色一下”：默认 `paper-writing`（除非用户明确要整篇判断/整篇改稿）。

