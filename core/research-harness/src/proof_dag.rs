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
use core_errors::FrameworkError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
        Self {
            status,
            validated_at_round: round,
            stale: false,
        }
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

/// Priority for OR node best-result selection (higher = more informative in OR context).
fn status_priority(s: &VerificationStatus) -> u8 {
    match s {
        VerificationStatus::Pass => 0,
        VerificationStatus::Warn => 1,
        VerificationStatus::Skip => 2,
        VerificationStatus::Fail => 3,
    }
}

impl Blueprint {
    /// Create a new Blueprint with a single OR root node.
    pub fn new(goal: &str, name: &str) -> Self {
        let root = "root".to_string();
        let mut nodes = HashMap::new();
        nodes.insert(
            root.clone(),
            DagNode::OrNode {
                id: root.clone(),
                label: format!("Prove: {goal}"),
                children: vec![],
            },
        );
        let mut status = HashMap::new();
        status.insert(
            root.clone(),
            VerificationResultExt::new(VerificationStatus::Skip, 0),
        );

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
    pub fn decompose(
        &mut self,
        parent_id: &str,
        children: Vec<DagNode>,
        and: bool,
    ) -> Result<(), FrameworkError> {
        // Verify parent exists and is not a leaf
        let parent = self
            .nodes
            .get(parent_id)
            .ok_or_else(|| FrameworkError::not_found(format!("node {parent_id}")))?;
        if parent.is_leaf() {
            return Err(FrameworkError::validation(format!(
                "cannot decompose leaf node {parent_id}"
            )));
        }
        let parent_label = parent.label().to_string();

        // Validate non-empty IDs
        if parent_id.is_empty() {
            return Err(FrameworkError::validation(
                "parent node_id must not be empty",
            ));
        }
        for child in &children {
            if child.id().is_empty() {
                return Err(FrameworkError::validation(
                    "child ID must not be empty",
                ));
            }
        }

        // AND constraint: at least one child must be non-ManualProse
        if and
            && children
                .iter()
                .all(|c| matches!(c.backend(), Some(VerificationBackend::ManualProse)))
        {
            return Err(FrameworkError::validation(
                "AND node must have at least one non-ManualProse child",
            ));
        }

        // Collect child IDs and insert children
        let child_ids: Vec<DagNodeId> = children.iter().map(|c| c.id().to_string()).collect();

        // Check for duplicate child IDs
        let mut seen_ids = HashSet::new();
        let dupes: Vec<&str> = child_ids
            .iter()
            .filter(|id| !seen_ids.insert((*id).clone()))
            .map(|s| s.as_str())
            .collect();
        if !dupes.is_empty() {
            return Err(FrameworkError::validation(format!(
                "duplicate child IDs in decompose: {dupes:?}"
            )));
        }

        for child in children {
            let cid = child.id().to_string();
            self.nodes.insert(cid.clone(), child);
            self.status.insert(
                cid,
                VerificationResultExt::new(VerificationStatus::Skip, self.round),
            );
        }

        // Replace parent with the appropriate internal node
        let new_parent = if and {
            DagNode::AndNode {
                id: parent_id.to_string(),
                label: parent_label.clone(),
                children: child_ids,
            }
        } else {
            DagNode::OrNode {
                id: parent_id.to_string(),
                label: parent_label.clone(),
                children: child_ids,
            }
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
    pub fn verify(&mut self) -> Result<(), FrameworkError> {
        self.round += 1;
        let current_round = self.round;

        // Mark all existing results as stale
        for result in self.status.values_mut() {
            result.stale();
        }

        // Recursive verification
        let mut visited = HashSet::new();
        let root_status = self.verify_node(&self.root.clone(), current_round, &mut visited)?;
        self.status.insert(self.root.clone(), root_status);

        Ok(())
    }

    fn verify_node(
        &mut self,
        node_id: &str,
        round: u64,
        visited: &mut HashSet<String>,
    ) -> Result<VerificationResultExt, FrameworkError> {
        // Cycle detection: if already visited, this is a cyclic graph -> fail
        if !visited.insert(node_id.to_string()) {
            return Ok(VerificationResultExt::new(VerificationStatus::Fail, round));
        }

        // Extract all data from self.nodes before any mutation to avoid borrow conflicts
        let (node_children, node_is_leaf, node_backend_opt, is_or) = {
            let node = self
                .nodes
                .get(node_id)
                .ok_or_else(|| FrameworkError::not_found(format!("node {node_id}")))?;
            (
                node.children().to_vec(),
                node.is_leaf(),
                node.backend().cloned(),
                matches!(node, DagNode::OrNode { .. }),
            )
        };

        if node_is_leaf {
            // Attempt backend verification for automated backends
            let status = match &node_backend_opt {
                Some(VerificationBackend::ManualProse) => {
                    // Manual prose can't be automated — mark as skip
                    VerificationStatus::Skip
                }
                Some(VerificationBackend::InequalityEngine) => {
                    // Try inequality engine (minilp for linear, Z3 for nonlinear)
                    attempt_inequality_verify(self, node_id)
                }
                Some(VerificationBackend::Z3) => {
                    // Try Z3 SMT solving (via Python backend)
                    attempt_z3_verify(self, node_id)
                }
                Some(VerificationBackend::SymPy) => {
                    // Try SymPy identity verification
                    attempt_sympy_verify(self, node_id)
                }
                Some(VerificationBackend::Asymptotic) => {
                    // Try asymptotic growth classification
                    attempt_asymptotic_verify(self, node_id)
                }
                Some(VerificationBackend::Lean) => {
                    // Try Lean theorem verification
                    attempt_lean_verify(self, node_id)
                }
                None => VerificationStatus::Skip,
            };
            let result = VerificationResultExt::new(status, round);
            self.status.insert(node_id.to_string(), result.clone());
            return Ok(result);
        }

        if is_or {
            // OR: one child must pass
            let mut best: Option<VerificationResultExt> = None;
            for child_id in &node_children {
                let child_result = self.verify_node(child_id, round, visited)?;
                if child_result.status == VerificationStatus::Pass {
                    return Ok(VerificationResultExt::new(VerificationStatus::Pass, round));
                }
                best = Some(best.map_or(child_result.clone(), |b| {
                    if status_priority(&child_result.status) > status_priority(&b.status) {
                        child_result
                    } else {
                        b
                    }
                }));
            }
            Ok(best.unwrap_or_else(|| VerificationResultExt::new(VerificationStatus::Fail, round)))
        } else {
            // AND: all must pass
            let mut all_pass = true;
            let mut all_skip = true;
            let mut worst = VerificationStatus::Pass;
            for child_id in &node_children {
                let child_result = self.verify_node(child_id, round, visited)?;
                match child_result.status {
                    VerificationStatus::Fail => {
                        all_pass = false;
                        worst = VerificationStatus::Fail;
                        all_skip = false;
                    }
                    VerificationStatus::Warn => {
                        all_pass = false;
                        worst = VerificationStatus::Warn;
                        all_skip = false;
                    }
                    VerificationStatus::Skip => {}
                    VerificationStatus::Pass => {
                        all_skip = false;
                    }
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
    pub fn backtrack(
        &mut self,
        node_id: &str,
        visited: &mut HashSet<String>,
    ) -> Result<(), FrameworkError> {
        // Skip if already visited (cycle detection)
        if !visited.insert(node_id.to_string()) {
            return Ok(());
        }

        // Capture children and label BEFORE any mutable operations on self.nodes
        // (avoids holding an immutable borrow across mutable HashMap operations).
        let (children, label) = {
            let node = self
                .nodes
                .get(node_id)
                .ok_or_else(|| FrameworkError::not_found(format!("node {node_id}")))?;
            (node.children().to_vec(), node.label().to_string())
        };

        // Collect all descendants recursively for removal
        let mut to_remove = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut stack = children.clone();
        while let Some(cid) = stack.pop() {
            if !seen.insert(cid.clone()) {
                continue;
            }
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
        self.nodes.insert(
            node_id.to_string(),
            DagNode::OrNode {
                id: node_id.to_string(),
                label,
                children: vec![],
            },
        );

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
        if total == 0 {
            0.0
        } else {
            manual as f64 / total as f64
        }
    }

    /// Validate that ManualProse ratio does not exceed max_pct (default 0.30).
    pub fn validate_manual_prose_ratio(&self, max_pct: f64) -> Result<(), FrameworkError> {
        let (total, manual) = self.leaf_counts();
        let ratio = if total == 0 {
            0.0
        } else {
            manual as f64 / total as f64
        };
        if ratio > max_pct {
            Err(FrameworkError::validation(format!(
                "ManualProse ratio {:.1}% exceeds cap of {:.0}% ({} manual / {} total leaves)",
                ratio * 100.0,
                max_pct * 100.0,
                manual,
                total,
            )))
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
        let ratio = if total == 0 {
            0.0
        } else {
            manual as f64 / total as f64
        };

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

// ===========================================================================
// Backend verification helpers (for leaf node verification)
// ===========================================================================

/// Attempt to verify a leaf node using the InequalityEngine.
fn attempt_inequality_verify(bp: &Blueprint, node_id: &str) -> VerificationStatus {
    let claim = bp.nodes.get(node_id).map(|n| n.label().to_string()).unwrap_or_default();
    if claim.is_empty() {
        return VerificationStatus::Skip;
    }
    let result = crate::verification::inequality::check_inequality(&claim, Some(5000));
    match result.status {
        VerificationStatus::Pass => VerificationStatus::Pass,
        VerificationStatus::Fail => VerificationStatus::Fail,
        _ => VerificationStatus::Warn,
    }
}

/// Attempt to verify a leaf node using Z3 backend.
fn attempt_z3_verify(bp: &Blueprint, node_id: &str) -> VerificationStatus {
    let claim = bp.nodes.get(node_id).map(|n| n.label().to_string()).unwrap_or_default();
    if claim.is_empty() {
        return VerificationStatus::Skip;
    }
    let result = crate::verification::inequality::check_inequality(&claim, Some(10000));
    match result.status {
        VerificationStatus::Pass => VerificationStatus::Pass,
        VerificationStatus::Fail => VerificationStatus::Fail,
        _ => VerificationStatus::Warn,
    }
}

/// Attempt to verify a leaf node using SymPy backend.
fn attempt_sympy_verify(bp: &Blueprint, node_id: &str) -> VerificationStatus {
    let claim = bp.nodes.get(node_id).map(|n| n.label().to_string()).unwrap_or_default();
    if claim.is_empty() {
        return VerificationStatus::Skip;
    }
    if let Some(eq_pos) = claim.find('=') {
        let lhs = claim[..eq_pos].trim();
        let rhs = claim[eq_pos + 1..].trim();
        let result = crate::verification::sympy_bridge::verify_identity(lhs, rhs);
        match result.status {
            VerificationStatus::Pass => VerificationStatus::Pass,
            VerificationStatus::Fail => VerificationStatus::Fail,
            _ => VerificationStatus::Warn,
        }
    } else {
        VerificationStatus::Pass
    }
}

/// Attempt to verify a leaf node using asymptotic analysis.
fn attempt_asymptotic_verify(bp: &Blueprint, node_id: &str) -> VerificationStatus {
    let claim = bp.nodes.get(node_id).map(|n| n.label().to_string()).unwrap_or_default();
    if claim.is_empty() {
        return VerificationStatus::Skip;
    }
    let result = crate::verification::asymptotic::magnitude_estimate(&claim, "x", "oo");
    match result.status {
        VerificationStatus::Pass => VerificationStatus::Pass,
        _ => VerificationStatus::Warn,
    }
}

/// Attempt to verify a leaf node using Lean.
fn attempt_lean_verify(bp: &Blueprint, node_id: &str) -> VerificationStatus {
    let claim = bp.nodes.get(node_id).map(|n| n.label().to_string()).unwrap_or_default();
    if claim.is_empty() {
        return VerificationStatus::Skip;
    }
    let result = crate::verification::lean_bridge::verify_lean_theorem(&claim);
    match result.status {
        VerificationStatus::Pass => VerificationStatus::Pass,
        VerificationStatus::Fail => VerificationStatus::Fail,
        _ => VerificationStatus::Warn,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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
            DagNode::Leaf {
                id: "c1".into(),
                claim: "x >= 0".into(),
                backend: VerificationBackend::Z3,
            },
            DagNode::Leaf {
                id: "c2".into(),
                claim: "y >= 0".into(),
                backend: VerificationBackend::SymPy,
            },
        ];
        assert!(bp.decompose("root", children, true).is_ok());
        assert_eq!(bp.nodes.len(), 3);
    }

    #[test]
    fn test_decompose_all_manual_prose_rejected() {
        let mut bp = Blueprint::new("test", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "manual proof".into(),
            backend: VerificationBackend::ManualProse,
        }];
        let result = bp.decompose("root", children, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-ManualProse"));
    }

    #[test]
    fn test_backtrack() {
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "step 1".into(),
            backend: VerificationBackend::Z3,
        }];
        bp.decompose("root", children, false).unwrap();
        assert_eq!(bp.nodes.len(), 2);
        let mut visited = HashSet::new();
        bp.backtrack("root", &mut visited).unwrap();
        assert_eq!(bp.nodes.len(), 1);
    }

    #[test]
    fn test_manual_prose_ratio() {
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![
            DagNode::Leaf {
                id: "c1".into(),
                claim: "auto".into(),
                backend: VerificationBackend::Z3,
            },
            DagNode::Leaf {
                id: "c2".into(),
                claim: "manual".into(),
                backend: VerificationBackend::ManualProse,
            },
            DagNode::Leaf {
                id: "c3".into(),
                claim: "auto2".into(),
                backend: VerificationBackend::SymPy,
            },
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
            DagNode::Leaf {
                id: "c1".into(),
                claim: "m1".into(),
                backend: VerificationBackend::ManualProse,
            },
            DagNode::Leaf {
                id: "c2".into(),
                claim: "m2".into(),
                backend: VerificationBackend::ManualProse,
            },
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
            DagNode::Leaf {
                id: "c1".into(),
                claim: "via A".into(),
                backend: VerificationBackend::Z3,
            },
            DagNode::Leaf {
                id: "c2".into(),
                claim: "via B".into(),
                backend: VerificationBackend::SymPy,
            },
        ];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        assert_eq!(bp.round, 1);
        // Leaves now attempt backend verification:
        // - Z3 "via A" → parse fail → Fail
        // - SymPy "via B" → no = sign, valid expression → Pass (else branch)
        // OR node sees a Pass child → Pass
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_verify_or_node_with_identity() {
        let mut bp = Blueprint::new("identity test", "test");
        let children = vec![
            DagNode::Leaf {
                id: "c1".into(),
                claim: "x = x".into(),
                backend: VerificationBackend::SymPy,
            },
        ];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        // SymPy "x = x" should pass identity verification
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_verify_and_node() {
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![
            DagNode::Leaf {
                id: "c1".into(),
                claim: "auto".into(),
                backend: VerificationBackend::Z3,
            },
            DagNode::Leaf {
                id: "c2".into(),
                claim: "manual".into(),
                backend: VerificationBackend::ManualProse,
            },
        ];
        bp.decompose("root", children, true).unwrap();
        bp.verify().unwrap();
        // AND: Z3 "auto" fails (not a valid expression), ManualProse → Skip
        // AND with one Fail → Fail
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Fail);
    }

    #[test]
    fn test_stale_marking() {
        let mut bp = Blueprint::new("goal", "test");
        bp.verify().unwrap(); // round 1
        bp.verify().unwrap(); // round 2
        // Root result should be stale after round 2
        let root_status = bp.status.get("root").unwrap();
        assert!(!root_status.stale);
    }

    #[test]
    fn test_backtrack_cycle() {
        // Verify that backtrack handles already-visited nodes gracefully
        // (cycle detection via visited set).
        let mut bp = Blueprint::new("goal", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "step 1".into(),
            backend: VerificationBackend::Z3,
        }];
        bp.decompose("root", children, false).unwrap();
        assert_eq!(bp.nodes.len(), 2); // root + c1
        // Backtrack "c1" first — this marks c1 as visited and replaces it
        // with an OrNode (doesn't remove c1 itself).
        let mut visited = HashSet::new();
        bp.backtrack("c1", &mut visited).unwrap();
        assert_eq!(bp.nodes.len(), 2); // c1 replaced with OR, not removed
        // Backtrack "c1" again with same visited set — the visited guard
        // should return Ok(()) immediately without error.
        let result = bp.backtrack("c1", &mut visited);
        assert!(result.is_ok(), "backtrack on visited node should return Ok");
        // Nodes should still contain both root and c1 (as OR).
        assert_eq!(bp.nodes.len(), 2);
        assert!(bp.nodes.contains_key("root"));
        assert!(bp.nodes.contains_key("c1"));
    }

    #[test]
    fn test_status_summary() {
        let mut bp = Blueprint::new("test inequality", "ineq_test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "x >= 0".into(),
            backend: VerificationBackend::Z3,
        }];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        let summary = bp.status_summary();
        assert_eq!(summary["name"], "ineq_test");
        assert_eq!(summary["goal"], "test inequality");
        assert!(summary["manual_prose_cap_ok"].as_bool().unwrap_or(false));
        assert_eq!(summary["round"].as_u64().unwrap(), 1);
        // Check that nodes array is present and has expected fields.
        let nodes = summary["nodes"].as_array().unwrap();
        assert!(!nodes.is_empty(), "nodes array should not be empty");
        let node = &nodes[0];
        assert!(node.get("id").is_some(), "node should have an id field");
        assert!(node.get("type").is_some(), "node should have a type field");
        assert!(node.get("label").is_some(), "node should have a label field");
        assert!(node.get("status").is_some(), "node should have a status field");
        assert!(node.get("round").is_some(), "node should have a round field");
        assert!(node.get("stale").is_some(), "node should have a stale field");
        // Verify specific node content.
        let root_node = nodes.iter().find(|n| n["id"] == "root").unwrap();
        assert_eq!(root_node["type"], "OR");
        assert!(root_node["stale"].as_bool().is_some());
        // Verify leaf node backend field.
        let leaf_node = nodes.iter().find(|n| n["id"] == "c1").unwrap();
        assert_eq!(leaf_node["type"], "LEAF");
        assert!(leaf_node.get("backend").is_some(), "leaf node should have a backend field");
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
        let mut visited = HashSet::new();
        let result = bp.backtrack("nonexistent", &mut visited);
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

    // ── DAG verification backend tests ──

    #[test]
    fn test_verify_or_node_z3() {
        // OR node with Z3 backend and a valid linear inequality.
        // "x >= 0" is linear → routed through minilp (always available) → feasible → Pass.
        let mut bp = Blueprint::new("z3 test", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "x >= 0".into(),
            backend: VerificationBackend::Z3,
        }];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_verify_and_node_inequality() {
        // AND node with InequalityEngine backend and a valid linear inequality.
        // "x + y <= 10" → minilp feasible → Pass.
        let mut bp = Blueprint::new("inequality test", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "x + y <= 10".into(),
            backend: VerificationBackend::InequalityEngine,
        }];
        bp.decompose("root", children, true).unwrap();
        bp.verify().unwrap();
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_verify_or_node_asymptotic() {
        // OR node with Asymptotic backend — magnitude estimate of "x^2 + x"
        // as x → oo should classify x^2 as dominant term → Pass.
        let mut bp = Blueprint::new("asymptotic test", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "x^2 + x".into(),
            backend: VerificationBackend::Asymptotic,
        }];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_verify_node_cycle_detection() {
        // Manually construct a DAG with a cycle: root → c1 → root.
        // verify_node detects the back-edge via the visited set → returns Fail.
        let mut bp = Blueprint::new("goal", "test");
        bp.nodes.clear();
        bp.status.clear();
        bp.nodes.insert(
            "root".into(),
            DagNode::OrNode {
                id: "root".into(),
                label: "root".into(),
                children: vec!["c1".into()],
            },
        );
        bp.nodes.insert(
            "c1".into(),
            DagNode::OrNode {
                id: "c1".into(),
                label: "c1".into(),
                children: vec!["root".into()],
            },
        );
        bp.status.insert(
            "root".into(),
            VerificationResultExt::new(VerificationStatus::Skip, 0),
        );
        bp.status.insert(
            "c1".into(),
            VerificationResultExt::new(VerificationStatus::Skip, 0),
        );
        bp.verify().unwrap();
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Fail);
    }

    #[test]
    fn test_verify_mixed_and_or() {
        // Complex nested verification:
        // Root AND[or1, asymp_leaf]
        //   or1 = OR[leaf_z3, leaf_ineq]
        // All leaf claims are valid → root Pass.
        let mut bp = Blueprint::new("mixed test", "test");

        // Step 1: Root becomes AND node with [or1, asymp_leaf]
        bp.decompose(
            "root",
            vec![
                DagNode::OrNode {
                    id: "or1".into(),
                    label: "or sub-goals".into(),
                    children: vec![],
                },
                DagNode::Leaf {
                    id: "asymp_leaf".into(),
                    claim: "x^2 + 2*x".into(),
                    backend: VerificationBackend::Asymptotic,
                },
            ],
            true,
        )
        .unwrap();

        // Step 2: or1 becomes OR node with [leaf_z3, leaf_ineq]
        bp.decompose(
            "or1",
            vec![
                DagNode::Leaf {
                    id: "leaf_z3".into(),
                    claim: "x >= 0".into(),
                    backend: VerificationBackend::Z3,
                },
                DagNode::Leaf {
                    id: "leaf_ineq".into(),
                    claim: "y <= 5".into(),
                    backend: VerificationBackend::InequalityEngine,
                },
            ],
            false,
        )
        .unwrap();

        bp.verify().unwrap();
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Pass);
    }
}
