---
name: graphviz-expert
description: |
  Create Graphviz/DOT diagrams for precise, orthogonal, publication-quality
  flowcharts, dependency graphs, state machines, class diagrams, and network
  topologies.
  Use when the user asks for Graphviz, DOT, `.dot` / `.gv`, 精确流程图, 正交流程图,
  依赖图, 调用图, 类图, 拓扑图, or when Mermaid is too loose for pixel-precise,
  orthogonal, or dense graph layouts.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - Graphviz
  - DOT
  - flowchart
  - graph layout
  - publication diagram
  - dependency graph
runtime_requirements:
  commands:
    - dot
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - graphviz
    - dot
    - flowchart
    - graph-layout
    - publication-diagram
    - dependency-graph
risk: low
source: local
---

# Graphviz Expert

This skill owns **Graphviz/DOT diagram design and code generation** for
precise, publication-quality graph visualizations.

## When to use

- The user asks for Graphviz, DOT, `.dot`, or `.gv` diagrams
- The user needs **pixel-perfect**, **orthogonal**, or **Word-style** process
  charts where Mermaid layout is insufficient
- The task involves complex dependency graphs, call graphs, or class hierarchies
  with many nodes and edges
- The user needs precise control over node placement, edge routing, and layout
- The diagram must be rendered as high-quality PNG, SVG, or PDF
- The user wants black/white publication diagrams suitable for journal submission

## Do not use

- The user wants simple Mermaid diagrams that render inline in Markdown →
  use `$mermaid-expert`
- The user wants Mermaid-specific features (sequence diagrams, gantt, journey) →
  use `$mermaid-expert`
- The task is about AI image generation → use `$image-generated`
- The task is about scientific charts/plots → use `$scientific-figure-plotting`

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
