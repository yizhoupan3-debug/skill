# LaTeX 结构安全写作蓝图（LaTeX Writer 精华）

吸收 **LaTeX Writer** 精华：在改 `.tex` 时保持 **可编译 + 可 diff + 章节契约不破**，与 [`claim-spine-and-section-contract.md`](claim-spine-and-section-contract.md) 对齐。

## 何时启用

- 用户手稿源文件为 `.tex` / `.Rmd`（Rmd 只改源，不手改 pandoc 产物）。
- 任务含「加一节」「改结构」「补 label/引用」「换模板」。
- 与 `$paper-workbench` 改稿并行：先过本蓝图再落 prose。

## 硬约束（五宿主相同）

1. **源真源**：只改仓库内作者维护的 `.tex` / `.Rmd`；禁止把 `build/`、`*.aux`、`*.log` 当编辑对象。
2. **一次一动**：单轮优先一个 `section` 或一个 `environment` 块；跨多节重构先交 outline（见 claim-spine section contract）。
3. **编译守门**：结构性改动后须能 `latexmk` / 项目 `render_*.R` 通过，或明确 blocker（缺图、缺 bib 条目）。
4. **标签纪律**：每个 `figure`/`table`/`equation` 有唯一 `\label{...}`；正文 `\ref{}` 与 label 一一对应，禁止悬空引用。
5. **宏与包**：不引入新包除非用户授权或模板已列出；自定义 `\newcommand` 集中在前言区，禁止在正文散落同义宏。
6. **数学环境**：display 公式用 `equation`/`align` 等语义环境；行内与块级不混用同一编号链。

## 推荐骨架（IMRaD 可映射）

```latex
% --- preamble: 包、宏、bib 资源（与目标模板一致）---
\documentclass{...}
\usepackage{...}
% \newcommand 集中区

\begin{document}
\maketitle
\begin{abstract} ... \end{abstract}

\section{Introduction}\label{sec:intro}
\section{Related Work}\label{sec:related}
\section{Methods}\label{sec:methods}
\section{Results}\label{sec:results}
\section{Discussion}\label{sec:discussion}
% \section{Conclusion} 若期刊要求分立

\bibliography{refs}  % 或 \printbibliography
\end{document}
```

## 安全编辑清单（改前 30 秒）

| 检查 | 通过标准 |
|------|----------|
| 编辑范围 | 符合 [`edit-scope-gate.md`](edit-scope-gate.md)（默认 `surgical`） |
| Claim 边界 | 新句不超出 claim card（见 claim-spine） |
| 引用 | 新 `\cite{}` 须在 `.bib` 有 key；交 `$citation-management` |
| 图表 | 新 float 含 `\caption` + `\label`；路径相对、可版本管理 |
| 编译 | 本地或 CI 可复现的一键命令已记录 |

## 与 harness 的衔接

- **叙事 / 章节 handoff**：[`claim-spine-and-section-contract.md`](claim-spine-and-section-contract.md)
- **编译加速（可选）**：`latex-compile-acceleration` skill（archived 表；按需显式路由）
- **手稿前门**：`$paper-workbench` → prose chain → [`prose-chain-contract.md`](prose-chain-contract.md)

## 常见失败模式

- 在 abstract 引入正文才有的符号定义 → 移到 Methods 或首次使用前定义。
- 复制会议模板 `\usepackage` 堆叠导致 option clash → 减法：对齐目标期刊最小集。
- 用 `\textbf` 假粗体强调贡献句 → 改叙事与证据句，非版式堆叠。
