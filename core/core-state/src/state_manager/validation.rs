// External research structured validation (claims, contradiction_sweep, retrieval_trace).
// Extracted from state_manager.rs during module split.

use serde_json::Value;

pub const EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN: usize = 40;

fn nonempty_trimmed_string_at(value: &Value, ctx: &str, key: &str) -> Result<(), String> {
    let Some(t) = value.as_str() else {
        return Err(format!("{ctx}: `{key}` must be string"));
    };
    if t.trim().is_empty() {
        return Err(format!("{ctx}: `{key}` must be non-empty"));
    }
    Ok(())
}

fn validate_nonempty_string_items(arr: &[Value], ctx: &str, arr_name: &str) -> Result<(), String> {
    if arr.is_empty() {
        return Err(format!("{ctx}: `{arr_name}` must be non-empty"));
    }
    for (idx, elem) in arr.iter().enumerate() {
        let label = format!("{ctx}.{arr_name}[{idx}]");
        nonempty_trimmed_string_at(elem, &label, "item")?;
    }
    Ok(())
}

pub fn source_traceable_heuristic(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return true;
    }
    if lower.starts_with("doi:10.") {
        return true;
    }
    if lower.starts_with("10.") && lower.contains('/') {
        return true;
    }
    for prefix in [
        "arxiv:",
        "pmid:",
        "isbn:",
        "dataset:",
        "official_doc:",
        "huggingface:",
        "hf:",
        "github:",
        "kaggle:",
        "geojson:",
    ] {
        if lower.starts_with(prefix) {
            return true;
        }
    }
    false
}

fn validate_source_list_traceable(
    sources: &[Value],
    ctx: &str,
    min_len: usize,
    err_label: &str,
) -> Result<(), String> {
    if sources.len() < min_len {
        return Err(format!(
            "external_research strict: {ctx} `{err_label}` must have at least {min_len} entries, got {}",
            sources.len()
        ));
    }
    for (j, sv) in sources.iter().enumerate() {
        let Some(s) = sv.as_str() else {
            return Err(format!(
                "external_research strict: {ctx} `{err_label}[{j}]` must be string"
            ));
        };
        if !source_traceable_heuristic(s) {
            return Err(format!(
                "external_research strict: {ctx} `{err_label}[{j}]` not traceable: {s:?}"
            ));
        }
    }
    Ok(())
}

/// Stricter checks when `RFV_LOOP_STATE.external_research_strict` is true; run only after
/// [`validate_external_research_structured`] succeeds.
pub fn validate_external_research_strict(v: &Value) -> Result<(), String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "external_research strict: root must be object".to_string())?;

    let Some(unk) = obj.get("unknowns") else {
        return Err(
            "external_research strict: missing `unknowns` key (use [] or null)".to_string(),
        );
    };
    if !unk.is_null() && !unk.is_array() {
        return Err("external_research strict: `unknowns` must be array or null".to_string());
    }

    let claims = obj
        .get("claims")
        .and_then(Value::as_array)
        .ok_or_else(|| "external_research strict: claims must be array".to_string())?;
    let claims_len = claims.len();

    let sweep = obj
        .get("contradiction_sweep")
        .and_then(Value::as_array)
        .ok_or_else(|| "external_research strict: contradiction_sweep must be array".to_string())?;
    let min_sweep = std::cmp::max(2, claims_len / 2);
    if sweep.len() < min_sweep {
        return Err(format!(
            "external_research strict: contradiction_sweep must have at least {min_sweep} entries, got {}",
            sweep.len()
        ));
    }
    for (i, item) in sweep.iter().enumerate() {
        let ctx = format!("contradiction_sweep[{i}]");
        let row = item
            .as_object()
            .ok_or_else(|| format!("external_research strict: {ctx} entry must be object"))?;
        let sources = row
            .get("sources")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("external_research strict: {ctx} sources must be array"))?;
        validate_source_list_traceable(sources, &ctx, 1, "sources")?;
    }

    for (i, c) in claims.iter().enumerate() {
        let ctx = format!("claims[{i}]");
        let row = c
            .as_object()
            .ok_or_else(|| format!("external_research strict: {ctx} must be object"))?;
        let sources = row
            .get("sources")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("external_research strict: {ctx} sources must be array"))?;
        validate_source_list_traceable(sources, &ctx, 2, "sources")?;
    }

    let trace = obj
        .get("retrieval_trace")
        .and_then(Value::as_object)
        .ok_or_else(|| "external_research strict: retrieval_trace must be object".to_string())?;
    let queries = trace
        .get("queries_used")
        .and_then(Value::as_array)
        .ok_or_else(|| "external_research strict: queries_used must be array".to_string())?;
    if queries.len() < 3 {
        return Err(format!(
            "external_research strict: queries_used must have at least 3 entries, got {}",
            queries.len()
        ));
    }

    for key in ["inclusion_rules", "exclusions", "exclusion_rationale"] {
        let field = trace.get(key).and_then(Value::as_str).ok_or_else(|| {
            format!("external_research strict: retrieval_trace `{key}` must be string")
        })?;
        if field.trim().len() < EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN {
            return Err(format!(
                "external_research strict: retrieval_trace `{key}` must be at least {} non-whitespace chars (trimmed len={})",
                EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN,
                field.trim().len()
            ));
        }
    }

    Ok(())
}


/// Validates optional structured external research blob for `append_round`.
/// Aligns with lane-templates **deep mode** YAML (`claims`, `contradiction_sweep`, `retrieval_trace`, optional `unknowns` / `quantitative_replays`).
pub fn validate_external_research_structured(v: &Value) -> Result<(), String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "external_research must be a JSON object".to_string())?;

    let claims = obj
        .get("claims")
        .ok_or_else(|| "external_research missing `claims`".to_string())?;
    let claims = claims
        .as_array()
        .ok_or_else(|| "external_research.claims must be array".to_string())?;
    if claims.is_empty() {
        return Err("external_research.claims must be non-empty".to_string());
    }
    for (i, c) in claims.iter().enumerate() {
        let ctx = format!("external_research.claims[{i}]");
        let row = c
            .as_object()
            .ok_or_else(|| format!("{ctx}: claim entry must be object"))?;
        let claim_v = row
            .get("claim")
            .ok_or_else(|| format!("{ctx}: missing `claim`"))?;
        nonempty_trimmed_string_at(claim_v, &ctx, "claim")?;
        let sources = row
            .get("sources")
            .ok_or_else(|| format!("{ctx}: missing `sources`"))?;
        let sources = sources
            .as_array()
            .ok_or_else(|| format!("{ctx}: sources must be array"))?;
        validate_nonempty_string_items(sources, &ctx, "sources")?;
    }

    let sweep_key = obj
        .get("contradiction_sweep")
        .ok_or_else(|| "external_research missing `contradiction_sweep`".to_string())?;
    let sweep = sweep_key
        .as_array()
        .ok_or_else(|| "external_research.contradiction_sweep must be array".to_string())?;
    if sweep.is_empty() {
        return Err("external_research.contradiction_sweep must be non-empty".to_string());
    }
    for (i, item) in sweep.iter().enumerate() {
        let ctx = format!("external_research.contradiction_sweep[{i}]");
        let row = item
            .as_object()
            .ok_or_else(|| format!("{ctx}: entry must be object"))?;
        let rk = row
            .get("related_claim_or_topic")
            .ok_or_else(|| format!("{ctx}: missing `related_claim_or_topic`"))?;
        nonempty_trimmed_string_at(rk, &ctx, "related_claim_or_topic")?;
        let contradict = row
            .get("contradicting_or_limiting_evidence")
            .ok_or_else(|| format!("{ctx}: missing `contradicting_or_limiting_evidence`"))?;
        nonempty_trimmed_string_at(contradict, &ctx, "contradicting_or_limiting_evidence")?;
        let sources = row
            .get("sources")
            .ok_or_else(|| format!("{ctx}: missing `sources`"))?;
        let sources = sources
            .as_array()
            .ok_or_else(|| format!("{ctx}: sources must be array"))?;
        validate_nonempty_string_items(sources, &ctx, "sources")?;
    }

    if let Some(u) = obj.get("unknowns") {
        if u.is_null() {
            // skip unknowns
        } else {
            let arr = u
                .as_array()
                .ok_or_else(|| "external_research.unknowns must be array or null".to_string())?;
            for (i, rowv) in arr.iter().enumerate() {
                let ctx = format!("external_research.unknowns[{i}]");
                let row = rowv
                    .as_object()
                    .ok_or_else(|| format!("{ctx}: entry must be object"))?;
                let q = row
                    .get("question")
                    .ok_or_else(|| format!("{ctx}: missing `question`"))?;
                nonempty_trimmed_string_at(q, &ctx, "question")?;
                let why = row
                    .get("why_insufficient")
                    .ok_or_else(|| format!("{ctx}: missing `why_insufficient`"))?;
                nonempty_trimmed_string_at(why, &ctx, "why_insufficient")?;
            }
        }
    }

    if let Some(qr) = obj.get("quantitative_replays") {
        if qr.is_null()
            || (qr
                .as_str()
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("none")))
        {
            // optional / explicit N/A sentinel
        } else if let Some(entries) = qr.as_array() {
            for (i, rowv) in entries.iter().enumerate() {
                let ctx = format!("external_research.quantitative_replays[{i}]");
                let row = rowv
                    .as_object()
                    .ok_or_else(|| format!("{ctx}: entry must be object"))?;
                for key in [
                    "dataset_or_source_id",
                    "version_or_snapshot",
                    "window",
                    "replay_command",
                ] {
                    let f = row
                        .get(key)
                        .ok_or_else(|| format!("{ctx}: missing `{key}`"))?;
                    nonempty_trimmed_string_at(f, &ctx, key)?;
                }
            }
        } else {
            return Err(
                "external_research.quantitative_replays must be array, null, \"none\", or absent"
                    .to_string(),
            );
        }
    }

    let trace = obj
        .get("retrieval_trace")
        .ok_or_else(|| "external_research missing `retrieval_trace`".to_string())?;
    let tr = trace
        .as_object()
        .ok_or_else(|| "external_research.retrieval_trace must be object".to_string())?;
    let queries = tr
        .get("queries_used")
        .ok_or_else(|| "retrieval_trace missing `queries_used`".to_string())?;
    let queries = queries
        .as_array()
        .ok_or_else(|| "retrieval_trace.queries_used must be array".to_string())?;
    validate_nonempty_string_items(queries, "external_research.retrieval_trace", "queries_used")?;
    for key in ["inclusion_rules", "exclusions", "exclusion_rationale"] {
        let field = tr
            .get(key)
            .ok_or_else(|| format!("retrieval_trace missing `{key}`"))?;
        nonempty_trimmed_string_at(field, "external_research.retrieval_trace", key)?;
    }

    Ok(())
}
