> 本文件由 ppt-beamer skill 合并而来。原 skill 已 archived。

# LaTeX Beamer 幻灯片参考

## 概述

将演示文稿构建为 LaTeX Beamer 项目，早期编译并逐页审查生成的 PDF。默认视觉方向：克制的学术/技术 Beamer，包含刻意设计的封面、页脚线、颜色角色和强调框。除非用户明确要求，避免使用启动模板风格。

## 适用场景

- 构建或修改 LaTeX Beamer 演示文稿
- 将笔记、大纲或论文转换为可编辑的 `.tex` 幻灯片加 PDF
- 制作引用密集的学术或技术幻灯片，采用源码优先的工作流
- 构建可在 Git 中版本管理的演示文稿

## 不适用场景

- 用户需要可编辑的 PowerPoint 桌面文件 → 使用 `slides` skill
- 最终输出为 HTML 幻灯片加浏览器匹配的 PDF → 使用 `source-slide-formats` 参考
- 用户需要 Slidev 或 Marp 的快速 Markdown 转幻灯片工作流 → 使用 `source-slide-formats` 参考
- 主要任务是通用 LaTeX 编译速度优化 → 使用 `latex-compile-acceleration`

## Workflow

1. 创建 Beamer 工作区（`main.tex`、`assets/`、`build/`、源日志、可选 `refs.bib`）。
2. 在样式设计前将大纲转换为幻灯片计划。
3. 如果演示文稿需要可复用的视觉标识或品牌一致性，先通过 `$design-md` 路由，再编写主题宏。
4. 尽早定义视觉系统：主题、页脚线策略、强调框、封面/结尾规则、字号比例。
5. 选择本地资产和编译栈（默认 XeLaTeX，适用于中文/混合语言演示文稿）。
   - 如果编译延迟是主要问题而非幻灯片编写，路由到 `latex-compile-acceleration`。
6. 从模板开始，早期编译，从源头修复信息密度而非缩小文字。
7. 逐页渲染/审查 PDF，使用 `$visual-review` 检查重叠/层次/裁剪问题。
8. 交付 `.tex`、编译后的 PDF、本地资产和源日志。

详细编译/密度/QA/Mermaid 路径参见原始 skill 的 [references/workflow.md](../../.archive-cold/ppt-beamer/references/workflow.md)。

## 核心规则（摘要）

- 每帧一个核心信息 — 当两个不相关的内容共享一页时拆分。
- 所有图片和资产仅使用本地路径。
- 默认 16:9 宽高比；中文演示文稿使用 XeLaTeX + `ctex`。
- 在源码中刻意设计视觉系统：封面、页脚线、颜色、强调框、结尾。
- 当存在 `DESIGN.md` 时，将其映射为 Beamer 颜色/字体/强调框宏，而非自行发明主题样式。
- 可读性优先于最大信息密度 — `12pt` 基准，`\normalsize`–`\large` 正文。
- 除非明确要求，封面不放页脚；页码居中于页脚线区块。
- 不得捏造结果。缺失数据标注为计划中/待定/假设。
- 不使用溢出 hack（`\tiny`、`\resizebox`）。从源头修复。
- 不留中文孤行（1-2 个字符单独成行）；保持混合语言标记完整。
- 使用 `$visual-review` 进行视觉 QA 是必须的 — 仅日志检查不充分。

## 实用默认值

- 输出：可编辑的 Beamer 源码加编译后的 PDF，与源文件同目录
- 引擎：XeLaTeX + `ctex`（默认）
- 布局：16:9，每帧一个强信息
- 主题：克制学术风，可读字号，刻意的页脚线/强调框系统
- QA：编译 → 日志 → 渲染 PNG → `$visual-review`

## 最终检查（摘要）

- 帧数与 PDF 页数匹配；`latexmk` 干净退出。
- 通过 `$visual-review` 检查渲染的幻灯片；明确检查重叠。
- 不使用小字变通方案；正文无需缩放即可阅读。
- 无中文孤行；标题平衡；混合语言标记完整。
- 封面有刻意的层次结构，无意外页脚；有结尾帧。
- 所有资产本地化；所有声明可溯源；无捏造的实验结果。

## 资源

- [references/workflow.md](../../.archive-cold/ppt-beamer/references/workflow.md) — 端到端构建/编译/QA 流程
- [references/design-system.md](../../.archive-cold/ppt-beamer/references/design-system.md) — 主题和视觉系统规则
- [references/visual-qa.md](../../.archive-cold/ppt-beamer/references/visual-qa.md) — 渲染页 QA 指导
- [references/checklist.md](../../.archive-cold/ppt-beamer/references/checklist.md) — 完整签署清单

## 运行时依赖

- `latexmk`
- `npx`
- `rsvg-convert`
