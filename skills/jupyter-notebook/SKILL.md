---
name: "jupyter-notebook"
description: |
  Create, scaffold, refactor, and normalize Jupyter notebooks (`.ipynb`) for
  experiments, exploratory analysis, demos, and tutorials with clean cell
  structure, reproducibility, and reusable templates. Use when the user wants
  a notebook instead of a script, asks to convert analysis into notebook form,
  or needs structured `.ipynb` edits without hand-editing raw JSON.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - Jupyter notebook
  - ipynb
  - 实验 notebook
  - 分析 notebook
  - tutorial notebook
  - a notebook instead of a script
  - jupyter
  - notebook
  - experiment
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - jupyter
    - notebook
    - experiment
    - analysis
    - tutorial

---

- **Dual-Dimension Audit (Pre: Cell-Flow, Post: Run-Success/Reproducibility Results)** → runtime verification gate
# Jupyter Notebook Skill

Create clean, reproducible Jupyter notebooks for two primary modes:

- Experiments and exploratory analysis
- Tutorials and teaching-oriented walkthroughs

Prefer the bundled templates for consistent structure and fewer JSON mistakes.

## When to use
- Create a new `.ipynb` notebook from scratch.
- Convert rough notes or scripts into a structured notebook.
- Refactor an existing notebook to be more reproducible and skimmable.
- Build experiments or tutorials that will be read or re-run by other people.

## Decision tree
- If the request is exploratory, analytical, or hypothesis-driven, choose `experiment`.
- If the request is instructional, step-by-step, or audience-specific, choose `tutorial`.
- If editing an existing notebook, treat it as a refactor: preserve intent and improve structure.

## Skill path (set once)

```bash
export CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
```

User-scoped skills install under `$CODEX_HOME/skills` (default: `~/.codex/skills`).

## Workflow
1. Lock the intent.
Identify the notebook kind: `experiment` or `tutorial`.
Capture the objective, audience, and what "done" looks like.

2. Scaffold from the template.
Copy a bundled template to avoid hand-authoring raw notebook JSON from scratch.

```bash
mkdir -p output/jupyter-notebook
cp "$CODEX_HOME/skills/jupyter-notebook/assets/experiment-template.ipynb" \
  output/jupyter-notebook/compare-prompt-variants.ipynb
```

```bash
mkdir -p output/jupyter-notebook
cp "$CODEX_HOME/skills/jupyter-notebook/assets/tutorial-template.ipynb" \
  output/jupyter-notebook/intro-to-embeddings.ipynb
```

3. Fill the notebook with small, runnable steps.
Keep each code cell focused on one step.
Add short markdown cells that explain the purpose and expected result.
Avoid large, noisy outputs when a short summary works.

4. Apply the right pattern.
For experiments, follow `references/experiment-patterns.md`.
For tutorials, follow `references/tutorial-patterns.md`.

5. Edit safely when working with existing notebooks.
Preserve the notebook structure; avoid reordering cells unless it improves the top-to-bottom story.
Prefer targeted edits over full rewrites.
If you must edit raw JSON, review `references/notebook-structure.md` first.

6. Validate the result.
Run the notebook top-to-bottom when the environment allows.
- If execution is not possible, say so explicitly and call out how to validate locally.
- **Superior Quality Audit**: For production notebooks or shared experiments, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).
- Use the final pass checklist in `references/quality-checklist.md`.

## Templates
- Templates live in `assets/experiment-template.ipynb` and `assets/tutorial-template.ipynb`.
- After copying a template, update the title and cells using the JSON structure described in `references/notebook-structure.md`.

## Temp and output conventions
- Use `tmp/jupyter-notebook/` for intermediate files; delete when done.
- Write final artifacts under `output/jupyter-notebook/` when working in this repo.
- Use stable, descriptive filenames (for example, `ablation-temperature.ipynb`).

## Dependencies (install only when needed)
Prefer `uv` for dependency management. Optional packages for local notebook execution:

```bash
uv pip install jupyterlab ipykernel
```

## Environment
No required environment variables.

## Reference map
- `references/experiment-patterns.md`: experiment structure and heuristics.
- `references/tutorial-patterns.md`: tutorial structure and teaching flow.
- `references/notebook-structure.md`: notebook JSON shape and safe editing rules.
- `references/quality-checklist.md`: final validation checklist.

## Do not use

- The task is pure Python scripting without notebook format -> answer in the current implementation context
- The task is ML model training optimization -> use `$mac-memory-management` when the concern is Apple Silicon memory/MPS behavior; otherwise answer in the current implementation context
- The task is data cleaning/ETL without notebook requirement -> use the current artifact/source owner when one is selected; do not force a notebook
- The task is scientific figure plotting without notebook context → use `$scientific-figure-plotting`
- "强制进行 Notebook 深度审计 / 检查单元格执行顺序与运行结果一致性。"
- "Use the runtime verification gate to audit this notebook for run-success idealism."
