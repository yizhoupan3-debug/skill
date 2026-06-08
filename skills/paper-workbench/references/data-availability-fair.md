# Data Availability & FAIR（Nature-data 精华）

五宿主统一：手稿数据声明与仓库计划由 **`$paper-workbench`** 在改稿/rebuttal 阶段引用本 reference，不另设热路由 skill。

## 何时读

- 投稿前补 **Data Availability statement**。
- 审稿要求代码/数据/受限访问说明。
- 生物/地学稿需 accession、repository 选择。

## Statement 契约（英文稿常用句式骨架）

1. **Fully open**：数据与代码公开于 [repository]，DOI/URL，license。
2. **Controlled access**：谁可申请、审批机构、伦理批准号。
3. **Embargo**：期限与原因。
4. **Unavailable**：法律/伦理限制；仍说明为何无法共享。
5. **Generated during study**：中间文件是否提供；与正文图表对应关系。

## FAIR 快速核对

| 原则 | 检查问题 |
|------|----------|
| Findable | 有 DOI/ accession？metadata 含 title/creator/date？ |
| Accessible | URL 可访问或申请路径明确？ |
| Interoperable | 格式与社区标准（如 MIAME、MINSEQE）？ |
| Reusable | License、版本 pin、README 可复现？ |

## 仓库选择（按数据类型）

- 通用：`Zenodo`、`Figshare`、`OSF`
- 生物：`GEO`、`ENA`、`SRA`、`dbGaP`（受限）
- 代码：仓库 tag/release + `CITATION.cff` 或 Zenodo DOI

## 与可复现 skill 的分工

环境/seed/分析脚本 → `$experiment-reproducibility` + `research-record-minimum.md`。  
本 reference 只管**投稿面声明**与 FAIR 叙事，不替代实验记录。

## 输出默认

- 一段可粘贴进稿件的 **Data Availability** 英文（或中英对照若用户要求）。
- `unresolved` 列表：缺 accession、缺 license、伦理未批准等 blocker。
