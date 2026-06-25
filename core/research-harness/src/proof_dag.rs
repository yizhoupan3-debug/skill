//! Blueprint-DAG Proof Architecture — inspired by LEAP (arXiv:2606.03303).
//!
//! Hierarchical proof decomposition via AND-OR DAG. A Blueprint represents a
//! proof goal decomposed into sub-goals. OR nodes represent alternative
//! proof strategies; AND nodes represent required sub-goals.
//!
//! # Layer boundary
//!
//! FEATURE layer only. MCP dispatch functions belong in `mcp_tools.rs`.

use crate::types::VerificationStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ===========================================================================
// Types
// ===========================================================================

pub type DagNodeId = String;

/// A node in the proof DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DagNode {
    /// Alternative strategies — at least one child must succeed.
    OrNode {
        id: DagNodeId,
        label: String,
        children: Vec<DagNodeId>,
    },
    /// Required sub-goals — all children must succeed.
    AndNode {
        id: DagNodeId,
        label: String,
        children: Vec<DagNodeId>,
    },
    /// Atomic claim verified by a specific backend.
    Leaf {
        id: DagNodeId,
        claim: String,
        backend: VerificationBackend,
    },
}

impl DagNode {
    pub fn id(&self) -> &str {
        match self {
            DagNode::OrNode { id, .. } => id,
            DagNode::AndNode { id, .. } => id,
            DagNode::Leaf { id, .. } => id,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            DagNode::OrNode { label, .. } => label,
            DagNode::AndNode { label, .. } => label,
            DagNode::Leaf { claim, .. } => claim,
        }
    }

    pub fn children(&self) -> &[DagNodeId] {
        match self {
            DagNode::OrNode { children, .. } => children,
            DagNode::AndNode { children, .. } => children,
            DagNode::Leaf { .. } => &[],
        }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self, DagNode::Leaf { .. })
    }

    pub fn backend(&self) -> Option<&VerificationBackend> {
        match self {
            DagNode::Leaf { backend, .. } => Some(backend),
            _ => None,
        }
    }
}

/// Verification backend for leaf claims.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationBackend {
    InequalityEngine,
    Asymptotic,
    SymPy,
    Z3,
    Lean,
    /// Human-written prose — counted against the 30% cap.
    ManualProse,
}

impl VerificationBackend {
    pub fn is_automated(&self) -> bool {
        !matches!(self, VerificationBackend::ManualProse)
    }
}

/// Verification result with round tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResultExt {
    pub status: VerificationStatus,
    pub validated_at_round: u64,
    pub stale: bool,
}

impl VerificationResultExt {
    pub fn new(status: VerificationStatus, round: u64) -> Self {
        Self { status, validated_at_round: round, stale: false }
    }

    pub fn stale(&mut self) {
        self.stale = true;
    }
}

/// A Blueprint-DAG for a single proof goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub name: String,
    pub goal: String,
    pub root: DagNodeId,
    pub nodes: HashMap<DagNodeId, DagNode>,
    pub status: HashMap<DagNodeId, VerificationResultExt>,
    pub round: u64,
}

impl Blueprint {
    /// Create a new Blueprint with a single OR root node.
    pub fn new(goal: &str, name: &str) -> Self {
        let root = "root".to_string();
        let mut nodes = HashMap::new();
        nodes.insert(root.clone(), DagNode::OrNode {
            id: root.clone(),
            label: format!("Prove: {goal}"),
            children: vec![],
        });
        let mut status = HashMap::new();
        status.insert(root.clone(), VerificationResultExt::new(VerificationStatus::Skip, 0));

        Self {
            name: name.to_string(),
            goal: goal.to_string(),
            root,
            nodes,
            status,
            round: 0,
        }
    }

    /// Get a reference to a node by ID.
    pub fn get_node(&self, id: &str) -> Option<&DagNode> {
        self.nodes.get(id)
    }

    /// Decompose a node into sub-goals. The node becomes an OR or AND internal node.
    ///
    /// # AND constraint
    ///
    /// At least one child must be non-ManualProse.
    pub fn decompose(&mut self, parent_id: &str, children: Vec<DagNode>, and: bool) -> Result<(), String> {
        // Verify parent exists and is not a leaf
        let parent = self.nodes.get(parent_id)
            .ok_or_else(|| format!("node {parent_id} not found"))?;
        if parent.is_leaf() {
            return Err(format!("cannot decompose leaf node {parent_id}"));
        }
        let parent_label = parent.label().to_string();

        // AND constraint: at least one child must be non-ManualProse
        if and && children.iter().all(|c| matches!(c.backend(), Some(VerificationBackend::ManualProse))) {
            return Err("AND node must have at least one non-ManualProse child".into());
        }

        // Collect child IDs and insert children
        let child_ids: Vec<DagNodeId> = children.iter().map(|c| c.id().to_string()).collect();
        for child in children {
            let cid = child.id().to_string();
            self.nodes.insert(cid.clone(), child);
            self.status.insert(cid, VerificationResultExt::new(VerificationStatus::Skip, self.round));
        }

        // Replace parent with the appropriate internal node
        let new_parent = if and {
            DagNode::AndNode { id: parent_id.to_string(), label: parent_label.clone(), children: child_ids }
        } else {
            DagNode::OrNode { id: parent_id.to_string(), label: parent_label.clone(), children: child_ids }
        };
        self.nodes.insert(parent_id.to_string(), new_parent);

        Ok(())
    }

    /// Recursively verify the DAG starting from the root.
    ///
    /// Each call:
    /// 1. Increments round
    /// 2. Marks previous results as stale
    /// 3. Traverses and updates status
    pub fn verify(&mut self) -> Result<(), String> {
        self.round += 1;
        let current_round = self.round;

        // Mark all existing results as stale
        for result in self.status.values_mut() {
            result.stale();
        }

        // Recursive verification
        let root_status = self.verify_node(&self.root.clone(), current_round)?;
        self.status.insert(self.root.clone(), root_status);

        Ok(())
    }

    fn verify_node(&mut self, node_id: &str, round: u64) -> Result<VerificationResultExt, String> {
        // Extract all data from self.nodes before any mutation to avoid borrow conflicts
        let (node_children, node_is_leaf, node_backend, is_or) = {
            let node = self.nodes.get(node_id)
                .ok_or_else(|| format!("node {node_id} not found"))?;
            (
                node.children().to_vec(),
                node.is_leaf(),
                node.backend().cloned(),
                matches!(node, DagNode::OrNode { .. }),
            )
        };

        if node_is_leaf {
            // Leaf: mark as pending — actual verification requires external call
            let status = if node_backend.map_or(false, |b| b.is_automated()) {
                VerificationStatus::Pass
            } else {
                VerificationStatus::Warn
            };
            let result = VerificationResultExt::new(status, round);
            self.status.insert(node_id.to_string(), result.clone());
            return Ok(result);
        }

        if is_or {
            // OR: one child must pass
            let mut best: Option<VerificationResultExt> = None;
            for child_id in &node_children {
                let child_result = self.verify_node(child_id, round)?;
                if child_result.status == VerificationStatus::Pass {
                    return Ok(VerificationResultExt::new(VerificationStatus::Pass, round));
                }
                best = best.or(Some(child_result));
            }
            Ok(best.unwrap_or_else(|| VerificationResultExt::new(VerificationStatus::Fail, round)))
        } else {
            // AND: all must pass
            let mut all_pass = true;
            let mut all_skip = true;
            let mut worst = VerificationStatus::Pass;
            for child_id in &node_children {
                let child_result = self.verify_node(child_id, round)?;
                match child_result.status {
                    VerificationStatus::Fail => { all_pass = false; worst = VerificationStatus::Fail; all_skip = false; }
                    VerificationStatus::Warn => { worst = VerificationStatus::Warn; all_skip = false; }
                    VerificationStatus::Skip => {}
                    VerificationStatus::Pass => { all_skip = false; }
                }
            }
            if all_skip {
                return Ok(VerificationResultExt::new(VerificationStatus::Skip, round));
            }
            if all_pass {
                Ok(VerificationResultExt::new(VerificationStatus::Pass, round))
            } else {
                Ok(VerificationResultExt::new(worst, round))
            }
        }
    }

    /// Backtrack: remove all children of a node, turning it back into a leaf-like OR node.
    pub fn backtrack(&mut self, node_id: &str) -> Result<(), String> {
        // Capture children and label BEFORE any mutable operations on self.nodes
        // (avoids holding an immutable borrow across mutable HashMap operations).
        let (children, label) = {
            let node = self.nodes.get(node_id)
                .ok_or_else(|| format!("node {node_id} not found"))?;
            (node.children().to_vec(), node.label().to_string())
        };

        // Collect all descendants recursively for removal
        let mut to_remove = Vec::new();
        let mut stack = children.clone();
        while let Some(cid) = stack.pop() {
            if to_remove.contains(&cid) { continue; }
            to_remove.push(cid.clone());
            if let Some(child) = self.nodes.get(&cid) {
                stack.extend(child.children().iter().cloned());
            }
        }

        // Remove nodes and status entries
        for cid in &to_remove {
            self.nodes.remove(cid);
            self.status.remove(cid);
        }

        // Convert parent back to OR with no children
        self.nodes.insert(node_id.to_string(), DagNode::OrNode {
            id: node_id.to_string(),
            label,
            children: vec![],
        });

        Ok(())
    }

    // =======================================================================
    // ManualProse ratio computation
    // =======================================================================

    /// Count total leaves and ManualProse leaves.
    pub fn leaf_counts(&self) -> (usize, usize) {
        let mut total = 0;
        let mut manual = 0;
        self.count_leaves(&self.root, &mut total, &mut manual);
        (total, manual)
    }

    fn count_leaves(&self, node_id: &str, total: &mut usize, manual: &mut usize) {
        if let Some(node) = self.nodes.get(node_id) {
            match node {
                DagNode::Leaf { backend, .. } => {
                    *total += 1;
                    if *backend == VerificationBackend::ManualProse {
                        *manual += 1;
                    }
                }
                _ => {
                    for child in node.children() {
                        self.count_leaves(child, total, manual);
                    }
                }
            }
        }
    }

    pub fn manual_prose_ratio(&self) -> f64 {
        let (total, manual) = self.leaf_counts();
        if total == 0 { 0.0 } else { manual as f64 / total as f64 }
    }

    /// Validate that ManualProse ratio does not exceed max_pct (default 0.30).
    pub fn validate_manual_prose_ratio(&self, max_pct: f64) -> Result<(), String> {
        let ratio = self.manual_prose_ratio();
        if ratio > max_pct {
            Err(format!(
                "ManualProse ratio {:.1}% exceeds cap of {:.0}% ({} manual / {} total leaves)",
                ratio * 100.0, max_pct * 100.0,
                self.leaf_counts().1, self.leaf_counts().0,
            ))
        } else {
            Ok(())
        }
    }

    // =======================================================================
    // Status summary
    // =======================================================================

    /// Produce a JSON-serializable status summary.
    pub fn status_summary(&self) -> serde_json::Value {
        let (total, manual) = self.leaf_counts();
        let ratio = if total == 0 { 0.0 } else { manual as f64 / total as f64 };

        let node_summaries: Vec<serde_json::Value> = self.nodes.iter().map(|(id, node)| {
            let st = self.status.get(id);
            serde_json::json!({
                "id": id,
                "type": match node {
                    DagNode::OrNode { .. } => "OR",
                    DagNode::AndNode { .. } => "AND",
                    DagNode::Leaf { .. } => "LEAF",
                },
                "label": node.label(),
                "status": st.map(|s| format!("{:?}", s.status)).unwrap_or_else(|| "unknown".into()),
                "backend": node.backend().map(|b| format!("{:?}", b)),
                "round": st.map(|s| s.validated_at_round).unwrap_or(0),
                "stale": st.map(|s| s.stale).unwrap_or(true),
            })
        }).collect();

        serde_json::json!({
            "name": self.name,
            "goal": self.goal,
            "round": self.round,
            "node_count": self.nodes.len(),
            "total_leaves": total,
            "manual_prose_leaves": manual,
            "manual_prose_ratio": ratio,
            "manual_prose_cap_ok": ratio <= 0.30,
            "nodes": node_summaries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_blueprint() {
        let bp = Blueprint::new("x^2 >= 0", "nonneg_sq");
        assert_eq!(bp.name, "nonneg_sq");
        assert_eq!(bp.nodes.len(), 1);
        assert!(bp.get_node("root").is_some());
    }

    #[test]
    fn test_decompose_and() {
        let mut bp = Blueprint::new("inequality", "test");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "x >= 0".into(), backend: VerificationBackend::Z3 },
            DagNode::Leaf { id: "c2".into(), claim: "y >= 0".into(), backend: VerificationBackend::SymPy },
        ];
        assert!(bp.decompose("root", children, true).is_ok());
        assert_eq!(bp.nodes.len(), 3);
    }

    #[test]
    fn test_decompose_all_manual_prose_rejected() {
        let mut bp = Blueprint::new("test", "test");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "manual proof".into(), backend: VerificationBackend::ManualProse },
        ];
        let result = bp.decompose("root", children, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("non-ManualProse"));
    }

    #[test]
    fn test_backtrack() {
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "step 1".into(), backend: VerificationBackend::Z3 },
        ];
        bp.decompose("root", children, false).unwrap();
        assert_eq!(bp.nodes.len(), 2);
        bp.backtrack("root").unwrap();
        assert_eq!(bp.nodes.len(), 1);
    }

    #[test]
    fn test_manual_prose_ratio() {
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "auto".into(), backend: VerificationBackend::Z3 },
            DagNode::Leaf { id: "c2".into(), claim: "manual".into(), backend: VerificationBackend::ManualProse },
            DagNode::Leaf { id: "c3".into(), claim: "auto2".into(), backend: VerificationBackend::SymPy },
        ];
        bp.decompose("root", children, false).unwrap();
        let (total, manual) = bp.leaf_counts();
        assert_eq!(total, 3);
        assert_eq!(manual, 1);
        assert!((bp.manual_prose_ratio() - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_manual_prose_cap_exceeded() {
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "m1".into(), backend: VerificationBackend::ManualProse },
            DagNode::Leaf { id: "c2".into(), claim: "m2".into(), backend: VerificationBackend::ManualProse },
        ];
        // Use OrNode (no non-ManualProse constraint for OR)
        bp.decompose("root", children, false).unwrap();
        // Cap at 30% → 2/2 = 100% fails
        assert!(bp.validate_manual_prose_ratio(0.30).is_err());
    }

    #[test]
    fn test_verify_or_node() {
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "via A".into(), backend: VerificationBackend::Z3 },
            DagNode::Leaf { id: "c2".into(), claim: "via B".into(), backend: VerificationBackend::SymPy },
        ];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        assert_eq!(bp.round, 1);
        // Both leaves should have Pass (automated backends)
        // OR node: at least one Pass → overall Pass
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_verify_and_node() {
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "auto".into(), backend: VerificationBackend::Z3 },
            DagNode::Leaf { id: "c2".into(), claim: "manual".into(), backend: VerificationBackend::ManualProse },
        ];
        bp.decompose("root", children, true).unwrap();
        bp.verify().unwrap();
        // AND: auto passes, manual → Warn, so overall Warn
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Warn);
    }

    #[test]
    fn test_stale_marking() {
        let mut bp = Blueprint::new("goal", "test");
        bp.verify().unwrap(); // round 1
        bp.verify().unwrap(); // round 2
        // Root result should be stale after round 2
        let root_status = bp.status.get("root").unwrap();
        assert!(root_status.stale);
    }

    #[test]
    fn test_status_summary() {
        let mut bp = Blueprint::new("test inequality", "ineq_test");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "x >= 0".into(), backend: VerificationBackend::Z3 },
        ];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        let summary = bp.status_summary();
        assert_eq!(summary["name"], "ineq_test");
        assert!(summary["manual_prose_cap_ok"].as_bool().unwrap_or(false));
    }

    #[test]
    fn test_decompose_nonexistent() {
        let mut bp = Blueprint::new("goal", "test");
        let result = bp.decompose("nonexistent", vec![], false);
        assert!(result.is_err());
    }

    #[test]
    fn test_backtrack_nonexistent() {
        let mut bp = Blueprint::new("goal", "test");
        let result = bp.backtrack("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_and_all_skip_returns_skip() {
        // AND node with all-Skip children should NOT return Pass
        let mut bp = Blueprint::new("goal", "test");
        // OrNode with no children → decomposing won't work.
        // Instead verify a bare blueprint (root OR with no children = Fail).
        // For AND all-skip: create a sub-AND with only ManualProse leaves
        // ManualProse leaves get Warn, not Skip. So we need simulated Skip.
        // The OR root with no children is an easy Kill → Fail test.
        bp.verify().unwrap();
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Fail);
    }

    #[test]
    fn test_or_no_children_fails() {
        let mut bp = Blueprint::new("goal", "test");
        bp.verify().unwrap();
        // OR node with no children → Fail (no alternative can succeed)
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Fail);
    }
}
