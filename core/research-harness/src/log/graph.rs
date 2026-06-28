// Migrated from tools/research-log-rs/src/graph.rs

//! In-memory Knowledge Graph traversal for research entries.
//!
//! Builds an adjacency map from the `connections` table and provides
//! BFS/DFS shortest-path, neighborhood, statistics, and barrier-route tracing.
//! No external graph database — pure `HashMap` + `VecDeque`.

use std::collections::{HashMap, HashSet};

use crate::log::models::*;

/// Graph edge: (neighbor_id, relation_type, weight, confidence).
type GraphEdge = (String, Option<String>, f64, Option<f64>);

/// In-memory adjacency structure loaded from the `connections` table.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KnowledgeGraph {
    /// entry_id → Vec<(neighbor_id, relation_type, weight, confidence)>
    pub adjacency: HashMap<String, Vec<GraphEdge>>,
    /// All entry IDs appearing as either side of at least one connection.
    pub nodes: HashSet<String>,
}

/// Graph statistics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub avg_degree: f64,
    pub density: f64,
    pub isolated_nodes: usize,
    pub relation_counts: HashMap<String, usize>,
}

/// Full barrier-route trace: the barrier report, its associated entries, and the subgraph.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BarrierRoute {
    pub barrier: BarrierReport,
    pub root_entries: Vec<EntryWithFindings>,
    pub subgraph: KnowledgeGraph,
}

/// An entry bundled with its findings, tags, and connections.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EntryWithFindings {
    pub entry: Entry,
    pub findings: Vec<Finding>,
    pub tags: Vec<String>,
    pub connections: Vec<LogConnection>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knowledge_graph_struct_fields() {
        let kg = KnowledgeGraph {
            adjacency: HashMap::new(),
            nodes: HashSet::new(),
        };
        assert!(kg.adjacency.is_empty());
        assert!(kg.nodes.is_empty());
    }

    #[test]
    fn graph_stats_struct() {
        let stats = GraphStats {
            node_count: 5,
            edge_count: 4,
            avg_degree: 1.6,
            density: 0.4,
            isolated_nodes: 1,
            relation_counts: HashMap::new(),
        };
        assert_eq!(stats.node_count, 5);
        assert_eq!(stats.edge_count, 4);
    }

    #[test]
    fn entry_with_findings_struct() {
        let ewf = EntryWithFindings {
            entry: Entry {
                id: "e1".into(),
                direction: "deepen".into(),
                question: "q".into(),
                context: None,
                entry_point: "cli".into(),
                barrier_id: None,
                importance: 0,
                status: "active".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
            findings: vec![],
            tags: vec!["ml".into()],
            connections: vec![],
        };
        assert_eq!(ewf.entry.id, "e1");
        assert_eq!(ewf.tags.len(), 1);
    }
}
