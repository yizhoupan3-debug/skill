---
name: research-knowledge-graph
description: |
  Research Knowledge Graph — navigate connections between research entries,
  find paths, visualize graphs, manage entities, and search across workspaces.
  Use for requests like "show my research connections", "find path between",
  "visualize knowledge graph", "跨工作区搜索研究", "研究实体管理".
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: true
disabled-model-invocation: false
short_description: Research Knowledge Graph — navigate, visualize, trace connections
trigger_hints:
  - 知识图谱
  - research graph
  - 研究方向连接
  - 实体管理
  - 研究路径追溯
  - 知识关联
  - 研究方向关系
  - knowledge graph
  - 跨工作区搜索
  - 研究关系图
  - 连接查询
  - 研究历史追溯
  - barrier route
  - 障碍路径
trigger_hints_long: references/trigger-hints-long.md
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [research, knowledge-graph, graph, entities, hub]
risk: low
source: local
---

# Research Knowledge Graph

This skill provides CLI commands for navigating the **Research Knowledge Graph**
— the typed connections between research log entries, the entities extracted
from them (methods, datasets, metrics), and cross-workspace search via the hub.

All commands operate on the research log database at `artifacts/research-log/`.

## Commands

### Graph Navigation

| Command | Description |
|---------|-------------|
| `research-log neighbors <entry-id> [--relation]` | Show entries directly connected to a given entry |
| `research-log path --from <id> --to <id>` | BFS shortest path between two entries |
| `research-log subgraph <entry-id> [--max-depth] [--format text\|dot]` | Extract and render subgraph |
| `research-log viz [--entry-id] [--max-depth] [--format]` | Visualize knowledge graph (ASCII or Graphviz DOT) |
| `research-log graph-stats` | Statistics: node/edge count, density, relation breakdown |
| `research-log route --barrier-id <id> [--max-depth]` | Trace full research path from a barrier report |

### Entity Management

| Command | Description |
|---------|-------------|
| `research-log extract-entities <entry-id>` | Auto-extract entities (methods, datasets, metrics, models, tools) from entry text |
| `research-log add-entity <name> [--kind] [--description]` | Manually add a knowledge entity |
| `research-log search-entities <query> [--limit]` | FTS5 search entities by name/description |
| `research-log entry-entities <entry-id>` | Show entities associated with an entry |
| `research-log link-entities <entity-a> <entity-b> --relation <rel>` | Link two entities with a typed relation |

### Cross-Workspace Hub

| Command | Description |
|---------|-------------|
| `research-log hub-register [--path] [--name]` | Register current workspace in the hub |
| `research-log hub-index [--path]` | Index workspaces into the hub |
| `research-log hub-search <query> [--limit]` | Cross-workspace search |
| `research-log hub-list` | List registered workspaces |

### Autoresearch Mirror Commands

From within a research workspace, the same operations are available via:

```
autoresearch log:neighbors --entry-id <id>
autoresearch log:viz [--entry-id] [--max-depth]
autoresearch log:route --barrier-id <id>
autoresearch log:extract --entry-id <id>
autoresearch log:search-entities --query <text>
```

## Quick Start

```bash
# Show your entire knowledge graph
research-log viz

# Find connections to a specific entry
research-log neighbors rl-20260618120000

# Discover path between two research directions
research-log path --from rl-20260618100000 --to rl-20260620090000

# Extract entities from an entry
research-log extract-entities rl-20260618120000

# Register in cross-workspace hub
research-log hub-register
research-log hub-search "transformer"
```

## Cross-References

- Research log database: `core/research-harness/src/log/`
- Research workspace CLI: `core/research-harness/src/bin/autoresearch.rs`
- Harness specification: `docs/spec/research-harness.md` §19
- Entity extraction patterns: `core/research-harness/src/log/extract.rs`
- Graph traversal: `core/research-harness/src/log/graph.rs`
- Hub indexer: `core/research-harness/src/log/hub.rs`
