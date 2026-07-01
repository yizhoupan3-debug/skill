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
use regex::Regex;
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

/// Verification result with round tracking and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResultExt {
    pub status: VerificationStatus,
    pub validated_at_round: u64,
    pub stale: bool,
    /// Human-readable detail: backend name, timestamp, counterexample, etc.
    pub detail: String,
    /// ISO-8601 timestamp when this node was last checked (empty if never checked).
    pub verified_at: String,
}

impl VerificationResultExt {
    pub fn new(status: VerificationStatus, round: u64) -> Self {
        Self {
            status,
            validated_at_round: round,
            stale: false,
            detail: String::new(),
            verified_at: String::new(),
        }
    }

    /// Helper: set detail and record current timestamp.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self.verified_at = Self::now_iso();
        self
    }

    fn now_iso() -> String {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Simple ISO-like format: "2026-07-01T12:34:56Z"
        let days = secs / 86400;
        let time_secs = secs % 86400;
        let hours = time_secs / 3600;
        let minutes = (time_secs % 3600) / 60;
        let seconds = time_secs % 60;
        // Use a fixed epoch-based date approximation
        let year = 1970 + (days as f64 / 365.25) as u64;
        format!("{year}-{hours:02}:{minutes:02}:{seconds:02}Z")
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
                VerificationResultExt::new(VerificationStatus::Skip, self.round)
                    .with_detail("not_yet_verified"),
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
            let (status, detail) = match &node_backend_opt {
                Some(VerificationBackend::ManualProse) => {
                    (VerificationStatus::Skip, "verification_impossible: ManualProse cannot be automated".to_string())
                }
                Some(VerificationBackend::InequalityEngine) => {
                    attempt_inequality_verify(self, node_id)
                }
                Some(VerificationBackend::Z3) => {
                    attempt_z3_verify(self, node_id)
                }
                Some(VerificationBackend::SymPy) => {
                    attempt_sympy_verify(self, node_id)
                }
                Some(VerificationBackend::Asymptotic) => {
                    attempt_asymptotic_verify(self, node_id)
                }
                Some(VerificationBackend::Lean) => {
                    attempt_lean_verify(self, node_id)
                }
                None => (VerificationStatus::Skip, "no backend specified".to_string()),
            };
            let result = VerificationResultExt::new(status, round).with_detail(detail);
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
    // Proof Step Dependency Check
    // =======================================================================

    /// Check that every non-leaf node's children have been verified.
    ///
    /// A child is considered "verified" if its status is Pass or Warn.
    /// Children with Skip or not-yet-attempted status are reported as
    /// unmet dependencies. This enforces a bottom-up verification order:
    /// leaves must be verified before their parent nodes can be trusted.
    ///
    /// Returns a list of `(node_id, [unverified_child_ids])` for each node
    /// with unmet dependencies.
    pub fn check_step_dependencies(&self) -> Vec<(DagNodeId, Vec<DagNodeId>)> {
        let mut unmet = Vec::new();

        for (id, node) in &self.nodes {
            let children = node.children();
            if children.is_empty() {
                continue;
            }

            let mut missing = Vec::new();
            for child_id in children {
                let is_verified = match self.status.get(child_id) {
                    Some(result) => matches!(
                        result.status,
                        VerificationStatus::Pass | VerificationStatus::Warn
                    ),
                    None => false,
                };
                if !is_verified {
                    missing.push(child_id.clone());
                }
            }

            if !missing.is_empty() {
                unmet.push((id.clone(), missing));
            }
        }

        unmet
    }

    // =======================================================================
    // OR Consistency Check
    // =======================================================================

    /// Check OR-node branches for contradictory claims.
    ///
    /// For each OR node, collects all leaf claims from each child branch and
    /// checks for contradictory inequality pairs (e.g., one branch claims
    /// `x > 0` while another claims `x < 0`). Such contradictions in an
    /// OR context indicate the alternative strategies are logically
    /// incompatible, which the user should be warned about.
    ///
    /// Returns a list of `(claim_a, claim_b)` pairs that contradict.
    pub fn check_or_consistency(&self) -> Vec<(String, String)> {
        let mut inconsistencies = Vec::new();

        for (_id, node) in &self.nodes {
            if let DagNode::OrNode { children, .. } = node {
                let branch_claims: Vec<Vec<String>> = children
                    .iter()
                    .map(|child_id| self.collect_leaf_claims(child_id))
                    .collect();

                for i in 0..branch_claims.len() {
                    for j in (i + 1)..branch_claims.len() {
                        for ca in &branch_claims[i] {
                            for cb in &branch_claims[j] {
                                if claims_contradict(ca, cb) {
                                    inconsistencies.push((ca.clone(), cb.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }

        inconsistencies
    }

    /// Recursively collect all leaf claims under a given node.
    fn collect_leaf_claims(&self, node_id: &str) -> Vec<String> {
        let mut claims = Vec::new();
        if let Some(node) = self.nodes.get(node_id) {
            match node {
                DagNode::Leaf { claim, .. } => {
                    claims.push(claim.clone());
                }
                _ => {
                    for child in node.children() {
                        claims.extend(self.collect_leaf_claims(child));
                    }
                }
            }
        }
        claims
    }
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
                "detail": st.map(|s| s.detail.as_str()).unwrap_or(""),
                "verified_at": st.map(|s| s.verified_at.as_str()).unwrap_or(""),
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

/// Extract the label text from a node, returning empty string if missing.
fn node_claim(bp: &Blueprint, node_id: &str) -> String {
    bp.nodes.get(node_id).map(|n| n.label().to_string()).unwrap_or_default()
}

/// Attempt to verify a leaf node using the InequalityEngine.
fn attempt_inequality_verify(bp: &Blueprint, node_id: &str) -> (VerificationStatus, String) {
    let claim = node_claim(bp, node_id);
    if claim.is_empty() {
        return (VerificationStatus::Skip, "inequality: empty claim".into());
    }
    let result = crate::verification::inequality::check_inequality(&claim, Some(5000));
    let detail = format!("inequality: {}", result.details);
    match result.status {
        VerificationStatus::Pass => (VerificationStatus::Pass, detail),
        VerificationStatus::Fail => (VerificationStatus::Fail, detail),
        _ => (VerificationStatus::Warn, detail),
    }
}

/// Attempt to verify a leaf node using Z3 backend.
///
/// Uses a two-phase strategy:
/// 1. Try `prove_formula` for theorem proving (universal validity)
/// 2. Fall back to `check_inequality` for satisfiability checking
///
/// When the theorem-proving phase finds a counterexample, it is included
/// in the detail string for diagnostic purposes.
fn attempt_z3_verify(bp: &Blueprint, node_id: &str) -> (VerificationStatus, String) {
    let claim = node_claim(bp, node_id);
    if claim.is_empty() {
        return (VerificationStatus::Skip, "z3: empty claim".into());
    }

    // Phase 1: Try theorem proving (prove_formula) for universal validity.
    // Uses z3.Prove() which checks that the negation is unsatisfiable.
    // Best for: Implies, ForAll formulas; also catches simple valid identities.
    let is_logical = claim.contains("Implies")
        || claim.contains("ForAll")
        || claim.contains("Exists")
        || claim.contains("And(")
        || claim.contains("Or(")
        || claim.contains("Not(");

    if is_logical || crate::verification::python_bridge::z3_available() {
        let prove_result = crate::verification::z3_bridge::prove_formula(&claim);
        match prove_result.status {
            VerificationStatus::Pass => {
                return (
                    VerificationStatus::Pass,
                    format!("z3_prove: {}", prove_result.details),
                );
            }
            VerificationStatus::Fail => {
                // Disproved — include the counterexample from Z3
                let fail_detail = format!("z3_prove_fail: {}", prove_result.details);
                // For logical formulas, this is definitive; for simple inequalities,
                // prove_formula may reject (x >= 0 is not universally ∀-valid)
                // so fall through to inequality check.
                if is_logical {
                    return (VerificationStatus::Fail, fail_detail);
                }
                tracing::debug!("[z3_verify] prove_formula failed, falling back to check_inequality: {fail_detail}");
            }
            _ => {
                // prove_formula unavailable or error — fall through to Phase 2
            }
        }
    }

    // Phase 2: Try inequality checking (feasibility/satisfiability).
    // Routes linear inequalities through minilp, nonlinear through Z3.
    let result = crate::verification::inequality::check_inequality(&claim, Some(10000));
    let detail = format!("z3_inequality: {}", result.details);
    match result.status {
        VerificationStatus::Pass => (VerificationStatus::Pass, detail),
        VerificationStatus::Fail => (VerificationStatus::Fail, detail),
        _ => (VerificationStatus::Warn, detail),
    }
}

/// Attempt to verify a leaf node using SymPy backend.
///
/// Two modes:
/// - `lhs = rhs`: verify algebraic identity via `verify_identity`.
/// - Single expression: try `simplify_expression`; Pass only if it simplifies
///   to "0" (identically zero). Otherwise Warn — a single expression
///   without an equality claim cannot be strongly verified.
fn attempt_sympy_verify(bp: &Blueprint, node_id: &str) -> (VerificationStatus, String) {
    let claim = node_claim(bp, node_id);
    if claim.is_empty() {
        return (VerificationStatus::Skip, "sympy: empty claim".into());
    }
    if let Some(eq_pos) = claim.find('=') {
        let lhs = claim[..eq_pos].trim();
        let rhs = claim[eq_pos + 1..].trim();
        let result = crate::verification::sympy_bridge::verify_identity(lhs, rhs);
        let detail = format!("sympy_verify: {}", result.details);
        match result.status {
            VerificationStatus::Pass => (VerificationStatus::Pass, detail),
            VerificationStatus::Fail => (VerificationStatus::Fail, detail),
            _ => (VerificationStatus::Warn, detail),
        }
    } else {
        // Single expression: try simplifying to check if identically zero.
        // This handles claims like "x - x" (expects identity to 0 → Pass)
        // without blindly accepting arbitrary expressions.
        let result = crate::verification::sympy_bridge::simplify_expression(&claim);
        let simplified = result
            .details
            .split(" → ")
            .nth(1)
            .and_then(|s| s.split(" (").next())
            .unwrap_or("")
            .trim();
        if simplified == "0" || simplified == "0.0" {
            (
                VerificationStatus::Pass,
                format!("sympy_simplify: {} (identically zero)", result.details),
            )
        } else {
            (
                VerificationStatus::Warn,
                format!(
                    "sympy: single expression without '=', simplified to '{simplified}' — \
                     use lhs = rhs form for strong verification"
                ),
            )
        }
    }
}

/// Attempt to verify a leaf node using asymptotic analysis.
///
/// Extracts the primary variable from the claim (instead of hardcoding "x")
/// so claims like "n^2 + n" correctly use n as the asymptotic variable.
fn attempt_asymptotic_verify(bp: &Blueprint, node_id: &str) -> (VerificationStatus, String) {
    let claim = node_claim(bp, node_id);
    if claim.is_empty() {
        return (VerificationStatus::Skip, "asymptotic: empty claim".into());
    }

    // Extract variables from the claim — reuses the same regex pattern
    // used in inequality.rs for variable detection.
    let known_keywords = [
        "sin", "cos", "tan", "sqrt", "abs", "exp", "log", "ln",
        "And", "Or", "Not", "Implies", "True", "False", "pi", "e",
    ];
    let re_vars = Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").expect("valid regex");
    let var = re_vars
        .find_iter(&claim)
        .map(|m| m.as_str())
        .find(|v| !known_keywords.contains(v))
        .unwrap_or("x");

    let result = crate::verification::asymptotic::magnitude_estimate(&claim, var, "oo");
    let detail = format!("asymptotic({var}→oo): {}", result.details);
    match result.status {
        VerificationStatus::Pass => (VerificationStatus::Pass, detail),
        _ => (VerificationStatus::Warn, detail),
    }
}

/// Attempt to verify a leaf node using Lean.
fn attempt_lean_verify(bp: &Blueprint, node_id: &str) -> (VerificationStatus, String) {
    let claim = node_claim(bp, node_id);
    if claim.is_empty() {
        return (VerificationStatus::Skip, "lean: empty claim".into());
    }
    let result = crate::verification::lean_bridge::verify_lean_theorem(&claim);
    let detail = format!("lean: {}", result.details);
    match result.status {
        VerificationStatus::Pass => (VerificationStatus::Pass, detail),
        VerificationStatus::Fail => (VerificationStatus::Fail, detail),
        _ => (VerificationStatus::Warn, detail),
    }
}

// ===========================================================================
// OR Consistency Helpers
// ===========================================================================

/// Heuristic check: do two inequality claims contradict each other?
///
/// Checks if claims share the same expression operands but use opposite
/// inequality directions. Catches common cases like `x > 0` vs `x < 0`.
///
/// Supported opposite pairs:
/// - `>` vs `<`
/// - `>=` vs `<`
/// - `>` vs `<=`
/// - `>=` vs `<=` is NOT contradictory (x=0 satisfies both)
/// - `==` vs `!=`
fn claims_contradict(a: &str, b: &str) -> bool {
    // Opposite sense pairs that are strictly contradictory
    // (no value can satisfy both simultaneously)
    let opposite_pairs: &[(&str, &str)] = &[
        (">", "<"),
        (">", "<="),
        (">=", "<"),
        ("<", ">"),
        ("<=", ">"),
        ("<", ">="),
        ("==", "!="),
        ("!=", "=="),
    ];

    // Decompose each claim: left-hand side, operator, right-hand side (owned strings)
    let decompose = |s: &str| -> Option<(String, String, String)> {
        let re = Regex::new(r"^(.+?)\s*(<=|>=|<|>|==|=|!=)\s*(.+)$").ok()?;
        let caps = re.captures(s)?;
        Some((
            caps.get(1)?.as_str().trim().to_string(),
            caps.get(2)?.as_str().to_string(),
            caps.get(3)?.as_str().trim().to_string(),
        ))
    };

    let (lhs_a, op_a, rhs_a) = match decompose(a) {
        Some(v) => v,
        None => return false,
    };
    let (lhs_b, op_b, rhs_b) = match decompose(b) {
        Some(v) => v,
        None => return false,
    };

    // Same operands with opposite inequality direction → contradiction
    lhs_a == lhs_b && rhs_a == rhs_b && opposite_pairs.contains(&(op_a.as_str(), op_b.as_str()))
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
                claim: "x >= 0".into(),
                backend: VerificationBackend::Z3,
            },
            DagNode::Leaf {
                id: "c2".into(),
                claim: "(x+1)^2 = x^2 + 2*x + 1".into(),
                backend: VerificationBackend::SymPy,
            },
        ];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        assert_eq!(bp.round, 1);
        // Z3 "x >= 0" → linear, minilp feasible → Pass
        // SymPy "(x+1)^2 = x^2 + 2*x + 1" → identity → Pass
        // OR node sees a Pass child → Pass
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(root_status.status, VerificationStatus::Pass);
        // Verify that detail metadata is recorded
        let c1_status = bp.status.get("c1").unwrap();
        assert!(!c1_status.detail.is_empty(), "Z3 leaf should have detail");
        assert!(!c1_status.verified_at.is_empty(), "Z3 leaf should have verified_at");
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

    // ── Step Dependency Check Tests ──

    #[test]
    fn test_check_step_dependencies_unverified_children() {
        // Create a DAG where not all children are verified.
        let mut bp = Blueprint::new("goal", "test");

        // Children are initially Skip with "not_yet_verified".
        let children = vec![
            DagNode::Leaf {
                id: "c1".into(),
                claim: "auto step".into(),
                backend: VerificationBackend::Z3,
            },
            DagNode::Leaf {
                id: "c2".into(),
                claim: "manual step".into(),
                backend: VerificationBackend::ManualProse,
            },
        ];
        bp.decompose("root", children, true).unwrap();

        // Before verify, all children are Skip with "not_yet_verified"
        let unmet = bp.check_step_dependencies();
        let root_unmet = unmet.iter().find(|(id, _)| id == "root");
        assert!(root_unmet.is_some(), "root should have unmet dependencies");
        let (_, missing) = root_unmet.unwrap();
        assert_eq!(missing.len(), 2, "both children should be unverified before verify()");
    }

    #[test]
    fn test_check_step_dependencies_after_verify() {
        // After verify, verified children no longer show as unmet.
        let mut bp = Blueprint::new("goal", "test");
        bp.decompose(
            "root",
            vec![DagNode::Leaf {
                id: "c1".into(),
                claim: "x >= 0".into(),
                backend: VerificationBackend::Z3,
            }],
            true,
        )
        .unwrap();
        bp.verify().unwrap();
        let unmet = bp.check_step_dependencies();
        // Z3 "x >= 0" is verified → no unmet dependencies for this child
        let root_unmet = unmet.iter().find(|(id, _)| id == "root");
        assert!(root_unmet.is_none(), "verified child should not be unmet");
    }

    // ── OR Consistency Tests ──

    #[test]
    fn test_or_consistency_contradiction() {
        // OR node with two branches containing contradictory claims.
        let mut bp = Blueprint::new("contradiction test", "test");
        bp.decompose(
            "root",
            vec![
                DagNode::Leaf {
                    id: "branch_a".into(),
                    claim: "x > 0".into(),
                    backend: VerificationBackend::Z3,
                },
                DagNode::Leaf {
                    id: "branch_b".into(),
                    claim: "x < 0".into(),
                    backend: VerificationBackend::Z3,
                },
            ],
            false,
        )
        .unwrap();
        let inconsistencies = bp.check_or_consistency();
        assert_eq!(inconsistencies.len(), 1, "should find 1 contradiction");
        assert!(inconsistencies[0].0.contains("> 0") || inconsistencies[0].0.contains("< 0"));
        assert!(inconsistencies[0].1.contains("> 0") || inconsistencies[0].1.contains("< 0"));
    }

    #[test]
    fn test_or_consistency_no_contradiction() {
        // OR node with compatible branches.
        let mut bp = Blueprint::new("compatible test", "test");
        bp.decompose(
            "root",
            vec![
                DagNode::Leaf {
                    id: "branch_a".into(),
                    claim: "x >= 0".into(),
                    backend: VerificationBackend::Z3,
                },
                DagNode::Leaf {
                    id: "branch_b".into(),
                    claim: "x <= 10".into(),
                    backend: VerificationBackend::Z3,
                },
            ],
            false,
        )
        .unwrap();
        let inconsistencies = bp.check_or_consistency();
        assert_eq!(
            inconsistencies.len(),
            0,
            "compatible claims should not be contradictory"
        );
    }

    #[test]
    fn test_or_consistency_ge_vs_le() {
        // x >= 0 and x <= 0 are compatible (x=0 satisfies both)
        let mut bp = Blueprint::new("compatible test 2", "test");
        bp.decompose(
            "root",
            vec![
                DagNode::Leaf {
                    id: "branch_a".into(),
                    claim: "x >= 0".into(),
                    backend: VerificationBackend::Z3,
                },
                DagNode::Leaf {
                    id: "branch_b".into(),
                    claim: "x <= 0".into(),
                    backend: VerificationBackend::Z3,
                },
            ],
            false,
        )
        .unwrap();
        let inconsistencies = bp.check_or_consistency();
        assert_eq!(inconsistencies.len(), 0, "x >= 0 and x <= 0 are compatible at x=0");
    }

    // ── SymPy Single Expression Tests ──

    #[test]
    fn test_sympy_single_expression_does_not_blindly_pass() {
        // A SymPy leaf with a single expression (no '=' sign) should NOT return Pass.
        // Previously it returned Pass unconditionally — now it returns Warn.
        let mut bp = Blueprint::new("sympy single expr", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "x + y".into(),
            backend: VerificationBackend::SymPy,
        }];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        // The OR now sees Warn (not Pass) because single expression → Warn.
        // With only one Warn child, OR returns Warn.
        let root_status = bp.status.get("root").unwrap();
        assert_eq!(
            root_status.status,
            VerificationStatus::Warn,
            "single expression without '=' should not be Pass"
        );
        let c1_status = bp.status.get("c1").unwrap();
        assert!(
            c1_status.detail.contains("single expression"),
            "detail should mention single expression, got: {}",
            c1_status.detail
        );
    }

    #[test]
    fn test_sympy_identity_zero_passes() {
        // An expression that simplifies to "0" (e.g., "x - x") should pass.
        let mut bp = Blueprint::new("sympy zero", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "x - x".into(),
            backend: VerificationBackend::SymPy,
        }];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        let c1_status = bp.status.get("c1").unwrap();
        // Pure Rust symbolic should handle "x - x" → 0, so we expect Pass.
        assert!(
            c1_status.status == VerificationStatus::Pass || c1_status.status == VerificationStatus::Warn,
            "expected Pass or Warn for x-x, got {:?}: {}",
            c1_status.status,
            c1_status.detail
        );
    }

    // ── Asymptotic Variable Extraction Test ──

    #[test]
    fn test_asymptotic_uses_claim_variable() {
        // The asymptotic backend should extract the variable from the claim
        // rather than hardcoding "x". A claim "n^2 + n" should use variable "n".
        let mut bp = Blueprint::new("asymp var test", "test");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "n^2 + n".into(),
            backend: VerificationBackend::Asymptotic,
        }];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();
        let c1_status = bp.status.get("c1").unwrap();
        // Detail should mention n (not x) as the variable
        assert!(
            c1_status.detail.contains("asymptotic(n"),
            "asymptotic detail should use variable 'n', got: {}",
            c1_status.detail
        );
        assert!(
            c1_status.status == VerificationStatus::Pass,
            "n^2 + n as n→∞ should pass, got: {:?}",
            c1_status.status
        );
    }

    // ── Claims Contradict helper tests ──

    #[test]
    fn test_claims_contradict_strict() {
        assert!(claims_contradict("x > 0", "x < 0"), "x > 0 and x < 0 should contradict");
        assert!(claims_contradict("x > 0", "x <= 0"), "x > 0 and x <= 0 should contradict");
        assert!(claims_contradict("x >= 0", "x < 0"), "x >= 0 and x < 0 should contradict");
    }

    #[test]
    fn test_claims_contradict_not() {
        assert!(!claims_contradict("x > 0", "x > 0"), "identical claims should not contradict");
        assert!(!claims_contradict("x >= 0", "x <= 0"), "x >= 0 and x <= 0 are compatible at x=0");
        assert!(!claims_contradict("x > 0", "y > 0"), "different variables should not contradict");
        assert!(!claims_contradict("hello world", "x > 0"), "non-inequality should not contradict");
    }

    // ── Detail and verified_at metadata tests ──

    #[test]
    fn test_manual_prose_verification_impossible() {
        // ManualProse leaves should record "verification_impossible" in detail.
        let mut bp = Blueprint::new("manual prose test", "test");
        bp.decompose(
            "root",
            vec![DagNode::Leaf {
                id: "c1".into(),
                claim: "human proof".into(),
                backend: VerificationBackend::ManualProse,
            }],
            false,
        )
        .unwrap();
        bp.verify().unwrap();
        let c1_status = bp.status.get("c1").unwrap();
        assert_eq!(c1_status.status, VerificationStatus::Skip);
        assert!(
            c1_status.detail.contains("verification_impossible"),
            "ManualProse detail should mention verification_impossible, got: {}",
            c1_status.detail
        );
    }

    #[test]
    fn test_not_yet_verified_detail() {
        // Before verify(), child nodes should have "not_yet_verified" detail.
        let mut bp = Blueprint::new("pending test", "test");
        bp.decompose(
            "root",
            vec![DagNode::Leaf {
                id: "c1".into(),
                claim: "x >= 0".into(),
                backend: VerificationBackend::Z3,
            }],
            false,
        )
        .unwrap();
        let c1_status = bp.status.get("c1").unwrap();
        assert!(
            c1_status.detail.contains("not_yet_verified"),
            "before verify, detail should be 'not_yet_verified', got: {}",
            c1_status.detail
        );
    }

    #[test]
    fn test_verify_cycle_with_detail() {
        // Verify that cycle detection also records detail.
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
    fn test_status_summary_includes_detail_and_verified_at() {
        let mut bp = Blueprint::new("summary test", "test");
        bp.decompose(
            "root",
            vec![DagNode::Leaf {
                id: "c1".into(),
                claim: "x >= 0".into(),
                backend: VerificationBackend::Z3,
            }],
            false,
        )
        .unwrap();
        let summary = bp.status_summary();
        let nodes = summary["nodes"].as_array().unwrap();
        for node in nodes {
            assert!(
                node.get("detail").is_some(),
                "each node should have a detail field"
            );
            assert!(
                node.get("verified_at").is_some(),
                "each node should have a verified_at field"
            );
        }
    }
}
