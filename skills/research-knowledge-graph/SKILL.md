---
description: Research Knowledge Graph — navigate, visualize, trace connections between entries, manage entities, cross-workspace search.
metadata:
  platforms:
  - supported
  tags:
  - research
  - knowledge-graph
  - graph
  - entities
  - hub
  version: '1.0.0'
name: research-knowledge-graph
risk: low
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: Research Knowledge Graph — navigate, visualize, trace connections
source: local
trigger_hints:
- 知识图谱
- research graph
- 研究方向连接
- 实体管理
- research-knowledge-graph
- 研究路径追溯
- 知识关联
- 研究方向关系
- knowledge graph
- 跨工作区搜索
- 研究关系图
- kg viz
- 研究历史追溯
- barrier route
- 障碍路径
- log:neighbors
- log:viz
- log:route
- entity search
- hub search
- 连接查询
- 研究知识图谱可视化
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
| `cargo run -p research-harness --bin research-log -- neighbors <entry-id> [--relation]` | Show entries directly connected to a given entry |
| `cargo run -p research-harness --bin research-log -- path --from <id> --to <id>` | BFS shortest path between two entries |
| `cargo run -p research-harness --bin research-log -- subgraph <entry-id> [--max-depth] [--format text\|dot]` | Extract and render subgraph |
| `cargo run -p research-harness --bin research-log -- viz [--entry-id] [--max-depth] [--format]` | Visualize knowledge graph (ASCII or Graphviz DOT) |
| `cargo run -p research-harness --bin research-log -- graph-stats` | Statistics: node/edge count, density, relation breakdown |
| `cargo run -p research-harness --bin research-log -- route --barrier-id <id> [--max-depth]` | Trace full research path from a barrier report |

### Entity Management

| Command | Description |
|---------|-------------|
| `cargo run -p research-harness --bin research-log -- extract-entities <entry-id>` | Auto-extract entities (methods, datasets, metrics, models, tools) from entry text |
| `cargo run -p research-harness --bin research-log -- add-entity <name> [--kind] [--description]` | Manually add a knowledge entity |
| `cargo run -p research-harness --bin research-log -- search-entities <query> [--limit]` | FTS5 search entities by name/description |
| `cargo run -p research-harness --bin research-log -- entry-entities <entry-id>` | Show entities associated with an entry |
| `cargo run -p research-harness --bin research-log -- link-entities <entity-a> <entity-b> --relation <rel>` | Link two entities with a typed relation |

### Cross-Workspace Hub

| Command | Description |
|---------|-------------|
| `cargo run -p research-harness --bin research-log -- hub-register [--path] [--name]` | Register current workspace in the hub |
| `cargo run -p research-harness --bin research-log -- hub-index [--path]` | Index workspaces into the hub |
| `cargo run -p research-harness --bin research-log -- hub-search <query> [--limit]` | Cross-workspace search |
| `cargo run -p research-harness --bin research-log -- hub-list` | List registered workspaces |

### Autoresearch Mirror Commands

From within a research workspace, the same operations are available via:

```
cargo run -p research-harness --bin autoresearch -- log-neighbors --entry-id <id>
cargo run -p research-harness --bin autoresearch -- log-viz [--entry-id] [--max-depth]
cargo run -p research-harness --bin autoresearch -- log-route --barrier-id <id>
cargo run -p research-harness --bin autoresearch -- log-extract --entry-id <id>
cargo run -p research-harness --bin autoresearch -- log-search-entities --query <text>
```

## Quick Start

```bash
# Show your entire knowledge graph
cargo run -p research-harness --bin research-log -- viz

# Find connections to a specific entry
cargo run -p research-harness --bin research-log -- neighbors rl-20260618120000

# Discover path between two research directions
cargo run -p research-harness --bin research-log -- path --from rl-20260618100000 --to rl-20260620090000

# Extract entities from an entry
cargo run -p research-harness --bin research-log -- extract-entities rl-20260618120000

# Register in cross-workspace hub
cargo run -p research-harness --bin research-log -- hub-register
cargo run -p research-harness --bin research-log -- hub-search "transformer"
```

## Cross-References

- Research log database: `core/research-harness/src/log/`
- Research workspace CLI: `core/research-harness/src/bin/autoresearch.rs`
- Entity extraction patterns: `core/research-harness/src/log/extract.rs`
- Graph traversal: `core/research-harness/src/log/graph.rs`
- Hub indexer: `core/research-harness/src/log/hub.rs`
