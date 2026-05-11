---
name: diagramming
description: |
  Create Mermaid or Graphviz/DOT diagrams for flowcharts, process diagrams, sequence diagrams, ERDs, dependency graphs, state machines, and publication-quality technical diagrams.
  Use when the user asks for Mermaid or Graphviz/DOT, `.mmd` diagrams, 流程图, 研究流程图, 技术路线图, 方法图, 实验流程, pipeline 图, 时序图, 架构图, Gantt charts, user journeys, or wants readable markdown-ready text diagrams. Also use for Mermaid or Graphviz/DOT 美化, 导出 PNG/SVG, 自定义主题, diagramming-cli, or mmdc.
metadata:
  model: haiku
  version: "2.0.0"
  platforms: [codex, cursor]
  tags:
    - diagramming
    - flowchart
    - research-diagram
    - technical-roadmap
    - sequence-diagram
    - erd
risk: low
source: community
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - Mermaid or Graphviz/DOT
  - readable markdown-ready text diagrams
  - flowchart
  - 流程图
  - research diagram
  - 研究流程图
  - technical roadmap
  - 技术路线图
  - 方法图
  - 实验流程
  - pipeline 图
  - sequence diagram
  - 时序图
  - erd

---

# diagramming

Use this skill when the user needs a **text-based Mermaid or Graphviz/DOT diagram** that is easy
to edit, version, paste into Markdown, or reuse in docs, papers, issues, and
slide notes.

## When to use

- The user asks for Mermaid or Graphviz/DOT, `.mmd`, or markdown-friendly diagrams
- The user wants a **research flowchart**, **method pipeline**, **technical roadmap**,
  **experiment workflow**, **paper method figure**, **literature screening flow**,
  **ablation/evaluation pipeline**, or **training/inference workflow**
- The user wants a process visual for docs, README, API notes, or architecture docs
- The user wants one of these Mermaid or Graphviz/DOT families:
  - `flowchart`
  - `sequenceDiagram`
  - `erDiagram`
  - `stateDiagram-v2`
  - `journey`
  - `gantt`
  - `classDiagram`
  - `gitGraph`
  - `timeline`
  - `quadrantChart`

## Do not use

- The user wants AI-rendered raster artwork, screenshots, or non-text-editable illustration output
- The user explicitly wants PNG/SVG/PDF rendering automation and the task is
  mainly about browser/CLI rendering rather than Mermaid or Graphviz/DOT authoring
- The task is unrelated to diagrams

## Task ownership and boundaries

- This skill owns **diagram design + Mermaid or Graphviz/DOT code generation**
- It should choose the **simplest correct Mermaid or Graphviz/DOT type** rather than forcing
  everything into a flowchart
- It should keep diagrams **readable first**, not overly decorative
- If Mermaid or Graphviz/DOT is a bad fit for the requested visual quality, say so clearly and
  propose a better fallback instead of forcing Mermaid or Graphviz/DOT

## Required workflow

1. Extract goal, audience, entities, relationships, and requested format.
2. Choose the simplest correct Mermaid or Graphviz/DOT family.
3. Build a minimal structure first; only then improve labels, grouping, direction, and styling.
4. Check for clutter, long labels, crossing edges, mixed abstraction levels, and unclear branch labels.
5. Deliver paste-ready diagram code plus short notes only when assumptions matter.

## Style rules

- Prefer `TD` for vertical workflows and `LR` for system/architecture views
- Use `subgraph` to reduce clutter
- Keep labels short; use title case or concise noun phrases
- Use consistent node shapes within the same abstraction level
- Add styling only when it improves readability
- Do not overuse colors
- For academic or formal documentation, default to restrained styling
- For paper-like figures, prefer monochrome or low-saturation styling unless
  color encodes meaning
- Label branch outcomes explicitly: `Yes/No`, `Pass/Fail`, `Include/Exclude`

## Output format

Default to:

````markdown
## Diagram

```diagramming
<diagram here>
```

## Notes
- <assumption 1>
- <assumption 2>
````

When helpful, also include:
- why this Mermaid or Graphviz/DOT type was chosen
- an alternative layout direction (`TD` vs `LR`)
- a “simplified version” if the original is dense
- a caption-ready one-sentence summary for paper/docs reuse

## Quality checklist before delivery

- Mermaid or Graphviz/DOT syntax is valid
- The chosen diagram family matches the problem
- Labels are short enough to render well
- Decision branches are explicitly labeled
- Important relationships are visible without explanation
- The user can paste the output directly into Markdown

## Styling, export, and dark mode

For advanced theming (`%%{init:}%%`, `classDef`, `linkStyle`), high-resolution
export via `mmdc` CLI (PNG/SVG/PDF), academic palette, and dark mode adaptation,
see [`resources/styling-and-export.md`](resources/styling-and-export.md).

For diagram family selection, syntax reminders, and implementation patterns,
open `references/syntax-quickref.md` or `resources/implementation-playbook.md`
only when the task needs that extra detail.
