# LaTeX compile acceleration techniques

This reference is the heavy layer for `latex-compile-acceleration`. Keep
`SKILL.md` as the routing/execution entrypoint and put commands, matrices, and
tradeoffs here.

## Core rule

Do not guess. Separate three timings before recommending a fix:

- **clean build**: no aux/cache output exists.
- **warm build**: same sources, existing aux/cache output.
- **edit loop**: one small source edit, then rebuild/preview.

The best optimization is the one that removes the measured bottleneck without
making the final full build less trustworthy.

## Fast measurement pack

Set the root file once:

```bash
MAIN=main.tex
```

Clean build:

```bash
latexmk -C "$MAIN"
/usr/bin/time -p latexmk -pdf -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build "$MAIN"
```

Warm build:

```bash
/usr/bin/time -p latexmk -pdf -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build "$MAIN"
```

Edit-loop approximation:

```bash
touch "$MAIN"
/usr/bin/time -p latexmk -pdf -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build "$MAIN"
```

If `hyperfine` is available:

```bash
hyperfine --warmup 1 \
  'latexmk -pdf -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build main.tex'
```

Quick log scan:

```bash
rg -n "Rerun|Citation|Reference|undefined|No file|Warning|^!" build/*.log
```

Dependency/output scan:

```bash
rg -n "\\.(bib|bbl|bcf|run\\.xml|toc|lof|lot|aux|pdf|png|eps|svg)" build/*.fls
```

## Decision tree

1. If no baseline exists, install or use `latexmk` first. It removes manual
   rerun guesswork and gives a stable surface for further tuning.
2. If warm builds remain slow and figures dominate, externalize TikZ/PGFPlots
   or isolate figures with `standalone`.
3. If every run spends time loading packages/fonts before pages are processed,
   try preamble precompilation with `mylatexformat`.
4. If only one chapter changes in a large thesis/book, use `\includeonly` or
   `subfiles`.
5. If the pain is editor feedback latency, use `latexmk -pvc`, draft mode, or
   TeXpresso.
6. If CI cold start dominates, optimize package install/cache strategy before
   touching TeX source.
7. If bibliography, index, glossary, or references dominate, keep the
   convergence path serial and optimize rerun discipline instead of parallelism.

## Technique matrix

| Need / bottleneck | Recommended move | Why it helps | Caveats | Sources |
|---|---|---|---|---|
| Unknown or repetitive local builds | `latexmk` | Tracks dependencies and reruns until refs settle | Not true AST-level incremental compilation | [CTAN latexmk](https://ctan.org/pkg/latexmk), [manual PDF](https://tug.ctan.org/support/latexmk/latexmk.pdf), [man page](https://www.mankier.com/1/latexmk) |
| Watch-based rebuilds | `latexmk -pvc` | Rebuilds on file changes | Still reruns TeX when watched sources change | [latexmk man page](https://www.mankier.com/1/latexmk) |
| Near-live preview | TeXpresso | Designed for live rendering and fast error feedback | Editor integration and project maturity matter | [TeXpresso repo](https://github.com/let-def/texpresso) |
| Heavy TikZ | TikZ externalization | Reuses figure PDFs instead of re-typesetting each run | Needs shell escape and cache invalidation discipline | [PGF/TikZ external library](https://tikz.dev/library-external) |
| Heavy PGFPlots | PGFPlots externalization | Reuses exported plots | Preamble/style changes may require cache reset | [PGFPlots externalization docs](https://tikz.dev/pgfplots/libs-external) |
| Large thesis/book, one chapter changing | `\includeonly` | Compiles selected `\include` files while preserving aux scaffolding | Requires `\include`; omitted chapters are not rendered | [LaTeX splitting input](https://latexref.xyz/Splitting-the-input.html), [`\include` / `\includeonly`](https://latexref.xyz/_005cinclude-_0026-_005cincludeonly.html) |
| Chapter/subdocument isolation | `subfiles` | Subfiles can compile alone or inside the main document | Requires project structure buy-in | [CTAN subfiles](https://ctan.org/pkg/subfiles), [subfiles repo](https://github.com/gsalzer/subfiles) |
| Figure/snippet isolation | `standalone` | Compiles figures separately and includes PDFs later | Best when figures are natural separate units | [CTAN standalone](https://ctan.org/pkg/standalone), [standalone repo](https://github.com/MartinScharrer/standalone) |
| Cleaner wrapper behavior | ClutTeX / `latexrun` | Better temp-output handling and wrapper ergonomics | Not guaranteed raw-engine speedup | [ClutTeX repo](https://github.com/minoki/cluttex), [latexrun repo](https://github.com/aclements/latexrun) |
| CI cold starts / reproducibility | Tectonic + cache | Self-contained engine and cached bundles | Not a drop-in replacement for all TeX Live workflows | [Tectonic repo](https://github.com/tectonic-typesetting/tectonic), [guide](https://tectonic-typesetting.github.io/book/latest/getting-started/first-document.html), [setup action](https://github.com/marketplace/actions/setup-tectonic) |
| Explicit task graph | `pytask-latex` | Makes LaTeX dependencies first-class in a workflow | Extra Python tooling; usually still shells out | [pytask-latex repo](https://github.com/pytask-dev/pytask-latex) |
| Document-directed recipes | `arara` | Encodes build commands in the document | More orchestration than raw speed | [arara repo](https://github.com/islandoftex/arara) |
| Package-heavy preamble | `mylatexformat` | Dumps static preamble state into `.fmt` | Regenerate after preamble changes; some packages/fonts resist dumping | [CTAN mylatexformat](https://ctan.org/pkg/mylatexformat), [TeX.SE](https://tex.stackexchange.com/q/39058) |
| Writing phase, images not needed | draft mode | Skips image rendering and marks overfull boxes | Image boxes are blank; final build must disable draft | common best practice |
| Intermediate passes do not need PDFs | engine `-draftmode` | Updates aux without writing PDF | Use only in controlled recipes; final pass must write PDF | [latexmk man page](https://www.mankier.com/1/latexmk) |
| PDF compression costs CPU | lower compression | Avoids zlib/object compression during dev | Larger PDFs; re-enable for final | [TeX.SE](https://tex.stackexchange.com/q/51849) |
| Slow image conversion | pre-convert images | Avoids on-the-fly EPS/SVG conversion | Adds source asset workflow | [Overleaf graphics guide](https://www.overleaf.com/learn/latex/Inserting_Images) |
| Independent chapter builds | `make -jN` with isolated outputs | Runs true independent compile units concurrently | Requires separately compilable units and merge/sign-off path | [TeX.SE](https://tex.stackexchange.com/q/8791) |

## Baseline commands

pdfLaTeX:

```bash
latexmk -pdf -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build main.tex
```

XeLaTeX:

```bash
latexmk -xelatex -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build main.tex
```

LuaLaTeX:

```bash
latexmk -lualatex -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build main.tex
```

Watch mode:

```bash
latexmk -xelatex -pvc -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build main.tex
```

Aux-only pass, when manually controlling final output:

```bash
pdflatex -draftmode -interaction=nonstopmode -halt-on-error -file-line-error main.tex
pdflatex -interaction=nonstopmode -halt-on-error -file-line-error main.tex
```

## `.latexmkrc` recipes

Default project-local `.latexmkrc`:

```perl
$pdf_mode = 1;  # 1=pdflatex, 4=lualatex, 5=xelatex

$pdflatex = 'pdflatex -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 %O %S';
$xelatex  = 'xelatex  -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 %O %S';
$lualatex = 'lualatex -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 %O %S';

$out_dir = 'build';
$aux_dir = 'build/aux';

$bibtex_use = 2;  # use biber when biblatex needs it
$clean_ext = 'synctex.gz run.xml bbl bcf nav snm vrb fdb_latexmk fls';

$preview_continuous_mode = 1;
$pdf_previewer = 'open -a Preview %S';  # macOS
```

For TikZ externalization, make shell escape explicit:

```perl
$pdflatex = 'pdflatex -shell-escape -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 %O %S';
$xelatex  = 'xelatex  -shell-escape -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 %O %S';
$lualatex = 'lualatex -shell-escape -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 %O %S';
```

## TikZ / PGFPlots externalization

Preamble:

```latex
\usepackage{tikz}
\usetikzlibrary{external}
\tikzexternalize[prefix=build/tikz/]
```

Build:

```bash
mkdir -p build/tikz
latexmk -pdf -shell-escape -outdir=build main.tex
```

Use forced remake after style or macro changes:

```latex
\tikzset{external/force remake}
```

Then remove it after the cache is refreshed.

## Preamble precompilation with `mylatexformat`

Add a dump boundary before the document body:

```latex
% static package/font/macro setup above
\endofdump

\begin{document}
```

Generate the format:

```bash
pdftex -ini -jobname="main-preamble" "&pdflatex" mylatexformat.ltx main.tex
```

For XeLaTeX:

```bash
xetex -ini -jobname="main-preamble" "&xelatex" mylatexformat.ltx main.tex
```

Use it as the first line of `main.tex`:

```latex
%&main-preamble
```

Regenerate the `.fmt` after package, font, preamble macro, or class changes.
If LuaLaTeX/OpenType font dumping fails or becomes fragile, prefer figure
externalization or draft-mode tactics instead.

## Large document tactics

`\includeonly`:

```latex
\includeonly{chapters/introduction,chapters/methods}
```

Rules:

- Use with `\include{...}`, not plain `\input{...}`.
- Keep one full build without `\includeonly` before submission.
- Rebuild all aux files after chapter splits, label moves, or bibliography
  changes.

`subfiles` main file:

```latex
\documentclass{book}
\usepackage{subfiles}
\begin{document}
\subfile{chapters/introduction}
\end{document}
```

`subfiles` chapter file:

```latex
\documentclass[../main.tex]{subfiles}
\begin{document}
Chapter text.
\end{document}
```

Compile a chapter directly:

```bash
latexmk -pdf -outdir=build/chapters chapters/introduction.tex
```

## Draft and local-iteration speedups

Class draft mode:

```latex
\documentclass[draft]{article}
```

Graphics draft mode only:

```latex
\usepackage[draft]{graphicx}
```

Development-only PDF compression bypass:

```latex
\pdfcompresslevel=0
\pdfobjcompresslevel=0
```

For LuaLaTeX, use:

```latex
\pdfvariable compresslevel=0
\pdfvariable objcompresslevel=0
```

Remove these before final output unless larger PDFs are acceptable.

## Tectonic recipes

Batch compile:

```bash
tectonic -X compile main.tex
```

Watch:

```bash
tectonic -X watch main.tex
```

Good fit:

- CI where bundle caching matters.
- Reproducible source-first papers.
- Projects that do not rely on unusual TeX Live shell-escape workflows.

Bad fit:

- Heavy custom shell-escape pipelines.
- Workflows requiring exact TeX Live distribution behavior.

## CI optimization

### GitHub Actions with TeX Live caching

```yaml
- uses: texlive-action/setup-texlive-action@v3
  with:
    profile-path: .github/texlive.profile
    packages-path: .github/texlive.packages
```

Minimal `.github/texlive.profile`:

```text
selected_scheme scheme-minimal
TEXDIR /tmp/texlive
TEXMFLOCAL /tmp/texlive/texmf-local
TEXMFSYSCONFIG /tmp/texlive/texmf-config
TEXMFSYSVAR /tmp/texlive/texmf-var
option_doc 0
option_src 0
```

Cache key should include:

- TeX Live year or image digest.
- `.github/texlive.profile`.
- `.github/texlive.packages`.
- build script or `.latexmkrc`.

### Tectonic cache

```yaml
- uses: actions/cache@v4
  with:
    path: ~/.cache/Tectonic
    key: tectonic-${{ runner.os }}-${{ hashFiles('Tectonic.toml', 'main.tex', '**/*.bib') }}
```

### Docker

Use a pinned image or pinned TeX Live install. Enable layer cache:

```bash
docker buildx build \
  --cache-from type=gha \
  --cache-to type=gha,mode=max \
  -t latex-build .
```

Prefer installing the minimal package set once in the image over installing a
full TeX Live distribution on every CI run.

## Stability and error recovery

Interaction modes:

| Mode | Flag | Behavior | Best for |
|---|---|---|---|
| `errorstopmode` | default | Stops and waits for input | interactive debugging |
| `nonstopmode` | `-interaction=nonstopmode` | Continues and logs errors | `latexmk` / CI |
| `batchmode` | `-interaction=batchmode` | Minimal terminal output | mature automation |
| `scrollmode` | `-interaction=scrollmode` | Scrolls past errors | semi-interactive debugging |

Root-error scan:

```bash
rg -n "^!|^l\\.|Undefined control sequence|Emergency stop|Fatal error" build/*.log
```

Clean discipline:

```bash
latexmk -c main.tex   # light clean, keeps PDF
latexmk -C main.tex   # full clean
```

Use `latexmk -C` after major structural changes, class changes, bibliography
tool changes, or unexplained stale-reference behavior.

## Reproducibility

For bit-stable PDFs when supported by the engine:

```latex
\pdfinfoomitdate=1
\pdftrailerid{}
\pdfsuppressptexinfo=-1
```

In CI:

```bash
export SOURCE_DATE_EPOCH=1704067200
latexmk -pdf main.tex
```

Dependency snapshot:

```latex
\RequirePackage{snapshot}
```

This emits package-version data that helps reproduce a build environment later.

## LuaLaTeX and XeLaTeX notes

LuaLaTeX and XeLaTeX can be slower than pdfLaTeX because font discovery,
OpenType shaping, and Unicode stacks cost more per run.

Useful checks:

- Avoid loading many font families during draft iterations.
- Prefer figure externalization when TikZ is used with LuaLaTeX.
- Measure `microtype`, `fontspec`, and language packages separately if the
  preamble dominates.
- Do not force pdfLaTeX if Unicode/OpenType output quality is a project
  requirement; speed cannot trump correctness.

## Parallelization rules

Good parallel targets:

- `\include` chapters compiled into isolated output directories.
- `subfiles` chapter previews.
- `standalone` figures.
- TikZ/PGFPlots externalized figure PDFs.
- CI shards with explicit source and output ownership.

Bad parallel targets:

- Shared bibliography/index/glossary convergence.
- Shared `build/` or aux directories.
- One monolithic root document.
- Package-heavy preamble load.
- Very short local watch loops.

Safe Makefile shape:

```make
CHAPTERS := intro methods results

.PHONY: chapters
chapters: $(CHAPTERS:%=build/chapters/%.pdf)

build/chapters/%.pdf: chapters/%.tex
	mkdir -p build/chapters/$*
	latexmk -pdf -outdir=build/chapters/$* $<
```

Run:

```bash
make -j4 chapters
```

Keep final integration serial:

```bash
latexmk -C main.tex
latexmk -pdf -outdir=build main.tex
```

## Cache invalidation checklist

| Change | Must invalidate or re-check |
|---|---|
| preamble/class/package/font change | `.fmt`, externalized figures using changed macros/styles |
| bibliography file or style change | `.bbl`, `.bcf`, `run.xml`, full ref convergence |
| figure source change | corresponding externalized/standalone output |
| chapter split or label move | root aux files and `\includeonly` assumptions |
| engine change | all aux/output/cache files |
| CI package list change | TeX Live/Tectonic cache key |
| `.latexmkrc` change | clean build and cache key |

## Validation checklist

After any speed change:

1. Time a clean build.
2. Time a warm build.
3. Time the edit/preview loop.
4. Confirm no unresolved reference/citation warnings remain.
5. Confirm bibliography/index/glossary tools still run when needed.
6. Confirm cache invalidation for the relevant change type.
7. Confirm a deliberately broken file produces readable file-line errors.
8. Confirm `latexmk -C && latexmk ...` still produces the final PDF.
9. For CI, compare cold-start and second-run cache-hit timing.

## Rust control-plane boundary

Rust is useful for durable orchestration: host entrypoints, batch state,
parallel lane fan-out/fan-in, summaries, and resume metadata.

Rust should not hard-code the LaTeX tactic decision. The decision depends on
TeX source structure, engine behavior, figures, bibliography, and user workflow.
Keep that judgment in this skill layer and use Rust only to coordinate bounded
analysis or compile lanes when the parallelism gate is satisfied.
