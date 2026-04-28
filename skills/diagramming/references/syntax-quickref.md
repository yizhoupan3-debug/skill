# Mermaid Expert — Syntax Quick Reference

Common patterns and pitfalls for Mermaid diagram generation.

## Node Label Quoting Rules

- Labels with special chars must be quoted: `id["Label (Info)"]`
- No HTML tags in labels
- No unescaped parentheses in unquoted labels

## Diagram Type Selection

| User Need | Diagram Type |
|---|---|
| Flow / process | flowchart TD/LR |
| Sequence / API call | sequenceDiagram |
| State machine | stateDiagram-v2 |
| Timeline | timeline |
| Entity relationship | erDiagram |
| Class hierarchy | classDiagram |
| Git branching | gitGraph |
| Gantt / schedule | gantt |
| Pie chart | pie |
| Mindmap | mindmap |

## Common Pitfalls

- Forgetting `end` for subgraphs
- Using reserved words as node IDs
- Missing quotes around labels with special characters
- Circular references in flowcharts causing render issues
