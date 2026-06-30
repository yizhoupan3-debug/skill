//! Compile-time validation of LOOP_REGISTRY.json and RUNTIME_REGISTRY.json.
//!
//! Uses serde_json::Value for structural checks without adding a JSON Schema
//! crate dependency. Validates that registry files have the expected top-level
//! fields and that loop entries have all required fields and no unrecognized
//! fields. Warnings are emitted via cargo:warning= so they appear during builds.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // CARGO_MANIFEST_DIR = <repo_root>/core/goal-engine
    // Navigate up to repo root (../..) to find configs/framework/
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let config_dir = manifest_dir
        .parent()
        .expect("core/goal-engine parent (core/)")
        .parent()
        .expect("core/ parent (repo root)")
        .join("configs")
        .join("framework");

    validate_loop_registry(&config_dir);
    validate_runtime_registry(&config_dir);
}

fn validate_loop_registry(dir: &PathBuf) {
    let path = dir.join("LOOP_REGISTRY.json");
    println!("cargo:rerun-if-changed={}", path.display());

    let json_str = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            println!("cargo:warning=Cannot read LOOP_REGISTRY.json: {e}");
            return;
        }
    };

    let reg: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            println!("cargo:warning=Cannot parse LOOP_REGISTRY.json: {e}");
            return;
        }
    };

    // Check top-level fields
    let known_top: HashSet<&str> = ["schema_version", "loops"].into();
    if let Some(obj) = reg.as_object() {
        for key in obj.keys() {
            if !known_top.contains(key.as_str()) {
                println!("cargo:warning=LOOP_REGISTRY.json: unknown top-level field '{key}'");
            }
        }
    }

    if reg.get("schema_version").and_then(|v| v.as_str()).is_none() {
        println!("cargo:warning=LOOP_REGISTRY.json: missing or invalid 'schema_version' (expected string)");
    }

    let loops = match reg.get("loops").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            println!("cargo:warning=LOOP_REGISTRY.json: missing or invalid 'loops' (expected array)");
            return;
        }
    };

    let required_loop_fields: HashSet<&str> = ["loop_id", "profile", "trigger"].into();
    let known_loop_fields: HashSet<&str> = [
        "loop_id", "profile", "trigger", "skill", "scope_based_safety",
        "default_safety", "scope_conflict_resolution", "cost_budget",
        "notification", "research_enabled", "research", "verify_quality_gate",
        "verify_closeout_gate", "static_actions", "subagent_protocol",
    ]
    .into();
    let known_research_fields: HashSet<&str> = [
        "barrier_threshold", "escalation_target", "max_research_time_min",
        "auto_resume", "require_human_approval", "freshness_window_min",
    ]
    .into();
    let known_trigger_fields: HashSet<&str> = ["type", "schedule", "timezone"].into();

    for (i, entry) in loops.iter().enumerate() {
        let obj = match entry.as_object() {
            Some(o) => o,
            None => {
                println!("cargo:warning=LOOP_REGISTRY.json: loops[{i}] is not an object");
                continue;
            }
        };

        // Check required fields
        for f in &required_loop_fields {
            if !obj.contains_key(*f) {
                println!("cargo:warning=LOOP_REGISTRY.json: loops[{i}] missing required field '{f}'");
            }
        }

        // Check for unknown fields
        for key in obj.keys() {
            if !known_loop_fields.contains(key.as_str()) {
                println!("cargo:warning=LOOP_REGISTRY.json: loops[{i}] unknown field '{key}'");
            }
        }

        // Validate research sub-object
        if let Some(research) = obj.get("research").and_then(|v| v.as_object()) {
            for key in research.keys() {
                if !known_research_fields.contains(key.as_str()) {
                    println!(
                        "cargo:warning=LOOP_REGISTRY.json: loops[{i}].research unknown field '{key}'"
                    );
                }
            }
        }

        // Validate trigger sub-object
        if let Some(trigger) = obj.get("trigger").and_then(|v| v.as_object()) {
            if !trigger.contains_key("type") {
                println!("cargo:warning=LOOP_REGISTRY.json: loops[{i}].trigger missing required field 'type'");
            }
            for key in trigger.keys() {
                if !known_trigger_fields.contains(key.as_str()) {
                    println!(
                        "cargo:warning=LOOP_REGISTRY.json: loops[{i}].trigger unknown field '{key}'"
                    );
                }
            }
        }
    }
}

fn validate_runtime_registry(dir: &PathBuf) {
    let path = dir.join("RUNTIME_REGISTRY.json");
    println!("cargo:rerun-if-changed={}", path.display());

    let json_str = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            println!("cargo:warning=Cannot read RUNTIME_REGISTRY.json: {e}");
            return;
        }
    };

    let reg: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            println!("cargo:warning=Cannot parse RUNTIME_REGISTRY.json: {e}");
            return;
        }
    };

    // Validate lifecycle_profiles section
    let known_profile_fields: HashSet<&str> = [
        "loop_capable", "closeout_mode", "review_gate", "spawn_first_nudge",
        "cost_budget", "escalation", "disable_spawn_first_nudge",
        "goal_continuation", "kill_switch", "verification_required",
        "interactive_capable", "pause_timeout_secs",
    ]
    .into();

    if let Some(profiles) = reg.get("lifecycle_profiles").and_then(|v| v.as_object()) {
        for (profile_name, profile_val) in profiles {
            let obj = match profile_val.as_object() {
                Some(o) => o,
                None => {
                    println!(
                        "cargo:warning=RUNTIME_REGISTRY.json: lifecycle_profiles.{profile_name} is not an object"
                    );
                    continue;
                }
            };
            for key in obj.keys() {
                if !known_profile_fields.contains(key.as_str()) {
                    println!(
                        "cargo:warning=RUNTIME_REGISTRY.json: lifecycle_profiles.{profile_name} unknown field '{key}'"
                    );
                }
            }
        }
    }

    // Verify top-level required fields exist
    for f in &["schema_version", "lifecycle_profiles"] {
        if reg.get(*f).is_none() {
            println!("cargo:warning=RUNTIME_REGISTRY.json: missing required top-level field '{f}'");
        }
    }
}
