// Migrated from tools/research-log-rs/src/graph.rs

//! In-memory Knowledge Graph traversal for research entries.
//!
//! Builds an adjacency map from the `connections` table and provides
//! BFS/DFS shortest-path, neighborhood, statistics, and barrier-route tracing.
//! No external graph database — pure `HashMap` + `VecDeque`.

use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::Connection;

use crate::log::db;
use crate::log::models::*;

/// In-memory adjacency structure loaded from the `connections` table.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KnowledgeGraph {
    /// entry_id → Vec<(neighbor_id, relation_type, weight, confidence)>
    pub adjacency: HashMap<String, Vec<(String, Option<String>, f64, Option<f64>)>>,
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

// ── Graph loading ──

/// Load the full graph from all connections in the database.
pub fn load_full_graph(conn: &Connection) -> Result<KnowledgeGraph, anyhow::Error> {
    let connections = db::get_all_connections(conn)?;
    build_graph(&connections)
}

/// Load a subgraph centered on `entry_id` up to `max_depth` hops.
pub fn load_subgraph(
    conn: &Connection,
    center: &str,
    max_depth: usize,
) -> Result<KnowledgeGraph, anyhow::Error> {
    let all_connections = db::get_all_connections(conn)?;
    // Find reachable node IDs via BFS
    let reachable = bfs_node_set(&all_connections, center, max_depth);
    // Filter connections to only those where both ends are reachable
    let filtered: Vec<LogConnection> = all_connections
        .into_iter()
        .filter(|c| reachable.contains(&c.entry_id_a) && reachable.contains(&c.entry_id_b))
        .collect();
    build_graph(&filtered)
}

/// Build a KnowledgeGraph from a list of connections.
fn build_graph(connections: &[LogConnection]) -> Result<KnowledgeGraph, anyhow::Error> {
    let mut adjacency: HashMap<String, Vec<(String, Option<String>, f64, Option<f64>)>> =
        HashMap::new();
    let mut nodes = HashSet::new();

    for c in connections {
        nodes.insert(c.entry_id_a.clone());
        nodes.insert(c.entry_id_b.clone());

        // A → B
        adjacency
            .entry(c.entry_id_a.clone())
            .or_default()
            .push((
                c.entry_id_b.clone(),
                c.relation.clone(),
                c.weight,
                c.confidence,
            ));

        // B → A (undirected traversal)
        adjacency
            .entry(c.entry_id_b.clone())
            .or_default()
            .push((
                c.entry_id_a.clone(),
                c.relation.clone(),
                c.weight,
                c.confidence,
            ));
    }

    Ok(KnowledgeGraph { adjacency, nodes })
}

// ── BFS helpers ──

/// BFS to find all node IDs reachable within `max_depth` from `start`.
fn bfs_node_set(
    connections: &[LogConnection],
    start: &str,
    max_depth: usize,
) -> HashSet<String> {
    // Build temporary adjacency for BFS
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for c in connections {
        adj.entry(c.entry_id_a.as_str())
            .or_default()
            .push(c.entry_id_b.as_str());
        adj.entry(c.entry_id_b.as_str())
            .or_default()
            .push(c.entry_id_a.as_str());
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start.to_string());
    queue.push_back((start, 0usize));

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if let Some(neighbors) = adj.get(node) {
            for &nbor in neighbors {
                if visited.insert(nbor.to_string()) {
                    queue.push_back((nbor, depth + 1));
                }
            }
        }
    }
    visited
}

// ── Query API ──

/// Get direct neighbors of an entry, optionally filtered by relation type.
pub fn get_neighbors<'a>(
    graph: &'a KnowledgeGraph,
    entry_id: &str,
    relation_filter: Option<&[&str]>,
) -> Vec<(&'a String, Option<&'a str>, f64, Option<f64>)> {
    let Some(edges) = graph.adjacency.get(entry_id) else {
        return vec![];
    };

    edges
        .iter()
        .filter(|(_, rel, _, _)| {
            relation_filter.map_or(true, |allowed| {
                rel.as_deref().map_or(false, |r| allowed.contains(&r))
            })
        })
        .map(|(nid, rel, w, conf)| (nid, rel.as_deref(), *w, *conf))
        .collect()
}

/// BFS shortest path between two entries.
/// Returns `Some(vec![(node, relation_from_parent, weight), ...])` or `None`.
pub fn find_path(
    graph: &KnowledgeGraph,
    from: &str,
    to: &str,
    max_depth: usize,
) -> Option<Vec<(String, Option<String>, f64)>> {
    if from == to {
        return Some(vec![(from.to_string(), None, 1.0)]);
    }

    let mut visited: HashSet<&str> = HashSet::new();
    // parent map: child → (parent, relation, weight, depth)
    let mut parent: HashMap<&str, (&str, Option<String>, f64, usize)> = HashMap::new();
    let mut queue = VecDeque::new();

    visited.insert(from);
    queue.push_back((from, 0usize));

    while let Some((current, depth)) = queue.pop_front() {
        if current == to {
            break;
        }
        if depth >= max_depth {
            continue;
        }
        if let Some(edges) = graph.adjacency.get(current) {
            for (nbor, rel, w, _) in edges {
                if visited.insert(nbor.as_str()) {
                    parent.insert(
                        nbor.as_str(),
                        (current, rel.clone(), *w, depth + 1),
                    );
                    queue.push_back((nbor.as_str(), depth + 1));
                }
            }
        }
    }

    if !parent.contains_key(to) && from != to {
        return None;
    }

    // Reconstruct path backwards
    let mut path = Vec::new();
    let mut current = to;
    while let Some(&(par, ref rel, w, _)) = parent.get(current) {
        path.push((current.to_string(), rel.clone(), w));
        current = par;
    }
    path.push((from.to_string(), None, 1.0));
    path.reverse();
    Some(path)
}

/// BFS traversal from `start` with depth limit.
pub fn bfs_traverse(
    graph: &KnowledgeGraph,
    start: &str,
    max_depth: usize,
    relation_filter: Option<&[&str]>,
) -> Vec<(String, Option<String>, f64, usize)> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut result = Vec::new();
    let mut queue = VecDeque::new();

    visited.insert(start.to_string());
    queue.push_back((start.to_string(), 0usize));

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if let Some(edges) = graph.adjacency.get(&node) {
            for (nbor, rel, w, _) in edges {
                let filtered = relation_filter.map_or(true, |allowed| {
                    rel.as_deref().map_or(false, |r| allowed.contains(&r))
                });
                if !filtered {
                    continue;
                }
                if visited.insert(nbor.clone()) {
                    result.push((nbor.clone(), rel.clone(), *w, depth + 1));
                    queue.push_back((nbor.clone(), depth + 1));
                }
            }
        }
    }

    result
}

/// DFS traversal (pre-order) from `start` with depth limit.
pub fn dfs_traverse(
    graph: &KnowledgeGraph,
    start: &str,
    max_depth: usize,
) -> Vec<String> {
    let mut visited = HashSet::new();
    let mut result = Vec::new();
    let mut stack = vec![(start.to_string(), 0usize)];

    while let Some((node, depth)) = stack.pop() {
        if !visited.insert(node.clone()) {
            continue;
        }
        result.push(node.clone());
        if depth >= max_depth {
            continue;
        }
        if let Some(edges) = graph.adjacency.get(&node) {
            for (nbor, _, _, _) in edges.iter().rev() {
                if !visited.contains(nbor.as_str()) {
                    stack.push((nbor.clone(), depth + 1));
                }
            }
        }
    }

    result
}

/// Compute graph statistics.
pub fn get_graph_stats(graph: &KnowledgeGraph) -> GraphStats {
    let node_count = graph.nodes.len();
    let edge_count: usize = graph.adjacency.values().map(|v| v.len()).sum::<usize>() / 2; // undirected
    let avg_degree = if node_count > 0 {
        (edge_count as f64 * 2.0) / node_count as f64
    } else {
        0.0
    };
    let max_possible_edges = if node_count > 1 {
        node_count * (node_count - 1) / 2
    } else {
        1
    };
    let density = if max_possible_edges > 0 {
        edge_count as f64 / max_possible_edges as f64
    } else {
        0.0
    };

    let mut isolated_nodes = 0;
    for node in &graph.nodes {
        let deg = graph
            .adjacency
            .get(node)
            .map_or(0, |v| v.len());
        if deg == 0 {
            isolated_nodes += 1;
        }
    }

    let mut relation_counts: HashMap<String, usize> = HashMap::new();
    for (node, edges) in &graph.adjacency {
        for (nbor, rel, _, _) in edges {
            // 只统计 node < nbor 方向的边，避免双向存储造成的双倍计数
            if node < nbor {
                if let Some(r) = rel {
                    *relation_counts.entry(r.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    GraphStats {
        node_count,
        edge_count,
        avg_degree,
        density,
        isolated_nodes,
        relation_counts,
    }
}

/// Trace the full research path from a barrier report.
///
/// Returns the barrier report, all entries associated with it, and the
/// subgraph centered on those entries.
pub fn trace_barrier_route(
    conn: &Connection,
    barrier_id: &str,
    max_depth: usize,
) -> Result<BarrierRoute, anyhow::Error> {
    // Find the barrier report
    let mut stmt = conn.prepare_cached(
        "SELECT barrier_id, entry_id, loop_id, report, created_at
         FROM barrier_reports WHERE barrier_id=?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![barrier_id])?;
    let barrier = match rows.next()? {
        Some(row) => BarrierReport {
            barrier_id: row.get(0)?,
            entry_id: row.get(1)?,
            loop_id: row.get(2)?,
            report: row.get(3)?,
            created_at: row.get(4)?,
        },
        None => anyhow::bail!("Barrier report not found: {}", barrier_id),
    };

    // Collect root entries associated with this barrier
    let mut root_entry_ids = Vec::new();
    if let Some(ref eid) = barrier.entry_id {
        root_entry_ids.push(eid.clone());
    }
    // Also search for entries whose barrier_id matches
    {
        let mut stmt2 = conn.prepare_cached(
            "SELECT id FROM entries WHERE barrier_id=?1 AND id != ?2",
        )?;
        let barrier_entry = barrier.entry_id.as_deref().unwrap_or("");
        let mut rows2 = stmt2.query(rusqlite::params![barrier_id, barrier_entry])?;
        while let Some(row) = rows2.next()? {
            root_entry_ids.push(row.get::<_, String>(0)?);
        }
    }

    // Load the subgraph rooted at the barrier entries
    let all_connections = db::get_all_connections(conn)?;
    let mut all_reachable = HashSet::new();
    for rid in &root_entry_ids {
        let reachable = bfs_node_set(&all_connections, rid, max_depth);
        all_reachable.extend(reachable);
    }
    // Also include the barrier's own entry
    if let Some(ref eid) = barrier.entry_id {
        all_reachable.insert(eid.clone());
    }

    let filtered: Vec<LogConnection> = all_connections
        .into_iter()
        .filter(|c| all_reachable.contains(&c.entry_id_a) && all_reachable.contains(&c.entry_id_b))
        .collect();
    let subgraph = build_graph(&filtered)?;

    // Load full entry data for root entries
    let mut root_entries = Vec::new();
    for rid in root_entry_ids {
        if let Some(entry) = db::get_entry(conn, &rid)? {
            let findings = db::get_findings(conn, &rid)?;
            let tags = db::get_tags(conn, &rid)?;
            let connections = db::get_connections_for_entry(conn, &rid)?;
            root_entries.push(EntryWithFindings {
                entry,
                findings,
                tags,
                connections,
            });
        }
    }

    Ok(BarrierRoute {
        barrier,
        root_entries,
        subgraph,
    })
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
                id: "e1".into(), direction: "deepen".into(), question: "q".into(),
                context: None, entry_point: "cli".into(), barrier_id: None,
                importance: 0, status: "active".into(),
                created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-01T00:00:00Z".into(),
            },
            findings: vec![],
            tags: vec!["ml".into()],
            connections: vec![],
        };
        assert_eq!(ewf.entry.id, "e1");
        assert_eq!(ewf.tags.len(), 1);
    }
}
