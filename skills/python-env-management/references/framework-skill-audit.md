# Framework skill audit — pip / uv-pip elimination

Tracked files that taught **operator-facing** `pip`, `uv pip`, or `python -m pip`. Normative owner: `$python-env-management`.

| Status | Path |
|--------|------|
| aligned | `skills/experiment-reproducibility/SKILL.md` |
| aligned | `skills/jupyter-notebook/SKILL.md` |
| aligned | `skills/youtube-summarizer/SKILL.md` |
| aligned | `skills/pdf/references/detailed-guide.md` |
| aligned | `skills/scientific-figure-plotting/references/style-libraries.md` |
| aligned | `skills/scientific-figure-plotting/references/plotnine-guide.md` |
| aligned | `skills/scientific-figure-plotting/references/stat-annotations.md` |
| aligned | `skills/scientific-figure-plotting/references/cjk-font-guide.md` |

**Replacement patterns**

```bash
uv add <pkg>              # runtime dep
uv add --dev <pkg>        # dev / plotting stack
uv sync --all-groups
uvx --with <pkg> <cmd>    # one-shot
uv tool install <cli>
```

Do **not** add new `pip install` / `uv pip` lines in skill bodies. Re-scan after skill edits:

```bash
rg 'pip install|uv pip|python -m pip' skills/ --glob '*.md'
```
