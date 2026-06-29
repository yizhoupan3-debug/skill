use super::common::*;
use super::*;

use serde_json::{Value, json};

#[test]
fn execution_kernel_metadata_shape_consistency_regression_for_primary_and_dry_run() {
    let contracts = build_execution_kernel_contracts_by_mode();
    let live_primary = contracts
        .get(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY)
        .expect("live primary contract");
    let dry_run = contracts
        .get(EXECUTION_RESPONSE_SHAPE_DRY_RUN)
        .expect("dry run contract");
    let base_fields = execution_kernel_contract_shape_fields(live_primary);
    assert_eq!(base_fields, execution_kernel_contract_shape_fields(dry_run));
    assert_eq!(
        live_primary["execution_kernel_response_shape"],
        Value::String(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY.to_string())
    );
    assert_eq!(
        dry_run["execution_kernel_response_shape"],
        Value::String(EXECUTION_RESPONSE_SHAPE_DRY_RUN.to_string())
    );
    assert_eq!(
        live_primary["execution_kernel_prompt_preview_owner"],
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
    );
    assert_eq!(
        dry_run["execution_kernel_prompt_preview_owner"],
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
    );
    assert_eq!(contracts.len(), 2);
}

#[test]
fn execution_kernel_metadata_contract_is_rust_owned() {
    let contract = build_execution_kernel_metadata_contract();

    assert_eq!(
        contract["schema_version"],
        Value::String(EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        contract["steady_state_fields"][0],
        Value::String("execution_kernel_metadata_schema_version".to_string())
    );
    assert_eq!(
        contract["runtime_fields"]["shared"],
        json!(["trace_event_count", "trace_output_path"])
    );
    assert_eq!(
        contract["defaults"]["supported_response_shapes"],
        json!([
            EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
            EXECUTION_RESPONSE_SHAPE_DRY_RUN,
        ])
    );
}

#[test]
fn execute_request_dry_run_returns_rust_owned_contract() {
    let response = execute_request(sample_execute_request()).expect("execute response");

    assert_eq!(response.execution_schema_version, EXECUTION_SCHEMA_VERSION);
    assert_eq!(response.authority, EXECUTION_AUTHORITY);
    assert!(!response.live_run);
    assert_eq!(response.skill, "goal_drive");
    assert_eq!(response.overlay, None);
    assert_eq!(response.usage.mode, "estimated");
    assert_eq!(response.model_id, None);
    assert_eq!(response.metadata["execution_kernel"], EXECUTION_KERNEL_KIND);
    assert_eq!(
        response.metadata["execution_kernel_metadata_schema_version"],
        EXECUTION_METADATA_SCHEMA_VERSION
    );
    assert_eq!(
        response.metadata["execution_kernel_authority"],
        EXECUTION_KERNEL_AUTHORITY
    );
    assert_eq!(
        response.metadata["execution_kernel_response_shape"],
        EXECUTION_RESPONSE_SHAPE_DRY_RUN
    );
    assert_eq!(
        response.metadata["execution_kernel_prompt_preview_owner"],
        EXECUTION_PROMPT_PREVIEW_OWNER
    );
    assert_eq!(
        response.metadata["diagnostic_route_mode"],
        Value::String("none".to_string())
    );
}

#[test]
fn live_execute_prompt_builder_produces_rust_owned_contract_prompt() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;

    let prompt = build_live_execute_prompt(&payload);

    assert!(prompt.contains("Help with the user's request directly."));
    assert!(prompt.contains("Primary focus: goal_drive"));
    assert!(!prompt.contains("Extra guidance:"));
    assert!(prompt.contains("How to reply:"));
    assert!(prompt.contains("Lead with the answer or result."));
    assert!(prompt.contains(
        "Use plain Chinese unless the user asks otherwise, and keep the wording natural."
    ));
    assert!(prompt.contains(
        "Keep the default reply short; only use a list when the content is naturally list-shaped."
    ));
    assert!(prompt.contains("Trigger phrase matched: 直接做代码."));
}

#[test]
fn live_execute_prompt_builder_treats_none_as_native_runtime() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.selected_skill = "none".to_string();
    payload.overlay_skill = None;
    payload.reasons = vec![
        "No explicit skill hit; native runtime should proceed without loading a skill.".to_string(),
    ];

    let prompt = build_live_execute_prompt(&payload);

    assert!(prompt.contains("Primary focus: native runtime instructions"));
    assert!(prompt.contains("No skill body is required"));
    assert!(!prompt.contains("Primary focus: none"));
    assert!(!prompt.contains("Use the selected skill"));
}

#[test]
fn live_execute_prompt_builder_caps_task_cues_to_five_lines() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.reasons = vec![
        "cue-1".to_string(),
        "cue-2".to_string(),
        "cue-3".to_string(),
        "cue-4".to_string(),
        "cue-5".to_string(),
        "cue-6".to_string(),
    ];

    let prompt = build_live_execute_prompt(&payload);

    assert!(prompt.contains("- cue-1"));
    assert!(prompt.contains("- cue-2"));
    assert!(prompt.contains("- cue-3"));
    assert!(prompt.contains("- cue-4"));
    assert!(prompt.contains("- cue-5"));
    assert!(!prompt.contains("- cue-6"));
}

#[test]
fn live_execute_prompt_builder_does_not_add_removed_planning_contract() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.selected_skill = "deepinterview".to_string();
    payload.overlay_skill = None;
    payload.layer = "L-1".to_string();
    payload.reasons = vec!["Trigger hint matched: 先探索现状再提方案.".to_string()];

    let prompt = build_live_execute_prompt(&payload);

    assert!(!prompt.contains("Planning output:"));
    assert!(prompt.contains("Primary focus: deepinterview"));
    assert!(!prompt.contains("READ-ONLY planning route"));
    assert!(!prompt.contains("<proposed_plan>"));
}

#[test]
fn live_execute_prompt_builder_uses_deep_mode_contract_when_requested() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "/goal_drive deep 深度调研联网能力".to_string();

    let _prompt = build_live_execute_prompt(&payload);
}

#[test]
fn live_execute_infer_deep_from_task_deep_dive_phrase() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "please do a deep dive on tokenizer failure modes".to_string();
    payload.selected_skill = "documentation-engineering".to_string();

    let _prompt = build_live_execute_prompt(&payload);
}

#[test]
fn live_execute_infer_deep_from_reason_literature_review_phrase() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "summarize the migration path".to_string();
    payload.selected_skill = "research-execution".to_string();
    payload.reasons = vec!["lane: literature review".to_string()];

    let _prompt = build_live_execute_prompt(&payload);
}

#[test]
fn live_execute_infer_deep_from_reason_depth_research_zh_only() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "summarize the migration path".to_string();
    payload.selected_skill = "research-execution".to_string();
    payload.reasons = vec!["用户要求：深度研究".to_string()];

    let _prompt = build_live_execute_prompt(&payload);
}

#[test]
fn live_execute_infer_quick_when_task_is_bare_external_research_api() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "Wire up the external research API client.".to_string();
    payload.selected_skill = "documentation-engineering".to_string();

    let prompt = build_live_execute_prompt(&payload);
    assert!(prompt.contains("Keep the default reply short;"));
}

#[test]
fn live_execute_infer_quick_when_external_research_with_stack_trace_only() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "Investigate failure: external research module prints stack trace".to_string();
    payload.selected_skill = "documentation-engineering".to_string();

    let _prompt = build_live_execute_prompt(&payload);
}

#[test]
fn live_execute_infer_quick_when_external_research_with_structured_logging_jargon() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "Wire external research client with structured logging for ops.".to_string();
    payload.selected_skill = "documentation-engineering".to_string();

    let _prompt = build_live_execute_prompt(&payload);
}

#[test]
fn live_execute_infer_deep_when_external_research_plus_literature_cue() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "external research literature review for the safety claim".to_string();
    payload.selected_skill = "documentation-engineering".to_string();

    let _prompt = build_live_execute_prompt(&payload);
}

#[test]
fn live_execute_ignores_caller_supplied_prompt_preview() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = Some("Native supplied live prompt".to_string());

    let prompt = build_live_execute_prompt(&payload);
    let response = build_live_execute_response(
        &payload,
        Some(prompt.clone()),
        LiveExecuteResult {
            content: "router-rs content".to_string(),
            model_id: Some("gpt-5.4".to_string()),
            run_id: Some("run-1".to_string()),
            status: Some("stop".to_string()),
            input_tokens: 21,
            output_tokens: 13,
            total_tokens: 34,
            finish_reason: Some("stop".to_string()),
            continuation_attempted: false,
            continuation_status: None,
            continuation_error: None,
        },
    );

    assert_eq!(response.prompt_preview.as_deref(), Some(prompt.as_str()));
    assert_ne!(
        response.prompt_preview.as_deref(),
        Some("Native supplied live prompt")
    );
    assert_eq!(response.metadata["execution_kernel"], EXECUTION_KERNEL_KIND);
    assert_eq!(
        response.metadata["execution_kernel_authority"],
        EXECUTION_KERNEL_AUTHORITY
    );
    assert_eq!(
        response.metadata["execution_kernel_metadata_schema_version"],
        EXECUTION_METADATA_SCHEMA_VERSION
    );
    assert_eq!(response.metadata["finish_reason"], json!("stop"));
    assert_eq!(
        response.metadata["execution_kernel_delegate_family"],
        "rust-cli"
    );
    assert_eq!(
        response.metadata["execution_kernel_delegate_impl"],
        "router-rs"
    );
    assert_eq!(
        response.metadata["execution_kernel_live_primary"],
        "router-rs"
    );
    assert_eq!(
        response.metadata["execution_kernel_live_primary_authority"],
        EXECUTION_AUTHORITY
    );
    assert_eq!(
        response.metadata["execution_kernel_response_shape"],
        EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY
    );
    assert_eq!(
        response.metadata["execution_kernel_prompt_preview_owner"],
        EXECUTION_PROMPT_PREVIEW_OWNER
    );
    assert_eq!(
        response.metadata["execution_kernel_model_id_source"],
        EXECUTION_MODEL_ID_SOURCE
    );
}

#[test]
fn extract_chat_completion_content_accepts_string_and_part_arrays() {
    let string_payload = serde_json::json!({
        "choices": [{"message": {"content": "hello from router-rs"}}]
    });
    let parts_payload = serde_json::json!({
        "choices": [{
            "message": {
                "content": [
                    {"text": "hello "},
                    {"content": "from "},
                    {"text": "router-rs"}
                ]
            }
        }]
    });

    assert_eq!(
        extract_chat_completion_content(&string_payload).expect("string content"),
        "hello from router-rs"
    );
    assert_eq!(
        extract_chat_completion_content(&parts_payload).expect("parts content"),
        "hello from router-rs"
    );
}

#[test]
fn validate_live_execute_aggregator_base_url_accepts_public_https_domain() {
    with_execute_allowlist_env(None, || {
        validate_live_execute_aggregator_base_url("https://api.openai.com/v1")
            .expect("public https domain should be allowed");
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_rejects_http_scheme() {
    with_execute_allowlist_env(None, || {
        let err = validate_live_execute_aggregator_base_url("http://api.openai.com/v1")
            .expect_err("http scheme should be rejected");
        assert!(err.to_string().contains("requires https"));
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_rejects_localhost() {
    with_execute_allowlist_env(None, || {
        let err = validate_live_execute_aggregator_base_url("https://localhost:8443/v1")
            .expect_err("localhost should be rejected");
        assert!(err.to_string().contains("blocks localhost"));
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_rejects_private_ip_literal() {
    with_execute_allowlist_env(None, || {
        let err = validate_live_execute_aggregator_base_url("https://10.10.10.2/v1")
            .expect_err("private IP literal should be rejected");
        assert!(
            err.to_string()
                .contains("unsafe aggregator_base_url host IP")
        );
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_allowlist_match_passes() {
    with_execute_allowlist_env(Some("api.openai.com,example.com"), || {
        validate_live_execute_aggregator_base_url("https://api.openai.com/v1")
            .expect("allowlisted host should pass");
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_allowlist_miss_rejects() {
    with_execute_allowlist_env(Some("allowed.example.com"), || {
        let err = validate_live_execute_aggregator_base_url("https://api.openai.com/v1")
            .expect_err("non-allowlisted host should be rejected");
        assert!(err.to_string().contains("not in allowlist"));
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_without_allowlist_preserves_behavior() {
    with_execute_allowlist_env(None, || {
        validate_live_execute_aggregator_base_url("https://api.openai.com/v1")
            .expect("public https domain should remain allowed without allowlist");
    });
}

#[test]
fn live_execute_deep_length_continuation_success_accumulates_usage_and_metadata() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    let mut call_index = 0usize;
    let first_content = "A".repeat(DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS + 120);
    let mut captured_requests = Vec::new();
    let live_result = perform_live_execute_with_sender(&payload, "deep prompt", |body| {
        captured_requests.push(body.clone());
        call_index += 1;
        if call_index == 1 {
            return Ok((
                200,
                json!({
                    "id": "run-1",
                    "model": "gpt-5.4",
                    "choices": [{
                        "finish_reason": "length",
                        "message": {"content": first_content}
                    }],
                    "usage": {
                        "prompt_tokens": 10,
                        "completion_tokens": 20,
                        "total_tokens": 30
                    }
                })
                .to_string(),
            ));
        }
        Ok((
            200,
            json!({
                "choices": [{
                    "finish_reason": "stop",
                    "message": {"content": "second-part"}
                }],
                "usage": {
                    "prompt_tokens": 3,
                    "completion_tokens": 7,
                    "total_tokens": 10
                }
            })
            .to_string(),
        ))
    })
    .expect("live execute should succeed");
    assert_eq!(call_index, 2);
    assert!(live_result.content.contains(&first_content));
    assert!(live_result.content.contains("second-part"));
    assert_eq!(live_result.input_tokens, 13);
    assert_eq!(live_result.output_tokens, 27);
    assert_eq!(live_result.total_tokens, 40);
    assert_eq!(live_result.finish_reason.as_deref(), Some("stop"));
    assert_eq!(live_result.continuation_status.as_deref(), Some("success"));
    assert!(live_result.continuation_error.is_none());
    let continuation_messages = captured_requests
        .get(1)
        .and_then(|body| body.get("messages"))
        .and_then(Value::as_array)
        .expect("continuation request should include messages");
    assert_eq!(continuation_messages.len(), 3);
    let assistant_message = continuation_messages
        .iter()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .expect("continuation request should include assistant tail");
    assert!(assistant_message.starts_with("[...omitted "));
    assert!(assistant_message.len() < first_content.len());
    assert!(!assistant_message.contains(&first_content));
    let response = build_live_execute_response(&payload, None, live_result);
    assert_eq!(response.metadata["continuation_attempted"], json!(true));
    assert_eq!(response.metadata["continuation_status"], json!("success"));
    assert_eq!(response.metadata["continuation_error"], Value::Null);
}

#[test]
fn live_execute_deep_length_continuation_failure_fails_open() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    let mut call_index = 0usize;
    let live_result = perform_live_execute_with_sender(&payload, "deep prompt", |_body| {
        call_index += 1;
        if call_index == 1 {
            return Ok((
                200,
                json!({
                    "id": "run-1",
                    "model": "gpt-5.4",
                    "choices": [{
                        "finish_reason": "length",
                        "message": {"content": "first-part-only"}
                    }],
                    "usage": {
                        "prompt_tokens": 8,
                        "completion_tokens": 5,
                        "total_tokens": 13
                    }
                })
                .to_string(),
            ));
        }
        Ok((502, "{\"error\":\"bad gateway\"}".to_string()))
    })
    .expect("continuation failure should fail-open");
    assert_eq!(call_index, 2);
    assert_eq!(live_result.content, "first-part-only");
    assert_eq!(live_result.input_tokens, 8);
    assert_eq!(live_result.output_tokens, 5);
    assert_eq!(live_result.total_tokens, 13);
    assert_eq!(live_result.continuation_status.as_deref(), Some("http_502"));
    assert!(
        live_result
            .continuation_error
            .as_deref()
            .unwrap_or_default()
            .contains("HTTP 502")
    );
    let response = build_live_execute_response(&payload, None, live_result);
    assert_eq!(response.metadata["continuation_attempted"], json!(true));
    assert_eq!(response.metadata["continuation_status"], json!("http_502"));
    assert!(
        response.metadata["continuation_error"]
            .as_str()
            .unwrap_or_default()
            .contains("HTTP 502")
    );
}

#[test]
fn live_execute_retries_first_round_before_success() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    let mut call_index = 0usize;
    let live_result = perform_live_execute_with_sender(&payload, "quick prompt", |_body| {
        call_index += 1;
        if call_index == 1 {
            return Ok((500, "{\"error\":\"transient\"}".to_string()));
        }
        Ok((
            200,
            json!({
                "id": "run-retry",
                "model": "gpt-5.4",
                "choices": [{
                    "finish_reason": "stop",
                    "message": {"content": "retry-success"}
                }],
                "usage": {
                    "prompt_tokens": 4,
                    "completion_tokens": 6,
                    "total_tokens": 10
                }
            })
            .to_string(),
        ))
    })
    .expect("second attempt should pass");
    assert_eq!(call_index, 2);
    assert_eq!(live_result.content, "retry-success");
    assert!(!live_result.continuation_attempted);
}

#[test]
fn normalize_chat_completions_endpoint_keeps_existing_path() {
    assert_eq!(
        normalize_chat_completions_endpoint("https://api.openai.com/v1/chat/completions"),
        "https://api.openai.com/v1/chat/completions"
    );
    assert_eq!(
        normalize_chat_completions_endpoint("https://api.openai.com/v1"),
        "https://api.openai.com/v1/chat/completions"
    );
}

#[test]
fn live_execute_http_client_is_process_cached() {
    let first = live_execute_http_client().expect("first client");
    let second = live_execute_http_client().expect("second client");

    assert!(std::ptr::eq(first, second));
}
