//! Framework prompt compression (`build_framework_prompt_compression_envelope`).

use serde_json::{json, Value};

use super::alias;
use super::constants::{
    FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY, FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
};
use super::value_text;

pub fn build_framework_prompt_compression_envelope(payload: Value) -> Result<Value, String> {
    let prompt = value_text(payload.get("prompt").or_else(|| payload.get("text")));
    let token_budget = payload
        .get("token_budget")
        .or_else(|| payload.get("budget"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            "framework prompt compression requires token_budget or budget".to_string()
        })?;
    let result = compress_prompt_with_rust_policy(&prompt, token_budget);
    Ok(json!({
        "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
        "authority": FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY,
        "compression": result,
    }))
}

fn compress_prompt_with_rust_policy(prompt: &str, token_budget: usize) -> Value {
    let input_token_estimate = alias::estimate_token_count(prompt);
    if token_budget == 0 {
        let output = "[omitted: token budget is zero]".to_string();
        return compression_payload(
            input_token_estimate,
            alias::estimate_token_count(&output),
            &output,
            "zero_budget",
            true,
            &["all".to_string()],
        );
    }
    if input_token_estimate <= token_budget {
        return compression_payload(
            input_token_estimate,
            input_token_estimate,
            prompt,
            "unchanged",
            false,
            &[],
        );
    }

    let lines = prompt.lines().collect::<Vec<_>>();
    let target_chars = token_budget.saturating_mul(4).max(1);
    let (output, strategy, omitted_sections) = if lines.len() >= 6 {
        let head = lines
            .iter()
            .take(3)
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>();
        let tail = lines
            .iter()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|line| (*line).to_string())
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
    let bounded_output = enforce_prompt_budget(output, token_budget);
    compression_payload(
        input_token_estimate,
        alias::estimate_token_count(&bounded_output),
        &bounded_output,
        &strategy,
        true,
        &omitted_sections,
    )
}

fn enforce_prompt_budget(output: String, token_budget: usize) -> String {
    let max_chars = token_budget.saturating_mul(4).max(1);
    if output.chars().count() <= max_chars {
        return output;
    }
    let marker = "\n[truncated tail]";
    if max_chars <= marker.chars().count() {
        return "[truncated]".chars().take(max_chars).collect();
    }
    let keep = max_chars - marker.chars().count();
    format!(
        "{}{}",
        output.chars().take(keep).collect::<String>(),
        marker
    )
}

fn compression_payload(
    input_token_estimate: usize,
    output_token_estimate: usize,
    output: &str,
    strategy: &str,
    truncated: bool,
    omitted_sections: &[String],
) -> Value {
    json!({
        "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
        "policy_owner": "rust",
        "prompt_policy_owner": "rust",
        "input_token_estimate": input_token_estimate,
        "output_token_estimate": output_token_estimate,
        "output": output,
        "compressed_prompt": output,
        "omitted_sections": omitted_sections,
        "strategy": strategy,
        "truncated": truncated,
        "artifact_offload_decision": false,
    })
}
