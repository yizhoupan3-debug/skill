# Standalone Workflow

Use this workflow whenever a figure must become a paper asset.

## Output Layout

Put generated files together:

```text
figures/<slug>/
  <slug>.tex
  build/<slug>.pdf
  build/<slug>.png
  include-snippet.tex
```

The `.tex` file is the editable source. The PDF is the paper asset. The PNG is
only for review or preview.

## Template

Start from `assets/standalone-figure.tex`.

Default document class:

```tex
\documentclass[tikz,border=2pt]{standalone}
```

Use `border=4pt` only when arrowheads, braces, or callouts sit close to the
canvas edge. Avoid large borders because they waste column width in papers.

## Compilation

Run:

```bash
bash <skill_path>/scripts/compile_standalone.sh figures/method/method.tex
```

Use `--engine xelatex` for CJK labels or fontspec. Use `--engine pdflatex` only
for ASCII/Latin figures that do not need system fonts.

Run the checker before compile:

```bash
bash <skill_path>/scripts/check_tikz_figure.sh figures/method/method.tex
```

## Paper Include Snippet

Create an include snippet next to the figure:

```tex
\begin{figure}[t]
  \centering
  \includegraphics[width=\linewidth]{figures/method/build/method.pdf}
  \caption{Concise paper caption written in the manuscript, not inside TikZ.}
  \label{fig:method}
\end{figure}
```

For a two-column paper, decide whether the figure belongs in `figure` or
`figure*` before final sizing.

## Final-Width Check

Review the PNG at the intended paper width:

- Single column: about `89mm`.
- Double column: about `183mm`.
- IEEE single column: about `3.5in`.

If labels are unreadable at the target width, enlarge the canvas/text hierarchy
or simplify labels. Do not rely on the reader zooming in.
