> 本文件由 source-slide-formats skill 合并而来。原 skill 已 archived。

# 源码格式幻灯片参考（Slidev / Marp / HTML）

本参考涵盖显式的源码编写幻灯片格式：Markdown、Slidev、Marp 和 HTML/CSS 演示文稿。

## 路由规则

对通用的"做个PPT"请求，先使用 `slides` skill。仅当用户或计划选择了源码格式、实时预览、HTML/CSS 布局控制或浏览器匹配的 PDF 导出时，才使用本参考。

## 适用场景

- 交付物为 Markdown、Slidev、Marp 或 HTML 幻灯片源码。
- 用户需要一个可编辑的文本源加可复现的导出命令。
- 浏览器布局保真度或 HTML 导出 PDF 很重要。
- 任务是跨格式幻灯片维护，源码一致性很重要。
- `.pptx` 可编辑性不是主要需求。

## 不适用场景

- 通用演示文稿接入 → 使用 `slides` skill。
- 源码优先的原生 `.pptx` 加 `deck.plan.json` → 使用 `slides` 原生 PPTX 通道。
- LaTeX Beamer 源码 → 使用 beamer 参考。
- 现有演示文稿修复，PowerPoint 保真度重要 → 使用 `slides` skill。

## Workflow

1. 确认源码格式：Markdown、Slidev、Marp 或 HTML。
2. 如果视觉标识、品牌一致性或可复用样式很重要，先通过 `$design-md` 路由，再编写源码，以便将 tokens 转换为 CSS 变量、Slidev 主题值或 Marp/HTML 样式规则。
3. 以一个源文件为单一事实来源，使导出可复现。
4. 从零开始时使用内置模板。
5. 最终导出前先渲染或预览。
6. 仅在用户要求内部细节时才报告源码/导出链接以外的信息。

## 设计 skill 交接

对 HTML/Slidev/Marp 演示文稿，当用户需要品牌化演示文稿、主题一致性、设计 tokens、图表调色板、可复用的章节/标题幻灯片语法或对 `DESIGN.md` 的验收时，使用 `$design-md`。快速纯文本幻灯片源可跳过。

## 格式说明

- **Markdown / Slidev / Marp**：可使用模板 `assets/slides.template.md`、`assets/slidev.template.md`、`scripts/setup_slidev.sh` 或 `scripts/setup_marp.sh`。
- **HTML/PDF**：当浏览器匹配输出很重要时，可使用 `assets/presentation.template.html`、`assets/print_pdf.template.js`、`scripts/export_pdf.js` 和 `scripts/screenshot_slides.js`。

## 资源

- [references/workflow.md](../../.archive-cold/source-slide-formats/references/workflow.md)
- [references/design-system.md](../../.archive-cold/source-slide-formats/references/design-system.md)
- [references/visual-design-principles.md](../../.archive-cold/source-slide-formats/references/visual-design-principles.md)

## 运行时依赖

- `node`
- `npm`
- `npx`
