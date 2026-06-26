//! Lossless prompt compression (`build_framework_prompt_compression_envelope`).
//!
//! ## Strategy: HTMC (Hierarchical Tiered Memory Compression)
//!
//! Instead of truncating text (structured_head_tail / tail_truncation), the
//! engine stores content sections in a content-addressed store and resolves
//! as many high-priority sections as fit within the token budget.  Offloaded
//! sections are replaced with human-readable `[ref:…]` placeholders that the
//! LLM can retrieve via `resolve_content`.
//!
//! Guarantee: every byte of the original prompt is recoverable from the store.
//! This is NOT compression — it is structured offloading with on-demand retrieval.
//!
//! ## Artifact root
//!
//! The caller **must** pass `"artifact_root"` in the payload when offloading may
//! be needed (input exceeds token budget).  The content store lives at
//! `{artifact_root}/{CONTENT_STORE_DIR}/`.  When everything fits in budget no
//! store is created — artifact_root is not required for that case.

use crate::alias;
use crate::content_store::ContentStore;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

use fr_utils::constants::{
    FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY,
    FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
};
use fr_utils::json_value::value_text;

// ── Exposed entry point ────────────────────────────────────────────────────────

pub fn build_framework_prompt_compression_envelope(
    payload: Value,
    context_window_size: Option<usize>,
) -> Result<Value, String> {
    let text = value_text(payload.get("prompt").or_else(|| payload.get("text")));
    if text.is_empty() {
        return Err("framework prompt compression requires prompt or text field".to_string());
    }
    let token_budget = payload
        .get("token_budget")
        .or_else(|| payload.get("budget"))
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .or_else(|| context_window_size.filter(|&s| s > 0).map(|s| s / 4))
        .ok_or_else(|| {
            "framework prompt compression requires token_budget or budget, or context_window_size"
                .to_string()
        })?;

    // Check the lossless-mode env flag (default on).
    let lossless = std::env::var("FRAMEWORK_COMPRESSION_LOSSLESS")
        .ok()
        .map(|v| v != "0" && v != "false")
        .unwrap_or(true);

    if !lossless {
        return compress_legacy(&text, token_budget);
    }

    let input_estimate = alias::estimate_token_count(&text);
    if token_budget == 0 {
        return Ok(zero_budget_output(input_estimate));
    }
    if input_estimate <= token_budget {
        return Ok(full_output(&text, input_estimate));
    }

    // Offloading is needed — resolve the artifact root for the content store.
    let artifact_root = payload
        .get("artifact_root")
        .and_then(Value::as_str)
        .map(Path::new)
        .ok_or_else(|| {
            format!(
                "artifact_root is required when input exceeds token budget \
                 (input={input_estimate}, budget={token_budget}); \
                 set in payload or via RUNTIME_REGISTRY.json::{field}",
                field = "runtime_artifact_root",
            )
        })?;

    compress_lossless(&text, token_budget, artifact_root)
}

// ── Lossless compression (HTMC) ────────────────────────────────────────────────

fn compress_lossless(
    text: &str,
    token_budget: usize,
    artifact_root: &Path,
) -> Result<Value, String> {
    let input_estimate = alias::estimate_token_count(text);

    let store = ContentStore::new(artifact_root);

    // Best-effort GC: remove content-store entries older than the configured threshold.
    // Override via FRAMEWORK_CONTENT_STORE_MAX_AGE_DAYS env var (default 7 days).
    let max_age_days: u64 = std::env::var("FRAMEWORK_CONTENT_STORE_MAX_AGE_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7);
    let _ = store.remove_stale(Duration::from_secs(max_age_days * 24 * 3600));

    // Store the full text so it is recoverable even after GC clears paragraph files.
    store.put(text)?;

    // Normalize line endings so paragraph splitting works on all platforms.
    let text = text.replace("\r\n", "\n");

    // 1. Split text into paragraphs.
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect();

    let total = paragraphs.len();

    // 2. Store each paragraph and assign priority.
    struct Section {
        hash: String,
        text: String,
        byte_size: usize,
        priority: u8,
        index: usize,
    }

    let mut sections: Vec<Section> = Vec::with_capacity(paragraphs.len());
    for (i, p) in paragraphs.into_iter().enumerate() {
        let hash = store.put(p)?;
        let priority = classify_priority(p, i, total);
        sections.push(Section {
            hash,
            text: p.to_string(),
            byte_size: p.len(),
            priority,
            index: i,
        });
    }

    // 3. Sort by priority (lower = higher), stable by original index.
    sections.sort_by_key(|s| (s.priority, s.index));

    // 4. Resolve as many sections as fit within budget.
    let mut resolved_parts: Vec<(usize, String)> = Vec::new();
    let mut offloaded: Vec<Value> = Vec::new();
    let mut available = token_budget;

    for sec in &sections {
        let cost = alias::estimate_token_count(&sec.text);
        if cost <= available {
            resolved_parts.push((sec.index, sec.text.clone()));
            available = available.saturating_sub(cost);
        } else {
            let hint = truncate_hint(&sec.text, 80);
            offloaded.push(json!({
                "hash": sec.hash,
                "hint": hint,
                "byte_size": sec.byte_size,
                "priority": sec.priority,
            }));
        }
    }

    // 5. Restore original order.
    resolved_parts.sort_by_key(|(idx, _)| *idx);
    let resolved_text: String = resolved_parts
        .into_iter()
        .map(|(_, t)| t)
        .collect::<Vec<_>>()
        .join("\n\n");

    // 6. Build offloaded placeholder section (metadata, not counted in budget).
    let offloaded_text: String = if offloaded.is_empty() {
        String::new()
    } else {
        let lines: Vec<String> = offloaded
            .iter()
            .map(|o| {
                let h = o["hash"].as_str().unwrap_or("?");
                let hint = o["hint"].as_str().unwrap_or("");
                format!(
                    "[ref:{h} — {hint} ({size} B)]",
                    size = o["byte_size"].as_u64().unwrap_or(0),
                )
            })
            .collect();
        format!("---\n## Offloaded content\n\n{}", lines.join("\n"))
    };

    let final_text = if offloaded_text.is_empty() {
        resolved_text
    } else {
        format!("{resolved_text}\n\n{offloaded_text}")
    };

    let output_estimate = alias::estimate_token_count(&final_text);

    Ok(json!({
        "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
        "authority": FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY,
        "compression": {
            "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
            "policy_owner": "rust",
            "prompt_policy_owner": "rust",
            "input_token_estimate": input_estimate,
            "output_token_estimate": output_estimate,
            "output": &final_text,
            "compressed_prompt": &final_text,
            "strategy": "hierarchical_lossless",
            "truncated": !offloaded.is_empty(),
            "recoverable": true,
            "content_refs": sections.iter().map(|s| json!({
                "hash": s.hash,
                "byte_size": s.byte_size,
                "priority": s.priority,
                "in_prompt": !offloaded.iter().any(|o| o["hash"] == s.hash),
            })).collect::<Vec<_>>(),
            "offloaded_refs": offloaded,
            "artifact_offload_decision": !offloaded.is_empty(),
        }
    }))
}

// ── Priority classification ────────────────────────────────────────────────────

fn classify_priority(paragraph: &str, index: usize, total: usize) -> u8 {
    let trimmed = paragraph.trim();

    // Structural markers → P0 (always include)
    if trimmed.starts_with("# ")
        || trimmed.starts_with("##")
        || trimmed.starts_with("---")
        || trimmed.starts_with("===")
        || trimmed.starts_with("```")
    {
        return 0;
    }

    // Very short single-line text that looks like a section title → P0.
    // Exclude short data outputs (JSON, key=value, quotes) from false positives.
    if trimmed.len() < 60
        && !trimmed.contains('\n')
        && !trimmed.contains(['{', '"', '='].as_slice())
    {
        return 0;
    }

    // First section → P0 (likely system instruction)
    if index == 0 {
        return 0;
    }

    // Second section → P1
    if index == 1 {
        return 1;
    }

    // Long tool outputs / evidence blocks → P3 (first to offload)
    if trimmed.len() > 5000 {
        return 3;
    }

    // Middle content → P2
    if index < total / 2 {
        return 2;
    }

    // Later sections → P2
    2
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn truncate_hint(text: &str, max: usize) -> String {
    let first_line = text.lines().next().unwrap_or(text);
    if first_line.chars().count() <= max {
        first_line.trim().to_string()
    } else {
        format!(
            "{}…",
            first_line
                .chars()
                .take(max.saturating_sub(1))
                .collect::<String>()
                .trim()
        )
    }
}


// ── Zero-budget / full-output helpers ──────────────────────────────────────────

fn zero_budget_output(input_estimate: usize) -> Value {
    let output = "[omitted: token budget is zero]";
    let output_estimate = alias::estimate_token_count(output);
    json!({
        "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
        "authority": FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY,
        "compression": {
            "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
            "policy_owner": "rust",
            "prompt_policy_owner": "rust",
            "input_token_estimate": input_estimate,
            "output_token_estimate": output_estimate,
            "output": output,
            "compressed_prompt": output,
            "strategy": "zero_budget",
            "truncated": true,
            "recoverable": false,
            "omitted_sections": ["all"],
            "artifact_offload_decision": false,
        }
    })
}

fn full_output(text: &str, estimate: usize) -> Value {
    json!({
        "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
        "authority": FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY,
        "compression": {
            "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
            "policy_owner": "rust",
            "prompt_policy_owner": "rust",
            "input_token_estimate": estimate,
            "output_token_estimate": estimate,
            "output": text,
            "compressed_prompt": text,
            "strategy": "hierarchical_lossless",
            "truncated": false,
            "recoverable": true,
            "content_refs": [],
            "offloaded_refs": [],
            "artifact_offload_decision": false,
        }
    })
}

// ── Legacy fallback (FRAMEWORK_COMPRESSION_LOSSLESS=false) ──────────────────────

fn compress_legacy(prompt: &str, token_budget: usize) -> Result<Value, String> {
    let input_token_estimate = alias::estimate_token_count(prompt);
    if token_budget == 0 {
        let output = "[omitted: token budget is zero]".to_string();
        return Ok(legacy_payload(
            input_token_estimate,
            alias::estimate_token_count(&output),
            &output,
            "zero_budget",
            true,
            &["all".to_string()],
        ));
    }
    if input_token_estimate <= token_budget {
        return Ok(legacy_payload(
            input_token_estimate,
            input_token_estimate,
            prompt,
            "unchanged",
            false,
            &[],
        ));
    }

    let lines = prompt.lines().collect::<Vec<_>>();
    let target_chars = token_budget.saturating_mul(3).max(1);
    let (output, strategy, omitted_sections) = if lines.len() >= 6 {
        let head = lines
            .iter()
            .take(3)
            .map(|l| (*l).to_string())
            .collect::<Vec<_>>();
        let tail = lines
            .iter()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|l| (*l).to_string())
            .collect::<Vec<_>>();
        let omitted = lines.len().saturating_sub(head.len() + tail.len());
        (
            [
                head,
                vec![format!("[omitted {omitted} middle lines]")],
                tail,
            ]
            .concat()
            .join("\n"),
            "structured_head_tail".to_string(),
            vec![format!("middle_lines:{omitted}")],
        )
    } else {
        let mut truncated = prompt.chars().take(target_chars).collect::<String>();
        truncated.push_str("\n[truncated tail]");
        (
            truncated,
            "tail_truncation".to_string(),
            vec!["tail".to_string()],
        )
    };
    let bounded = enforce_prompt_budget_legacy(output, token_budget);
    Ok(legacy_payload(
        input_token_estimate,
        alias::estimate_token_count(&bounded),
        &bounded,
        &strategy,
        true,
        &omitted_sections,
    ))
}

fn enforce_prompt_budget_legacy(output: String, token_budget: usize) -> String {
    let max_chars = token_budget.saturating_mul(3).max(1);
    let marker = "\n[truncated tail]";
    if output.char_indices().nth(max_chars).is_none() {
        return output;
    }
    let marker_char_count = marker.chars().count();
    if max_chars <= marker_char_count {
        return "[truncated]".chars().take(max_chars).collect();
    }
    let keep = max_chars - marker_char_count;
    let split_byte = output
        .char_indices()
        .nth(keep)
        .map(|(idx, _)| idx)
        .unwrap_or(output.len());
    format!("{}{}", &output[..split_byte], marker)
}

fn legacy_payload(
    input_estimate: usize,
    output_estimate: usize,
    output: &str,
    strategy: &str,
    truncated: bool,
    omitted_sections: &[String],
) -> Value {
    json!({
        "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
        "authority": FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY,
        "compression": {
            "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
            "policy_owner": "rust",
            "prompt_policy_owner": "rust",
            "input_token_estimate": input_estimate,
            "output_token_estimate": output_estimate,
            "output": output,
            "compressed_prompt": output,
            "strategy": strategy,
            "truncated": truncated,
            "omitted_sections": omitted_sections,
            "artifact_offload_decision": false,
        }
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_when_within_budget() {
        let text = "Hello, world!";
        let result = build_framework_prompt_compression_envelope(
            json!({"prompt": text, "token_budget": 100}),
            None,
        )
        .expect("compression");
        let comp = &result["compression"];
        assert_eq!(comp["strategy"], "hierarchical_lossless");
        assert!(!comp["truncated"].as_bool().unwrap());
        assert_eq!(comp["output"].as_str().unwrap(), text);
    }

    #[test]
    fn zero_budget() {
        let result = build_framework_prompt_compression_envelope(
            json!({"prompt": "some text", "token_budget": 0}),
            None,
        )
        .expect("compression");
        assert_eq!(result["compression"]["strategy"], "zero_budget");
        assert!(result["compression"]["truncated"].as_bool().unwrap());
    }

    #[test]
    fn offloads_when_budget_exceeded() {
        // 20 paragraphs, small budget
        let text = "A\n\nB\n\nC\n\nD\n\nE\n\nF\n\nG\n\nH\n\nI\n\nJ\n\nK\n\nL\n\nM\n\nN\n\nO\n\nP\n\nQ\n\nR\n\nS\n\nT";
        let dir = tempfile::tempdir().expect("tempdir");
        let result = build_framework_prompt_compression_envelope(
            json!({"prompt": text, "token_budget": 10, "artifact_root": dir.path()}),
            None,
        )
        .expect("compression");
        let comp = &result["compression"];
        assert_eq!(comp["strategy"], "hierarchical_lossless");
        assert!(comp["truncated"].as_bool().unwrap());
        assert!(comp["recoverable"].as_bool().unwrap());
        let refs = comp["offloaded_refs"].as_array().unwrap();
        assert!(!refs.is_empty(), "should have offloaded refs");
        // output should be non-empty text
        let output = comp["output"].as_str().unwrap();
        assert!(!output.is_empty(), "output should not be empty");
        // output should NOT contain [omitted] markers (the old lossy pattern)
        assert!(
            !output.contains("[omitted"),
            "output must not contain lossy [omitted] markers, got: {output:.100}"
        );
        // output should contain ref placeholders for offloaded sections
        assert!(
            output.contains("[ref:"),
            "output should contain [ref:...] placeholders"
        );
        // refs can be resolved from the store
        for oref in refs {
            let hash = oref["hash"].as_str().unwrap();
            let store = ContentStore::new(dir.path());
            let content = store.get(hash).expect("offloaded content must be retrievable");
            assert!(!content.is_empty(), "retrieved content must not be empty");
        }
    }

    #[test]
    fn offload_requires_artifact_root() {
        let text = "A\n\nB\n\nC\n\nD\n\nE\n\nF\n\nG\n\nH\n\nI\n\nJ\n\nK\n\nL\n\nM\n\nN\n\nO\n\nP\n\nQ\n\nR\n\nS\n\nT";
        let result = build_framework_prompt_compression_envelope(
            json!({"prompt": text, "token_budget": 10}),
            None,
        );
        assert!(result.is_err(), "should error when artifact_root is missing");
        let err = result.unwrap_err();
        assert!(
            err.contains("artifact_root"),
            "error should mention artifact_root: {err}"
        );
    }

    #[test]
    fn legacy_fallback_via_env() {
        let text = "line1\nline2\nline3\nline4\nline5\nline6";
        let result = compress_legacy(text, 5).expect("legacy");
        let comp = &result["compression"];
        assert_eq!(comp["strategy"], "structured_head_tail");
        assert!(comp["truncated"].as_bool().unwrap());
    }

    #[test]
    fn roundtrip_via_content_store() {
        use crate::content_store::ContentStore;
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ContentStore::new(dir.path());

        let paragraph = "# System instruction\n\nYou are a helpful assistant.";
        let hash = store.put(paragraph).expect("put");
        let retrieved = store.get(&hash).expect("get");
        assert_eq!(retrieved, paragraph);
    }

    #[test]
    fn short_text_no_offload() {
        let result = build_framework_prompt_compression_envelope(
            json!({"prompt": "Short text", "token_budget": 50}),
            None,
        )
        .expect("compression");
        assert!(!result["compression"]["truncated"].as_bool().unwrap());
    }

    #[test]
    fn classify_priority_test() {
        // P0: header-like
        assert_eq!(classify_priority("# Goal", 5, 10), 0);
        assert_eq!(classify_priority("```\ncode\n```", 5, 10), 0);
        // P0: first section
        assert_eq!(classify_priority("any text", 0, 10), 0);
        // P3: very long
        let long = "x".repeat(6000);
        assert_eq!(classify_priority(&long, 5, 10), 3);
    }

    #[test]
    fn hint_truncation() {
        let hint = truncate_hint(
            "This is a very long first line that should be truncated because it exceeds the max",
            40,
        );
        assert!(hint.chars().count() <= 41); // max + ellipsis
        assert!(hint.ends_with('…'));
    }

    #[test]
    fn legacy_tail_truncation() {
        let text = "short text";
        let output = enforce_prompt_budget_legacy(text.to_string(), 1);
        assert!(output.len() <= 3);
    }

    #[test]
    fn gc_runs_during_compression() {
        // Verify GC runs when compress_lossless is called with a zero max age.
        // 1. Create a ContentStore with pre-existing content.
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ContentStore::new(dir.path());
        let hash = store.put("stale content").expect("put");

        // 2. Call compression with a minimal GC threshold via env var.
        // SAFETY: env var manipulation is single-threaded in tests.
        unsafe { std::env::set_var("FRAMEWORK_CONTENT_STORE_MAX_AGE_DAYS", "0"); }
        let text = "A\n\nB\n\nC\n\nD\n\nE\n\nF\n\nG\n\nH\n\nI\n\nJ";
        let result = build_framework_prompt_compression_envelope(
            json!({"prompt": text, "token_budget": 5, "artifact_root": dir.path()}),
            None,
        );
        unsafe { std::env::remove_var("FRAMEWORK_CONTENT_STORE_MAX_AGE_DAYS"); }
        assert!(result.is_ok(), "compression should succeed");

        // 3. The pre-existing content should be gone (GC runs before new puts).
        //    NOTE: GC runs after the store is created, so old entries from step 1
        //    (created moments ago but with max_age=0) should be removed.
        let got = store.get(&hash);
        assert!(
            got.is_err(),
            "pre-existing content should be removed by GC with max_age=0"
        );

        // 4. Newly stored content (from the compression) should still exist.
        let refs = result
            .unwrap()["compression"]["offloaded_refs"]
            .as_array()
            .unwrap()
            .clone();
        for oref in &refs {
            let h = oref["hash"].as_str().unwrap();
            let content = store.get(h);
            assert!(
                content.is_ok(),
                "newly stored content should survive GC: {h}"
            );
        }
    }
}
