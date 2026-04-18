---
name: mermaid-expert
description: |
  Create Mermaid diagrams for flowcharts, process diagrams, sequence diagrams, ERDs, and state diagrams.
  Use when the user asks for Mermaid, `.mmd` diagrams, 流程图, 研究流程图, 技术路线图, 方法图, 实验流程, pipeline 图, 时序图, 架构图, Gantt charts, user journeys, or wants readable markdown-ready text diagrams. Also use for Mermaid 美化, 导出 PNG/SVG, 自定义主题, mermaid-cli, or mmdc.
metadata:
  model: haiku
  version: "2.0.0"
  platforms: [codex]
  tags:
    - mermaid
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
trigger_hints:
  - Mermaid
  - readable markdown-ready text diagrams
  - flowchart
  - research diagram
  - technical roadmap
  - sequence diagram
  - erd
---

# Mermaid Expert

Use this skill when the user needs a **text-based Mermaid diagram** that is easy
to edit, version, paste into Markdown, or reuse in docs, papers, issues, and
slide notes.

## When to use

- The user asks for Mermaid, `.mmd`, or markdown-friendly diagrams
- The user wants a **research flowchart**, **method pipeline**, **technical roadmap**,
  **experiment workflow**, **paper method figure**, **literature screening flow**,
  **ablation/evaluation pipeline**, or **training/inference workflow**
- The user wants a process visual for docs, README, API notes, or architecture docs
- The user wants one of these Mermaid families:
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

- The user wants a **pixel-perfect publication figure**, Word-style orthogonal
  flowchart, Illustrator-like polished artwork, or exact journal camera-ready art
  → use `$graphviz-expert`
- The user wants a Word-style black/white orthogonal process chart where
  Graphviz/DOT is a better fit than Mermaid → use `$graphviz-expert`
- The user explicitly wants PNG/SVG/PDF rendering automation and the task is
  mainly about browser/CLI rendering rather than Mermaid authoring
- The task is unrelated to diagrams

## Task ownership and boundaries

- This skill owns **diagram design + Mermaid code generation**
- It should choose the **simplest correct Mermaid type** rather than forcing
  everything into a flowchart
- It should keep diagrams **readable first**, not overly decorative
- If Mermaid is a bad fit for the requested visual quality, say so clearly and
  propose a better fallback instead of forcing Mermaid

## Required workflow

1. Extract:
   - goal of the diagram
   - audience
   - diagram type
   - entities / steps / decisions / relationships
   - desired output: Mermaid only, Mermaid + explanation, or Mermaid + styling tips
2. Choose the right Mermaid family:
   - `flowchart` for pipelines, workflows, decision trees, method overviews
   - `sequenceDiagram` for API / protocol / interaction timing
   - `erDiagram` for schema / entities / relationships
   - `stateDiagram-v2` for lifecycle or status transitions
   - `gantt` for roadmap / schedule
   - `journey` for user or experiment journey views
3. Build a **minimal correct version first**
4. Improve labels, grouping, direction, and styling only after structure is correct
5. Check for readability:
   - too many nodes
   - long labels
   - crossing edges
   - mixed abstraction levels
   - unclear decision outcomes
6. Deliver in a clean format the user can paste directly

## Diagram-specific guidance

### 1. Research flowcharts / method figures

Use `flowchart TD` or `flowchart LR` by default.

Prefer this structure:
- input / data
- preprocessing
- core method stages
- decision or branching logic
- outputs / evaluation

Rules:
- one box = one conceptual step
- use concise labels; move detail to notes below the diagram
- keep the main path visually dominant
- split overloaded figures into 2 diagrams if needed
- for papers, prefer neutral labels over chatty labels
- if the figure is for a paper, keep box text noun-phrase or short verb-phrase level
- if the flow includes evaluation or ablation, show them as terminal analysis branches,
  not as overloaded text inside the core method boxes

Common triggers:
- 研究流程图
- 方法流程图
- 技术路线图
- 实验流程
- 文献筛选流程图
- 消融实验流程
- 训练/推理流程
- pipeline 图
- protocol 图

Recommended research layouts:
- `TD`: literature → data → preprocessing → method → evaluation → conclusion
- `LR`: modular method pipelines with grouped stages
- `TD` + decision diamonds: screening / inclusion-exclusion / quality checks

### 2. Sequence diagrams

Use `sequenceDiagram` when order over time matters.

Rules:
- use `autonumber` when step order matters
- separate request and response clearly
- keep actor names short and stable
- use `Note over` sparingly

### 3. ER diagrams

Use `erDiagram` when the user wants entities, fields, or cardinalities.

Rules:
- include only important attributes unless field-level detail is requested
- use clear relationship verbs
- avoid dumping every column into the diagram

### 4. Architecture diagrams

Use `flowchart LR` for high-level architecture.

Rules:
- group related services with `subgraph`
- keep infra, app, and data layers distinct
- avoid mixing runtime flow and static topology in one figure

### 5. Gantt / technical roadmap

Use `gantt` for schedule-like roadmap visuals.

Rules:
- prefer milestone-level granularity
- avoid too many tiny tasks
- map research phases into clear sections

### 6. Literature screening / PRISMA-like flows

Use `flowchart TD`.

Rules:
- model counts as short labels, not paragraph text
- use decision nodes only where inclusion/exclusion logic matters
- keep reasons for exclusion in side notes or short branch labels
- if the user needs strict publication compliance, warn that Mermaid can draft
  the logic well but may need final polishing elsewhere

### 7. Training / inference / evaluation pipelines

Use `flowchart LR` when comparing branches, `TD` when emphasizing stage order.

Rules:
- separate training and inference when they differ materially
- keep datasets, models, and metrics visually distinct
- do not merge ablation, evaluation, and deployment into one unreadable chain

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

```mermaid
<diagram here>
```

## Notes
- <assumption 1>
- <assumption 2>
````

When helpful, also include:
- why this Mermaid type was chosen
- an alternative layout direction (`TD` vs `LR`)
- a “simplified version” if the original is dense
- a caption-ready one-sentence summary for paper/docs reuse

## Quality checklist before delivery

- Mermaid syntax is valid
- The chosen diagram family matches the problem
- Labels are short enough to render well
- Decision branches are explicitly labeled
- Important relationships are visible without explanation
- The user can paste the output directly into Markdown

## Styling, export, and dark mode

For advanced theming (`%%{init:}%%`, `classDef`, `linkStyle`), high-resolution
export via `mmdc` CLI (PNG/SVG/PDF), academic palette, and dark mode adaptation,
see [`resources/styling-and-export.md`](resources/styling-and-export.md).

## If more detail is needed

Open:
- `resources/implementation-playbook.md`
