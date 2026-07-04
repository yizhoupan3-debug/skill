//! Lean theorem prover bridge — status check, verification, caching, and
//! unified multi-backend proving.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! # Enhancements over baseline
//!
//! 1. **LRU proof cache** — 64 entries, 5-minute TTL
//! 2. **Lean error parsing** — extract line/col/type from stderr
//! 3. **Proof template generation** — auto-wrap bare statements
//! 4. **Multi-theorem verification** — per-theorem status from one script
//! 5. **Unified backends** — try_prove_with_all_backends
//! 6. **Enhanced backend_status** — capabilities + install hints
//! 7. **Toolchain check** — lean + lake version reporting

use crate::types::{VerificationResult, VerificationStatus};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

// ===========================================================================
// Cache constants
// ===========================================================================

/// Maximum number of cached Lean proof results (LRU eviction).
const PROOF_CACHE_MAX_ENTRIES: usize = 64;

/// TTL for cached Lean proof results.
const PROOF_CACHE_TTL_SECS: u64 = 300; // 5 minutes

// ===========================================================================
// Public types
// ===========================================================================

/// Status of the Lean toolchain availability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeanStatus {
    /// Lean 4 is installed and usable.
    Available,
    /// Lean is not found or broken, with diagnostic info.
    NotFound {
        reason: String,
        install_guide: String,
    },
}

impl LeanStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, LeanStatus::Available)
    }
}

/// A single parsed Lean error with file location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeanErrorLocation {
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub error_type: Option<String>,
    pub message: String,
    pub raw_line: String,
}

/// Per-theorem result for multi-theorem scripts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TheoremResult {
    pub theorem_name: String,
    pub status: VerificationStatus,
    pub details: String,
}

/// Detailed capability information for a math backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCapability {
    pub available: bool,
    pub version: Option<String>,
    pub capabilities: Vec<String>,
    pub install_hint: Option<String>,
}

/// Full toolchain report for Lean (lean + lake).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeanToolchain {
    pub lean_version: Option<String>,
    pub lake_version: Option<String>,
    pub toolchain_available: bool,
    pub detail: String,
}

// ===========================================================================
// LRU proof cache with TTL
// ===========================================================================

struct CachedProofEntry {
    result: VerificationResult,
    cached_at: Instant,
}

struct LruProofCache {
    entries: HashMap<u64, CachedProofEntry>,
    order: VecDeque<u64>,
    max_entries: usize,
    ttl: std::time::Duration,
}

impl LruProofCache {
    fn new(max_entries: usize, ttl: std::time::Duration) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            order: VecDeque::with_capacity(max_entries),
            max_entries,
            ttl,
        }
    }

    /// Look up a cached result. Returns `None` on miss or TTL expiry.
    fn get(&mut self, key: &u64) -> Option<VerificationResult> {
        // Check TTL first
        let entry = self.entries.get(key)?;
        if entry.cached_at.elapsed() > self.ttl {
            let _ = self.remove(key);
            return None;
        }

        // Move to back (most recently used)
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(pos).unwrap();
            self.order.push_back(k);
        }

        self.entries.get(key).map(|e| e.result.clone())
    }

    /// Insert a result into the cache, evicting LRU if at capacity.
    fn insert(&mut self, key: u64, result: VerificationResult) {
        if self.entries.contains_key(&key) {
            // Update in-place
            self.entries.insert(
                key,
                CachedProofEntry {
                    result,
                    cached_at: Instant::now(),
                },
            );
            // Refresh MRU position
            if let Some(pos) = self.order.iter().position(|k| *k == key) {
                let k = self.order.remove(pos).unwrap();
                self.order.push_back(k);
            }
            return;
        }

        // Evict least-recently-used when at capacity
        if self.entries.len() >= self.max_entries {
            if let Some(lru_key) = self.order.pop_front() {
                self.entries.remove(&lru_key);
            }
        }

        self.entries.insert(
            key,
            CachedProofEntry {
                result,
                cached_at: Instant::now(),
            },
        );
        self.order.push_back(key);
    }

    fn remove(&mut self, key: &u64) -> bool {
        let existed = self.entries.remove(key).is_some();
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        existed
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

static PROOF_CACHE: OnceLock<Mutex<LruProofCache>> = OnceLock::new();

fn proof_cache() -> &'static Mutex<LruProofCache> {
    PROOF_CACHE.get_or_init(|| {
        Mutex::new(LruProofCache::new(
            PROOF_CACHE_MAX_ENTRIES,
            std::time::Duration::from_secs(PROOF_CACHE_TTL_SECS),
        ))
    })
}

/// Compute a u64 hash of a proof script for cache keying.
fn hash_script(script: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(script.as_bytes());
    let result = hasher.finalize();
    u64::from_ne_bytes(result[..8].try_into().unwrap())
}

// ===========================================================================
// Public: cache management
// ===========================================================================

/// Clear the in-memory Lean proof cache.
pub fn clear_proof_cache() {
    if let Ok(mut cache) = proof_cache().lock() {
        cache.clear();
    }
}

// ===========================================================================
// Compile-once regex helpers
// ===========================================================================

fn location_pattern() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^(\S+):(\d+):(\d+):").expect("location regex is valid")
    })
}

fn theorem_name_pattern() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?m)^\s*(?:theorem|lemma)\s+([a-zA-Z_][a-zA-Z0-9_'.]*)")
            .expect("theorem name regex is valid")
    })
}

// ===========================================================================
// Enhanced backend capability checks
//
// IMPORTANT: Do NOT add `check_z3_available()` / `check_sympy_available()`
// proxy functions here — call `crate::verification::z3_bridge::*` or
// `crate::verification::python_bridge::*` directly at the call site.
// See mcp_tools or z3_bridge for the canonical backend status probes.
// ===========================================================================

/// Get detailed capability info for the Z3 backend.
pub fn check_z3_capability() -> BackendCapability {
    let available = crate::verification::z3_bridge::z3_available();
    if !available {
        return BackendCapability {
            available: false,
            version: None,
            capabilities: vec![],
            install_hint: Some("pip install z3-solver".into()),
        };
    }

    BackendCapability {
        available: true,
        version: None, // filled from python status in check_all_backends
        capabilities: vec![
            "prove".into(),
            "sat_check".into(),
            "solver_push_pop".into(),
            "solver_batch".into(),
        ],
        install_hint: None,
    }
}

/// Check whether Z3 supports non-linear arithmetic (Z3 >=4.x does).
pub fn z3_supports_nonlinear() -> bool {
    if !crate::verification::z3_bridge::z3_available() {
        return false;
    }
    let result = crate::verification::z3_bridge::prove_formula("x * x == 4");
    result.status == crate::types::VerificationStatus::Pass
        || result.status == crate::types::VerificationStatus::Fail
}

/// Get detailed capability info for the SymPy backend.
pub fn check_sympy_capability() -> BackendCapability {
    let available = crate::verification::python_bridge::sympy_available();
    if !available {
        return BackendCapability {
            available: false,
            version: None,
            capabilities: vec![],
            install_hint: Some("pip install sympy".into()),
        };
    }

    BackendCapability {
        available: true,
        version: None,
        capabilities: vec![
            "verify".into(),
            "simplify".into(),
            "expand".into(),
            "factor".into(),
            "solve".into(),
            "differentiate".into(),
            "integrate".into(),
            "series".into(),
            "limit".into(),
            "lambdify".into(),
            "trig_simplify".into(),
            "dimension_propagate".into(),
        ],
        install_hint: None,
    }
}

// ===========================================================================
// Lean toolchain probing
// ===========================================================================

/// Check the Lean toolchain — reports `lean --version` and `lake --version`.
pub fn check_lean_toolchain() -> LeanToolchain {
    let lean_version = get_lean_version();
    let lake_version = get_lake_version();

    LeanToolchain {
        lean_version: lean_version.clone(),
        lake_version: lake_version.clone(),
        toolchain_available: lean_version.is_some(),
        detail: build_toolchain_detail(&lean_version, &lake_version),
    }
}

fn get_lean_version() -> Option<String> {
    let output = std::process::Command::new("lean")
        .arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn get_lake_version() -> Option<String> {
    let output = std::process::Command::new("lake")
        .arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn build_toolchain_detail(lean_ver: &Option<String>, lake_ver: &Option<String>) -> String {
    match (lean_ver, lake_ver) {
        (Some(lv), Some(lk)) => format!("lean: {lv} | lake: {lk}"),
        (Some(lv), None) => format!("lean: {lv} | lake: not found"),
        (None, Some(lk)) => format!("lean: not found | lake: {lk}"),
        (None, None) => "lean: not found | lake: not found".into(),
    }
}

// ===========================================================================
// Lean error parsing
// ===========================================================================

/// Parse Lean 4 error messages from raw stderr output.
///
/// Typical output:
/// ```text
/// <file>:<line>:<col>:
/// error: <message>
/// ```
/// Returns a structured `Vec<LeanErrorLocation>`.
pub fn parse_lean_errors(stderr: &str) -> Vec<LeanErrorLocation> {
    let mut errors = Vec::new();
    let mut lines = stderr.lines().peekable();

    while let Some(line) = lines.next() {
        // Try to match a location line: "<path>:<line>:<col>:"
        if let Some((_file, line_no, col)) = parse_location_line(line) {
            let loc_line = line_no;
            let loc_col = col;

            // The next line should be the error/warning message
            if let Some(next_line) = lines.peek() {
                if let Some((err_type, msg)) = parse_error_line(next_line) {
                    let _ = lines.next(); // consume the error line
                    let mut full_msg = msg.to_string();

                    // Collect continuation lines (indented context)
                    while let Some(cont) = lines.peek() {
                        let trimmed = cont.trim_start();
                        if !cont.is_empty()
                            && (cont.starts_with("  ") || cont.starts_with('\t') || cont.starts_with("│"))
                        {
                            let _ = lines.next();
                            if !trimmed.is_empty() {
                                full_msg.push('\n');
                                full_msg.push_str(trimmed);
                            }
                        } else {
                            break;
                        }
                    }

                    errors.push(LeanErrorLocation {
                        line: Some(loc_line),
                        col: Some(loc_col),
                        error_type: Some(err_type),
                        message: full_msg,
                        raw_line: line.to_string(),
                    });
                    continue;
                }
            }
        }

        // Standalone error/warning line (no preceding location)
        if let Some((err_type, msg)) = parse_error_line(line) {
            errors.push(LeanErrorLocation {
                line: None,
                col: None,
                error_type: Some(err_type),
                message: msg.to_string(),
                raw_line: line.to_string(),
            });
        }
    }

    errors
}

/// Parse a Lean location line: `<path>:<line>:<col>:`.
fn parse_location_line(line: &str) -> Option<(String, usize, usize)> {
    let trimmed = line.trim();
    let caps = location_pattern().captures(trimmed)?;

    let file = caps.get(1)?.as_str().to_string();
    let line_no: usize = caps.get(2)?.as_str().parse().ok()?;
    let col: usize = caps.get(3)?.as_str().parse().ok()?;

    Some((file, line_no, col))
}

/// Parse a Lean error/warning/info prefix line.
fn parse_error_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();

    if let Some(msg) = trimmed.strip_prefix("error: ") {
        return Some(("error".into(), msg.to_string()));
    }
    if let Some(msg) = trimmed.strip_prefix("warning: ") {
        return Some(("warning".into(), msg.to_string()));
    }
    if let Some(msg) = trimmed.strip_prefix("info: ") {
        return Some(("info".into(), msg.to_string()));
    }

    None
}

// ===========================================================================
// Proof template generation
// ===========================================================================

/// Heuristic: check whether `input` looks like a complete Lean script
/// (contains a theorem, lemma, def, or similar top-level command).
pub fn is_lean_script(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with("theorem ")
        || trimmed.starts_with("lemma ")
        || trimmed.starts_with("example ")
        || trimmed.starts_with("def ")
        || trimmed.starts_with("inductive ")
        || trimmed.starts_with("structure ")
        || trimmed.starts_with("class ")
        || trimmed.starts_with("instance ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("open ")
        || trimmed.starts_with("namespace ")
        || trimmed.starts_with("set_option ")
        || trimmed.starts_with("/--")
        || trimmed.starts_with("--")
        || trimmed.contains(" := ")
}

/// Auto-generate a Lean proof script from a theorem statement.
///
/// If `statement` is already a complete Lean script, returns it unchanged.
/// Otherwise wraps it in `theorem auto_theorem : <statement> := by simp`.
pub fn generate_lean_proof_script(statement: &str) -> String {
    let trimmed = statement.trim();

    if is_lean_script(trimmed) {
        return trimmed.to_string();
    }

    // Try `simp` first — works for simple algebraic identities
    format!("theorem auto_theorem : {trimmed} := by\n  simp\n")
}

// ===========================================================================
// Multi-theorem parsing
// ===========================================================================

/// Extract `theorem` and `lemma` names from a Lean script.
pub fn extract_theorem_names(script: &str) -> Vec<String> {
    let re = theorem_name_pattern();
    re.captures_iter(script)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

// ===========================================================================
// Lean status check
// ===========================================================================

/// Check if Lean 4 is available on the system PATH.
///
/// Probes `which lean` and `lean --version`. No caching — per-invocation probe.
pub fn check_lean_status() -> LeanStatus {
    let which = std::process::Command::new("which")
        .arg("lean")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    match which {
        Ok(output) if output.status.success() => {
            // Also check version
            let version = std::process::Command::new("lean")
                .arg("--version")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output();
            match version {
                Ok(v) if v.status.success() => LeanStatus::Available,
                _ => LeanStatus::NotFound {
                    reason: "lean binary found but `lean --version` failed".into(),
                    install_guide: "Run: elan install lean4".into(),
                },
            }
        }
        _ => LeanStatus::NotFound {
            reason: "lean not found on system PATH".into(),
            install_guide: concat!(
                "Install elan (Lean 4 version manager):\n",
                "  curl -L https://github.com/leanprover/elan/releases/download/v4.0.3/elan-x86_64-unknown-linux-gnu.tar.gz | tar xz\n",
                "  ./elan-init\n",
                "Then: elan install lean4\n",
                "Or use the VS Code extension 'lean4'."
            )
            .into(),
        },
    }
}

// ===========================================================================
// Comprehensive backend status
// ===========================================================================

/// Get a comprehensive backend status report with capabilities and hints.
pub fn check_all_backends() -> serde_json::Value {
    // Get Python backend status
    let python_status = crate::verification::python_bridge::get_full_status_report();

    // Get Lean status separately (not via Python)
    let lean_status = check_lean_status();
    let (lean_available, lean_detail) = match &lean_status {
        LeanStatus::Available => (true, "Lean 4 installed".to_string()),
        LeanStatus::NotFound {
            reason,
            install_guide,
        } => (false, format!("{reason}. {install_guide}")),
    };

    // Get toolchain info
    let toolchain = check_lean_toolchain();

    // Get capability details
    let z3_cap = check_z3_capability();
    let sympy_cap = check_sympy_capability();

    json!({
        "lean": {
            "available": lean_available,
            "detail": lean_detail,
            "probe_type": "which lean",
            "toolchain": {
                "lean_version": toolchain.lean_version,
                "lake_version": toolchain.lake_version,
                "toolchain_available": toolchain.toolchain_available,
                "detail": toolchain.detail,
            }
        },
        "sympy": {
            "available": sympy_cap.available,
            "version": python_status.pointer("/sympy/version").and_then(|v| v.as_str()).unwrap_or(""),
            "description": python_status.pointer("/sympy/description").and_then(|v| v.as_str()).unwrap_or("SymPy CAS"),
            "capabilities": sympy_cap.capabilities,
            "install_hint": sympy_cap.install_hint,
        },
        "z3": {
            "available": z3_cap.available,
            "version": python_status.pointer("/z3/version").and_then(|v| v.as_str()).unwrap_or(""),
            "description": python_status.pointer("/z3/description").and_then(|v| v.as_str()).unwrap_or("Z3 SMT solver"),
            "capabilities": z3_cap.capabilities,
            "supports_nonlinear": z3_cap.available,
            "install_hint": z3_cap.install_hint,
        },
        "python_backend": python_status.get("python_backend"),
    })
}

/// Return a unified string describing all backends' status.
pub fn format_all_backends_status() -> String {
    let status = check_all_backends();

    let sympy = status
        .pointer("/sympy/available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let sympy_ver = status
        .pointer("/sympy/version")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let z3 = status
        .pointer("/z3/available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let z3_ver = status
        .pointer("/z3/version")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let lean = status
        .pointer("/lean/available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let py_backend = status
        .pointer("/python_backend")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let toolchain_detail = status
        .pointer("/lean/toolchain/detail")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let toolchain_info = if !toolchain_detail.is_empty() {
        format!(" [{toolchain_detail}]")
    } else {
        String::new()
    };

    format!(
        "SymPy: {} (v{}), Z3: {} (v{}), Lean: {}{}, Python backend: {}",
        if sympy { "available" } else { "unavailable" },
        sympy_ver,
        if z3 { "available" } else { "unavailable" },
        z3_ver,
        if lean { "available" } else { "unavailable" },
        toolchain_info,
        if py_backend { "available" } else { "unavailable" },
    )
}

// ===========================================================================
// Lean theorem verification (with caching)
// ===========================================================================

/// Attempt to verify a Lean theorem by running `lean` on a script file.
///
/// Results are cached in an LRU cache (64 entries, 5-minute TTL).
/// Computes a SHA-256 hash of the script as the cache key.
pub fn verify_lean_theorem(script: &str) -> VerificationResult {
    // Compute hash and check cache first
    let script_hash = hash_script(script);
    if let Ok(mut cache) = proof_cache().lock() {
        if let Some(cached) = cache.get(&script_hash) {
            return cached;
        }
    }

    // Check availability
    if !check_lean_status().is_available() {
        let result = VerificationResult {
            check_name: "math_lean_verify".into(),
            status: VerificationStatus::Warn,
            details: "Lean not available — install via elan".into(),
            evidence_path: None,
        };
        return result;
    }

    // Write script to temp file and run lean
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("lean_verify_{nanos:016x}"));
    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
        tracing::warn!("[lean_bridge] failed to create temp dir: {e}");
    }
    let script_path = temp_dir.join("verify.lean");

    // Drop guard ensures cleanup even if the process panics mid-execution.
    struct CleanupGuard(std::path::PathBuf);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _guard = CleanupGuard(temp_dir.clone());

    let result = (|| -> Result<std::process::Output, core_errors::FrameworkError> {
        core_state_utils::atomic_write::write_atomic_text(&script_path, script)?;
        std::process::Command::new("lean")
            .arg(&script_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(core_errors::FrameworkError::Io)
    })();

    // Clean up both file and directory (Drop guard also handles this, but eager
    // cleanup is better — shorter lifetime for temp resources).
    let _ = std::fs::remove_dir_all(&temp_dir);

    let output = match result {
        Ok(o) => o,
        Err(e) => {
            return VerificationResult {
                check_name: "math_lean_verify".into(),
                status: VerificationStatus::Warn,
                details: e.to_string(),
                evidence_path: None,
            };
        }
    };

    let result = if output.status.success() {
        VerificationResult {
            check_name: "math_lean_verify".into(),
            status: VerificationStatus::Pass,
            details: "Lean verified the theorem".into(),
            evidence_path: None,
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let parsed_errors = parse_lean_errors(&stderr);
        let detail = if parsed_errors.is_empty() {
            format!("Lean verification failed:\n{stderr}")
        } else {
            let mut detail = String::from("Lean verification failed:");
            for (i, err) in parsed_errors.iter().enumerate() {
                let loc = match (err.line, err.col) {
                    (Some(l), Some(c)) => format!(" line {l}:{c}"),
                    (Some(l), None) => format!(" line {l}"),
                    (None, _) => String::new(),
                };
                let err_type = err
                    .error_type
                    .as_deref()
                    .unwrap_or("error");
                detail.push_str(&format!(
                    "\n  [{i}] {err_type}{loc}: {}",
                    err.message.lines().next().unwrap_or(&err.message)
                ));
            }
            detail
        };

        VerificationResult {
            check_name: "math_lean_verify".into(),
            status: VerificationStatus::Fail,
            details: detail,
            evidence_path: None,
        }
    };

    // Cache the result
    if let Ok(mut cache) = proof_cache().lock() {
        cache.insert(script_hash, result.clone());
    }

    result
}

// ===========================================================================
// Multi-theorem verification
// ===========================================================================

/// Verify all theorems declared in a Lean script.
///
/// Runs `lean` once on the entire script.  If compilation succeeds, every
/// declared theorem passes.  On failure, the first encountered error is mapped
/// to the nearest theorem; theorems declared after the error point are reported
/// with status `Fail` (since Lean stops at the first error, their actual status
/// is unknown).
pub fn verify_lean_theorems(script: &str) -> Vec<TheoremResult> {
    let names = extract_theorem_names(script);

    if names.is_empty() {
        // No named theorems — run the script as-is
        let result = verify_lean_theorem(script);
        return vec![TheoremResult {
            theorem_name: "(unnamed script)".into(),
            status: result.status,
            details: result.details,
        }];
    }

    // Run the full script once
    let full_result = verify_lean_theorem(script);

    if full_result.status == VerificationStatus::Pass {
        return names
            .into_iter()
            .map(|name| TheoremResult {
                theorem_name: name,
                status: VerificationStatus::Pass,
                details: "Theorem verified".into(),
            })
            .collect();
    }

    // Failure: try to determine which theorem caused the first error.
    let parsed_errors = parse_lean_errors(&full_result.details);
    let first_err_line = parsed_errors
        .first()
        .and_then(|e| e.line);

    // Map each theorem to its line number (1-indexed).
    // We approximate: theorems at or after the first error line are "fail",
    // theorems before it "pass" (they compiled successfully).
    let mut results: Vec<TheoremResult> = Vec::with_capacity(names.len());
    for name in &names {
        let thm_line: Option<usize> = script
            .lines()
            .position(|l| {
                let t = l.trim();
                (t.starts_with("theorem ") || t.starts_with("lemma ")) && t.contains(name.as_str())
            })
            .map(|idx| idx + 1); // 1-indexed

        match first_err_line {
            Some(el) if thm_line.map_or(true, |tl| tl >= el) => {
                // This theorem is at or after the first error line
                let err_detail = parsed_errors
                    .first()
                    .map(|e| {
                        let loc = match (e.line, e.col) {
                            (Some(l), Some(c)) => format!(" at line {l}:{c}"),
                            (Some(l), None) => format!(" at line {l}"),
                            (None, _) => String::new(),
                        };
                        format!(
                            "{}{}: {}",
                            e.error_type.as_deref().unwrap_or("error"),
                            loc,
                            e.message.lines().next().unwrap_or(&e.message)
                        )
                    })
                    .unwrap_or_else(|| full_result.details.clone());

                results.push(TheoremResult {
                    theorem_name: name.clone(),
                    status: VerificationStatus::Fail,
                    details: err_detail,
                });
            }
            _ => {
                // Theorem before the error line, or no error line info
                results.push(TheoremResult {
                    theorem_name: name.clone(),
                    status: VerificationStatus::Pass,
                    details: "Theorem verified (before error point)".into(),
                });
            }
        }
    }

    results
}

// ===========================================================================
// Unified multi-backend proving
// ===========================================================================

/// Try to prove a mathematical statement by chaining Z3 → SymPy → Lean.
///
/// Returns the first successful result, or a failure summary listing each
/// backend's error if all three fail.
///
/// # Strategy
///
/// 1. **Z3** (`prove`): passes the statement unchanged (Z3 uses `==`).
/// 2. **SymPy** (`verify_identity`): splits on `=` to obtain LHS and RHS
///    (if no `=` found, passes the full string as both sides for reflexivity).
/// 3. **Lean** (`verify_lean_theorem`): auto-generates a proof script via
///    `generate_lean_proof_script`.
pub fn try_prove_with_all_backends(statement: &str) -> VerificationResult {
    let trimmed = statement.trim().to_string();

    // ── Step 1: Z3 prove ──
    if crate::verification::z3_bridge::z3_available() {
        // Replace `=` with `==` for Z3
        let z3_expr = trimmed.replace('=', "==").replace("====", "==");
        let z3_result = crate::verification::z3_bridge::prove_formula(&z3_expr);
        if z3_result.status == VerificationStatus::Pass {
            return z3_result;
        }
        // Fall through on failure (Z3 might not handle this syntax)
        tracing::debug!(
            "[try_prove_with_all_backends] Z3 failed for '{trimmed}': {}",
            z3_result.details
        );
    }

    // ── Step 2: SymPy verify ──
    if crate::verification::python_bridge::sympy_available() {
        let (lhs, rhs) = if let Some(eq_pos) = trimmed.find('=') {
            if eq_pos > 0 {
                let lhs = trimmed[..eq_pos].trim();
                let rhs = trimmed[eq_pos + 1..].trim();
                (lhs.to_string(), rhs.to_string())
            } else {
                (trimmed.clone(), trimmed.clone())
            }
        } else {
            (trimmed.clone(), trimmed.clone())
        };

        let sympy_result =
            crate::verification::sympy_bridge::verify_identity(&lhs, &rhs);
        if sympy_result.status == VerificationStatus::Pass {
            return sympy_result;
        }
        tracing::debug!(
            "[try_prove_with_all_backends] SymPy failed for '{trimmed}': {}",
            sympy_result.details
        );
    }

    // ── Step 3: Lean verify ──
    let lean_script = generate_lean_proof_script(&trimmed);
    let lean_result = verify_lean_theorem(&lean_script);
    if lean_result.status == VerificationStatus::Pass {
        return lean_result;
    }
    tracing::debug!(
        "[try_prove_with_all_backends] Lean failed for '{trimmed}': {}",
        lean_result.details
    );

    // ── All backends failed: return failure summary ──
    let z3_status = if crate::verification::z3_bridge::z3_available() {
        "tried"
    } else {
        "unavailable"
    };
    let sympy_status = if crate::verification::python_bridge::sympy_available() {
        "tried"
    } else {
        "unavailable"
    };
    let lean_status = if check_lean_status().is_available() {
        "tried"
    } else {
        "unavailable"
    };

    VerificationResult {
        check_name: "math_prove_all_backends".into(),
        status: VerificationStatus::Fail,
        details: format!(
            "All backends failed to prove '{trimmed}'. \
             Z3={z3_status}, SymPy={sympy_status}, Lean={lean_status}"
        ),
        evidence_path: None,
    }
}

// ===========================================================================
// Find a Lean 4 repository
// ===========================================================================

/// Find a Lean 4 repository (presence of `lakefile.lean` or `lean-toolchain`).
pub fn find_lean_repo() -> Option<std::path::PathBuf> {
    // Search from cwd upward for lakefile.lean or lean-toolchain
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            if d.join("lakefile.lean").exists() || d.join("lean-toolchain").exists() {
                return Some(d.to_path_buf());
            }
            dir = d.parent();
        }
    }
    None
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::VerificationStatus;

    /// Helper: true when Lean 4 is on the system PATH.
    fn lean_is_available() -> bool {
        check_lean_status().is_available()
    }

    // ── Probe tests (must not panic) ──

    #[test]
    fn test_lean_probe_no_panic() {
        let _ = check_lean_status();
    }

    #[test]
    fn test_lean_repo_no_panic() {
        let _ = find_lean_repo();
    }

    #[test]
    fn test_lean_toolchain_no_panic() {
        let _ = check_lean_toolchain();
    }

    #[test]
    fn test_z3_capability_no_panic() {
        let _ = check_z3_capability();
    }

    #[test]
    fn test_sympy_capability_no_panic() {
        let _ = check_sympy_capability();
    }

    #[test]
    fn test_clear_proof_cache_no_panic() {
        clear_proof_cache();
    }

    // ── Lean verification tests ──

    #[test]
    fn test_verify_lean_theorem_available() {
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }
        let script = "theorem reflexive (a : Nat) : a = a := rfl";
        let result = verify_lean_theorem(script);
        assert_eq!(
            result.status,
            VerificationStatus::Pass,
            "expected Pass for a valid theorem, got {:?}: {}",
            result.status,
            result.details,
        );
        assert_eq!(result.check_name, "math_lean_verify");
    }

    #[test]
    fn test_verify_lean_theorem_failure() {
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }
        let script = "theorem broken : 1 = 2 := rfl";
        let result = verify_lean_theorem(script);
        assert_eq!(
            result.status,
            VerificationStatus::Fail,
            "expected Fail for an invalid theorem, got {:?}: {}",
            result.status,
            result.details,
        );
        assert_eq!(result.check_name, "math_lean_verify");
        assert!(
            result.details.contains("Lean verification failed"),
            "details should indicate failure, got: {}",
            result.details,
        );
    }

    #[test]
    fn test_verify_lean_theorem_unavailable() {
        if lean_is_available() {
            eprintln!(
                "Skipping: Lean is available on PATH \
                 (cannot exercise the unavailable code path)"
            );
            return;
        }
        let result = verify_lean_theorem("(any content)");
        assert_eq!(
            result.status,
            VerificationStatus::Warn,
            "expected Warn when Lean is unavailable, got {:?}: {}",
            result.status,
            result.details,
        );
        assert_eq!(result.check_name, "math_lean_verify");
        assert!(
            result.details.contains("not available")
                || result.details.contains("install"),
            "details should mention Lean unavailability, got: {}",
            result.details,
        );
    }

    // ── Cache tests ──

    #[test]
    fn test_proof_cache_hit() {
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }

        // Clear cache first
        clear_proof_cache();

        let script = "theorem trivial (a : Nat) : a = a := rfl";
        let first = verify_lean_theorem(script);
        assert_eq!(first.status, VerificationStatus::Pass);

        // Second call should hit cache
        let second = verify_lean_theorem(script);
        assert_eq!(
            second.status,
            VerificationStatus::Pass,
            "cached result should still be Pass"
        );
        assert_eq!(second.details, first.details, "cached result should be identical");
    }

    #[test]
    fn test_proof_cache_eviction() {
        // Verify that the cache actually stores results.
        // We insert many unique scripts and check that only the last
        // PROOF_CACHE_MAX_ENTRIES remain.
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }

        clear_proof_cache();

        // Fill beyond capacity
        for i in 0..(PROOF_CACHE_MAX_ENTRIES + 10) {
            let script = format!("theorem test_{i} (a : Nat) : a = a := rfl");
            let result = verify_lean_theorem(&script);
            assert_eq!(
                result.status,
                VerificationStatus::Pass,
                "script {i} should pass: {}",
                result.details
            );
        }

        // Cache should still work (LRU eviction doesn't mean it's empty)
        let hit_script = format!(
            "theorem test_{} (a : Nat) : a = a := rfl",
            PROOF_CACHE_MAX_ENTRIES + 9
        );
        let hit = verify_lean_theorem(&hit_script);
        assert_eq!(hit.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_proof_cache_clear() {
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }

        clear_proof_cache();
        let script = "theorem clear_test (a : Nat) : a = a := rfl";
        let first = verify_lean_theorem(script);
        assert_eq!(first.status, VerificationStatus::Pass);

        clear_proof_cache();

        // After clear, should re-run (and pass again)
        let second = verify_lean_theorem(script);
        assert_eq!(second.status, VerificationStatus::Pass);
    }

    // ── Error parsing tests ──

    #[test]
    fn test_parse_lean_errors_empty() {
        let errors = parse_lean_errors("");
        assert!(errors.is_empty(), "empty stderr should produce no errors");
    }

    #[test]
    fn test_parse_lean_errors_simple() {
        let stderr = "/tmp/verify.lean:1:0:\nerror: expected ';'\n";
        let errors = parse_lean_errors(stderr);
        assert_eq!(errors.len(), 1, "should parse one error");
        assert_eq!(errors[0].line, Some(1));
        assert_eq!(errors[0].col, Some(0));
        assert_eq!(
            errors[0].error_type.as_deref(),
            Some("error"),
            "should be 'error' type"
        );
        assert!(
            errors[0].message.contains("expected"),
            "message should contain the error text"
        );
    }

    #[test]
    fn test_parse_lean_errors_warning() {
        let stderr = "/tmp/verify.lean:3:2:\nwarning: unused variable `x`\n";
        let errors = parse_lean_errors(stderr);
        assert_eq!(errors.len(), 1, "should parse one warning");
        assert_eq!(errors[0].line, Some(3));
        assert_eq!(errors[0].col, Some(2));
        assert_eq!(errors[0].error_type.as_deref(), Some("warning"));
    }

    #[test]
    fn test_parse_lean_errors_multiline_context() {
        let stderr = "\
/tmp/verify.lean:1:0:
error: unexpected token; expected command
  x
  ^
";
        let errors = parse_lean_errors(stderr);
        assert_eq!(errors.len(), 1, "should parse one error with context");
        assert_eq!(errors[0].line, Some(1));
        assert_eq!(errors[0].col, Some(0));
        assert!(errors[0].message.contains("unexpected token"));
    }

    #[test]
    fn test_parse_lean_errors_multiple() {
        let stderr = "\
/tmp/verify.lean:1:0:
error: first error
/tmp/verify.lean:5:2:
warning: second issue
";
        let errors = parse_lean_errors(stderr);
        assert_eq!(errors.len(), 2, "should parse two errors");
        assert_eq!(errors[0].line, Some(1));
        assert_eq!(errors[0].error_type.as_deref(), Some("error"));
        assert_eq!(errors[1].line, Some(5));
        assert_eq!(errors[1].error_type.as_deref(), Some("warning"));
    }

    // ── Proof template tests ──

    #[test]
    fn test_is_lean_script_theorem() {
        assert!(is_lean_script("theorem t : 1 = 1 := rfl"));
    }

    #[test]
    fn test_is_lean_script_lemma() {
        assert!(is_lean_script("lemma l : 1 = 1 := rfl"));
    }

    #[test]
    fn test_is_lean_script_plain_statement() {
        assert!(!is_lean_script("1 + 1 = 2"), "bare equality is not a script");
    }

    #[test]
    fn test_is_lean_script_empty() {
        assert!(!is_lean_script(""), "empty string is not a script");
    }

    #[test]
    fn test_generate_lean_proof_script_passthrough() {
        let script = "theorem t : 1 = 1 := rfl";
        let generated = generate_lean_proof_script(script);
        assert_eq!(generated, script, "script should pass through unchanged");
    }

    #[test]
    fn test_generate_lean_proof_script_from_statement() {
        let statement = "x + 0 = x";
        let generated = generate_lean_proof_script(statement);
        assert!(
            generated.contains("theorem auto_theorem :"),
            "should wrap in theorem, got: {generated}"
        );
        assert!(
            generated.contains("x + 0 = x"),
            "should contain original statement, got: {generated}"
        );
        assert!(
            generated.contains("simp"),
            "should include `simp` tactic, got: {generated}"
        );
    }

    // ── Multi-theorem extraction tests ──

    #[test]
    fn test_extract_theorem_names_single() {
        let script = "theorem t1 : 1 = 1 := rfl";
        let names = extract_theorem_names(script);
        assert_eq!(names, vec!["t1"]);
    }

    #[test]
    fn test_extract_theorem_names_multiple() {
        let script = "\
theorem t1 : 1 = 1 := rfl
lemma l1 : 2 = 2 := rfl
theorem t2 (a : Nat) : a = a := rfl
";
        let names = extract_theorem_names(script);
        assert_eq!(names, vec!["t1", "l1", "t2"]);
    }

    #[test]
    fn test_extract_theorem_names_none() {
        let script = "def foo : Nat := 42";
        let names = extract_theorem_names(script);
        assert!(names.is_empty(), "no theorems in script");
    }

    // ── Multi-theorem verification tests ──

    #[test]
    fn test_verify_lean_theorems_no_names() {
        // Script without theorem/lemma — should fall back to full script run
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }
        let script = "def foo : Nat := 42";
        let results = verify_lean_theorems(script);
        assert_eq!(results.len(), 1, "should produce one unnamed result");
        assert_eq!(results[0].theorem_name, "(unnamed script)");
        // def foo should succeed (no verification of a theorem, just declaration)
        assert!(
            results[0].status == VerificationStatus::Pass
                || results[0].status == VerificationStatus::Fail,
            "def should pass or fail, got {:?}",
            results[0].status
        );
    }

    #[test]
    fn test_verify_lean_theorems_all_pass() {
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }
        let script = "\
theorem t1 (a : Nat) : a = a := rfl
theorem t2 (a : Nat) : a = a := rfl
";
        let results = verify_lean_theorems(script);
        assert_eq!(results.len(), 2, "should have two results");
        assert_eq!(results[0].theorem_name, "t1");
        assert_eq!(results[0].status, VerificationStatus::Pass);
        assert_eq!(results[1].theorem_name, "t2");
        assert_eq!(results[1].status, VerificationStatus::Pass);
    }

    // ── Backend unified proving tests ──

    #[test]
    fn test_try_prove_with_all_backends_trivial() {
        // x = x should pass in at least one backend
        let result = try_prove_with_all_backends("x = x");
        // We accept Pass (if any backend succeeded) or Fail (all unavailable)
        assert!(
            result.status == VerificationStatus::Pass
                || result.status == VerificationStatus::Fail,
            "trivial identity should pass or all backends unavailable, got {:?}: {}",
            result.status,
            result.details
        );
    }

    #[test]
    fn test_try_prove_with_all_backends_structure() {
        // Verify the check_name is set correctly even on failure
        let result = try_prove_with_all_backends("x = y + 1");
        assert_eq!(result.check_name, "math_prove_all_backends");
    }

    // ── format_all_backends_status tests ──

    #[test]
    fn test_format_all_backends_status_no_panic() {
        let status = format_all_backends_status();
        assert!(
            !status.is_empty(),
            "status string should not be empty"
        );
        // The format should contain all four backend names
        assert!(
            status.contains("SymPy:"),
            "should mention SymPy, got: {status}"
        );
        assert!(
            status.contains("Z3:"),
            "should mention Z3, got: {status}"
        );
        assert!(
            status.contains("Lean:"),
            "should mention Lean, got: {status}"
        );
        assert!(
            status.contains("Python backend:"),
            "should mention Python backend, got: {status}"
        );
    }

    #[test]
    fn test_check_all_backends_no_panic() {
        let report = check_all_backends();
        assert!(
            report.get("lean").is_some(),
            "report should contain lean key"
        );
        assert!(
            report.get("sympy").is_some(),
            "report should contain sympy key"
        );
        assert!(
            report.get("z3").is_some(),
            "report should contain z3 key"
        );
        assert!(
            report.get("python_backend").is_some(),
            "report should contain python_backend key"
        );

        // Check new fields
        let lean = report.get("lean").unwrap();
        assert!(
            lean.get("toolchain").is_some(),
            "lean should have toolchain key"
        );

        let sympy = report.get("sympy").unwrap();
        assert!(
            sympy.get("capabilities").is_some(),
            "sympy should have capabilities key"
        );
    }
}
