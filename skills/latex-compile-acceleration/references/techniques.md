# LaTeX compile acceleration techniques

This note keeps the **source-backed technique matrix** outside `SKILL.md` so the
skill stays cheap to route.

## Practical ranking

For most repos, the best default stack is:

1. `latexmk` baseline
2. partial compile (`\includeonly`, `subfiles`, `standalone`) when structure allows
3. TikZ / PGFPlots externalization for figure-heavy projects
4. preamble precompilation (`mylatexformat`) for package-heavy documents
5. draft mode during iterative writing
6. TeXpresso for live preview workflows
7. Tectonic + cache for CI / reproducible environments

That ranking is an **inference from the upstream sources below** rather than a
single upstream tool claiming universal superiority.

## Technique matrix

| Need / bottleneck | Recommended move | Why it helps | Caveats | Sources |
|---|---|---|---|---|
| Standard local build is too slow or repetitive | `latexmk` | Tracks dependencies and reruns only as needed; supports preview-continuous mode (`-pvc`) | Still not true AST-level incremental compilation | [CTAN latexmk](https://ctan.org/pkg/latexmk), [latexmk manual PDF](https://tug.ctan.org/support/latexmk/latexmk.pdf), [latexmk man page](https://www.mankier.com/1/latexmk) |
| Need watch-based rebuilds | `latexmk -pvc` | Rebuilds on file changes without manual rerun loops | Still reruns TeX when watched sources change | [latexmk man page](https://www.mankier.com/1/latexmk) |
| Need near-live rendering while editing | TeXpresso | Designed for live rendering and immediate error feedback | Early-phase project; editor integration matters | [TeXpresso repo](https://github.com/let-def/texpresso) |
| Heavy TikZ figures dominate build time | TikZ externalization | Reuses generated figure PDFs instead of re-typesetting every figure | Cache invalidation after macro / preamble changes needs discipline | [PGF/TikZ external library](https://tikz.dev/library-external) |
| Heavy PGFPlots dominate build time | PGFPlots externalization | Later runs include exported graphics, reducing typesetting time considerably | Similar invalidation caveats | [PGFPlots externalization docs](https://tikz.dev/pgfplots/libs-external) |
| Large multi-file book / thesis, only one chapter is changing | `\includeonly` | Compile only selected `\include` files while preserving reference scaffolding | Requires `\include`; omitted chapters are not rendered | [LaTeX reference: splitting input](https://latexref.xyz/Splitting-the-input.html), [LaTeX reference: `\\include` / `\\includeonly`](https://latexref.xyz/_005cinclude-_0026-_005cincludeonly.html) |
| Need chapter / subdocument isolation | `subfiles` | Lets subfiles compile separately or under the main document | Requires project structure buy-in | [CTAN subfiles](https://ctan.org/pkg/subfiles), [subfiles repo](https://github.com/gsalzer/subfiles) |
| Need fast figure / snippet isolation | `standalone` | Great for figure-heavy workflows and separately compiled subdocuments | Most helpful when the repo already treats figures as separate units | [CTAN standalone](https://ctan.org/pkg/standalone), [standalone repo](https://github.com/MartinScharrer/standalone) |
| Want cleaner wrapper behavior and temp-output handling | ClutTeX / `latexrun` | Cleaner output handling, rerun management, and convenient wrappers | Wrapper ergonomics, not guaranteed raw-engine speedup | [ClutTeX repo](https://github.com/minoki/cluttex), [latexrun repo](https://github.com/aclements/latexrun) |
| CI cold starts / reproducibility pain | Tectonic + cache | Self-contained engine with local bundle caching; Actions ecosystem supports caching | Not always a drop-in replacement for every TeX Live workflow | [Tectonic repo](https://github.com/tectonic-typesetting/tectonic), [Tectonic first document guide](https://tectonic-typesetting.github.io/book/latest/getting-started/first-document.html), [setup-tectonic action](https://github.com/marketplace/actions/setup-tectonic) |
| Need a task-graph around LaTeX dependencies | `pytask-latex` | Explicit dependency graph on top of LaTeX projects | Extra Python tooling and still typically shells out to `latexmk` | [pytask-latex repo](https://github.com/pytask-dev/pytask-latex) |
| Need document-directed build rules | `arara` | Encodes build recipes in-document, reducing command drift | More orchestration than raw compile-speed improvement | [arara repo](https://github.com/islandoftex/arara) |
| Package-heavy preamble loads slowly | **Preamble precompilation** (`mylatexformat`) | Dumps preamble state to `.fmt` format file; subsequent runs skip package loading entirely — up to 2× speedup | `.fmt` must be regenerated after preamble changes; some packages incompatible; LuaLaTeX + OpenType fonts may not dump cleanly | [CTAN mylatexformat](https://ctan.org/pkg/mylatexformat), [TeX.SE: precompile preamble](https://tex.stackexchange.com/q/39058) |
| Writing / iterating, images not needed yet | **Draft mode** (`\documentclass[draft]{...}`) | Skips image rendering, marks overfull boxes; ideal for content-first editing | Images show as empty boxes; some packages change behavior in draft | common best practice |
| Intermediate TeX passes generate unneeded PDF | **`-draftmode` flag** for intermediate passes | Tells the engine to skip PDF output and only update aux files; final pass generates real PDF | latexmk does not auto-detect; needs custom `$pdflatex` recipe | [latexmk man page](https://www.mankier.com/1/latexmk) |
| PDF compression costs CPU during compile | **Reduce PDF compression** | `\pdfcompresslevel=0` + `\pdfobjcompresslevel=0` — removes zlib overhead at the cost of larger PDFs | Development only; re-enable for final output | [TeX.SE](https://tex.stackexchange.com/q/51849) |
| Images compiled slowly due to format conversion | **Image format optimization** | Pre-convert figures to PDF (vector) or JPEG (raster); pdfLaTeX handles PDF/JPEG/PNG natively but converts EPS on the fly | Requires upstream tooling (Inkscape, ImageMagick) | [Overleaf graphics guide](https://www.overleaf.com/learn/latex/Inserting_Images) |
| Multi-file project needs parallel chapter builds | **`make -jN` parallel** | Makefile defines per-chapter latexmk rules; `make -j4` compiles chapters concurrently | Only useful for independently compilable chapters; merge step needed | [TeX.SE](https://tex.stackexchange.com/q/8791) |

## Good default commands

### `latexmk` baseline

Use one of these as the first optimization pass:

```bash
latexmk -pdf -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build main.tex
```

```bash
latexmk -xelatex -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build main.tex
```

Watch mode:

```bash
latexmk -xelatex -pvc -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 -outdir=build main.tex
```

### Tectonic

Batch compile:

```bash
tectonic -X compile main.tex
```

Watch mode:

```bash
tectonic -X watch main.tex
```

### Preamble precompilation (`mylatexformat`)

Generate the `.fmt` format file (one-time, re-run only when preamble changes):

```bash
# For pdflatex
pdftex -ini -jobname="mypreamble" "&pdflatex" mylatexformat.ltx main.tex

# For xelatex
xetex -ini -jobname="mypreamble" "&xelatex" mylatexformat.ltx main.tex
```

Then add `%&mypreamble` as the **very first line** of `main.tex` to use the
precompiled preamble. Alternatively, use `\endofdump` in the preamble to define
which portion is "static" (precompiled) vs "dynamic" (re-processed every run).

### Draft-mode compilation

During writing, enable draft mode to skip image rendering:

```latex
\documentclass[draft]{article}
```

Or pass via command line without editing the `.tex` file:

```bash
latexmk -pdf -pdflatex="pdflatex %O '\PassOptionsToClass{draft}{article}\input{%S}'" main.tex
```

### PDF compression bypass (development only)

Add early in preamble for faster dev builds, remove for final output:

```latex
\pdfcompresslevel=0
\pdfobjcompresslevel=0
```

## `.latexmkrc` best practices

A well-configured `.latexmkrc` in the project root improves both speed and
tidiness. Example:

```perl
# Engine selection
$pdflatex = 'pdflatex -interaction=nonstopmode -halt-on-error -file-line-error -synctex=1 %O %S';
# $pdf_mode = 5;  # uncomment for xelatex
# $pdf_mode = 4;  # uncomment for lualatex

# Separate output and aux directories
$out_dir  = 'build';
$aux_dir  = 'build/aux';

# Extra extensions to clean with `latexmk -c`
$clean_ext = 'synctex.gz run.xml bbl nav snm vrb';

# Biber for bibliography (if using biblatex)
$bibtex_use = 2;

# Preview-continuous settings
$preview_continuous_mode = 1;
$pdf_previewer = 'open -a Preview %S';  # macOS; adjust for Linux
```

## Selection heuristics

Use these heuristics unless the repo already has a strong house style:

- **Paper / report / thesis, ordinary workflow** → `latexmk`
- **Figure-heavy scientific manuscript** → `latexmk` + externalization
- **Large thesis / book** → `latexmk` + `\includeonly` or `subfiles`
- **Live-preview-centric editing** → TeXpresso
- **CI / reproducible automation** → Tectonic + cache
- **Wrapper ergonomics / clean temp dirs matter** → ClutTeX or `latexrun`
- **Package-heavy preamble, body iterating fast** → `mylatexformat` precompilation
- **Writing-phase, images irrelevant** → draft mode + `\pdfcompresslevel=0`
- **Multi-chapter book, chapters are independent** → `make -jN` parallel builds
- **CI cold start / GitHub Actions** → `setup-texlive-action` + cache + minimal scheme

## Stability and error recovery

### Interaction modes

Choose the mode that fits your workflow phase:

| Mode | Flag | Behavior | Best for |
|---|---|---|---|
| `errorstopmode` | (default) | Stops on every error, waits for terminal input | Interactive debugging |
| `nonstopmode` | `-interaction=nonstopmode` | Continues past errors; writes all to `.log` | latexmk / CI builds |
| `batchmode` | `-interaction=batchmode` | Suppresses all terminal output except fatal errors | Fully automated scripts |
| `scrollmode` | `-interaction=scrollmode` | Scrolls past errors, shows output | Semi-interactive debugging |

### Error diagnosis workflow

1. **Search for `!`** in the `.log` file — every TeX error starts with `!`
2. **Use `-file-line-error`** to get `filename:line: error` format in the log
3. **Binary search**: comment out half the document, recompile, narrow down
4. **Minimal Working Example (MWE)**: isolate the broken snippet in a fresh file
5. **Clean aux files**: run `latexmk -C` to remove all generated files and rebuild from scratch

### Clean build discipline

Stale auxiliary files (`.aux`, `.toc`, `.lof`, `.bbl`) can cause phantom errors.
Adopt this discipline:

- Run `latexmk -C` (full clean) after major structural changes
- Run `latexmk -c` (light clean, keeps PDF) for routine resets
- In CI, always start from a clean state

## CI deep optimization

### GitHub Actions with TeX Live caching

```yaml
# .github/workflows/latex.yml
- uses: texlive-action/setup-texlive-action@v3
  with:
    # Minimal scheme — install only what you need
    profile-path: .github/texlive.profile
    packages-path: .github/texlive.packages
    # Caches TEXDIR automatically between runs
```

Example `.github/texlive.profile` for minimal footprint:

```
selected_scheme scheme-minimal
TEXDIR /tmp/texlive
TEXMFLOCAL /tmp/texlive/texmf-local
TEXMFSYSCONFIG /tmp/texlive/texmf-config
TEXMFSYSVAR /tmp/texlive/texmf-var
option_doc 0
option_src 0
```

### Docker optimization

- Use Alpine-based TeX Live images for smaller size
- Enable Docker layer caching: `docker buildx build --cache-from type=gha --cache-to type=gha,mode=max`
- Pre-install only required packages in the Dockerfile

### Reproducible PDF output

For bit-reproducible builds (useful for checksums, archival):

```latex
\pdfinfoomitdate=1
\pdftrailerid{}
\pdfsuppressptexinfo=-1
```

Set `SOURCE_DATE_EPOCH` in CI to pin timestamps:

```bash
export SOURCE_DATE_EPOCH=$(date -d '2024-01-01' +%s)
latexmk -pdf main.tex
```

### Dependency pinning

Use the `snapshot` package to record exact dependency versions:

```latex
\RequirePackage{snapshot}  % add before \documentclass
```

Generates a `.dep` file listing all package versions, which can be embedded via
`\RequireVersions{...}` for future verification.

## LuaLaTeX-specific notes

LuaLaTeX is typically 2–3× slower than pdfLaTeX due to OpenType font processing
and Lua interpreter overhead. If using LuaLaTeX:

- Minimize loaded fonts and font features
- Consider `babel` over `polyglossia` (measurable speedup in some locales)
- Evaluate whether `microtype` is worth the overhead
- TikZ externalization is even more impactful under LuaLaTeX

## Validation checklist

After any speed change:

1. time a **clean build**
2. time a **warm build**
3. time the **edit → preview** loop
4. verify references / bibliography still converge
5. verify cache invalidation after:
   - preamble change
   - bibliography change
   - figure-source change
6. verify **error recovery**: compile a deliberately broken file and confirm sensible error output
7. verify **clean build reproducibility**: `latexmk -C && latexmk` produces identical output
8. for CI: verify **cache hit** on second run and **cold-start** timing
