# 引用与学术诚信红线（Integrity redlines）

与 [`../SKILL.md`](../SKILL.md) 的 Truth Rules 叠加；涉及 **数据造假、剽窃、不可核验主张** 时，审稿侧按 `$paper-reviewer` **P0 一票否决**处理（见 [`../../paper-reviewer/references/severity-spec.md`](../../paper-reviewer/references/severity-spec.md)）。

## 不可核验的主张

- 禁止编造 DOI、PMID、卷期页码或「幽灵」文献。
- **软件与数据版本**：工具链、数据集、预印本须写 **可核对版本**（commit、发布日期、修订号、Zenodo DOI 等）；避免仅写「某开源工具」而无 pin。
- **预印本 vs 正式出版**：区分 arXiv/bioRxiv 版本与最终版本；结论引用以可复核版本为准。

## 图像与补充材料

- 一图多用、裁剪误导、过度处理（对比度/去背景）须在方法或图注披露；可疑处按 P0 风险上报，不替用户「圆过去」。

## 自我重复与抄袭

- **自我剽窃（self-plagiarism）**：未披露地回收旧文图表/段落进新稿，视为诚信问题；须引用、改写或取得许可并按期刊政策披露。
- **翻译式重复**：跨语言未标注的平行发表同等处理。

## 与 citation-management 工作流的衔接

核查时输出：**问题条目 → 严重度 → 建议修复（含版本 pin 或替换文献）→ 仍不可核验则标 unresolved**。
