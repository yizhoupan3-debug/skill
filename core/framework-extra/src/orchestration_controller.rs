//! Background/worker orchestration policy、runtime control-plane 描述与 observability 契约。
//!
//! This file contains static data constructors with hardcoded parameters.

use core_policy::error::FrameworkError;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use fr_contracts::execution_contract::{
    EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY, EXECUTION_SCHEMA_VERSION,
    build_execution_kernel_contracts_by_mode, build_execution_kernel_metadata_contract,
    build_steady_state_execution_kernel_metadata,
};
use routing_engine::route::ROUTE_AUTHORITY;
use fr_utils::constants::{
    FRAMEWORK_RUNTIME_AUTHORITY, RUNTIME_BACKGROUND_ORCHESTRATION_SCHEMA_VERSION,
    RUNTIME_EVENT_HANDOFF_SCHEMA_VERSION, RUNTIME_EVENT_SINK_SCHEMA_VERSION,
    RUNTIME_EVENT_STREAM_SCHEMA_VERSION,
};
use rt_storage::runtime_envelope_ids::{
    BACKGROUND_CONTROL_AUTHORITY, BACKGROUND_CONTROL_SCHEMA_VERSION,
    RUNTIME_CONTROL_PLANE_AUTHORITY, RUNTIME_CONTROL_PLANE_SCHEMA_VERSION,
    RUNTIME_INTEGRATOR_AUTHORITY, RUNTIME_INTEGRATOR_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION, RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION, RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY,
};
use rt_storage::runtime_storage::{
    runtime_backend_family_catalog_payload, runtime_backend_family_parity_payload,
};
use framework_kernel::stdio_payload_types::{
    BackgroundControlEffectPlanPayload, BackgroundControlRequestPayload,
    BackgroundControlResponsePayload,
};
use framework_kernel::stdio_payload_types::runtime_concurrency_defaults_payload;

use fr_utils::json_value::required_non_empty_string;

fn background_effect_plan(next_step: &str) -> BackgroundControlEffectPlanPayload {
    BackgroundControlEffectPlanPayload {
        next_step: next_step.to_string(),
        terminal_status: None,
        resolved_status: None,
        finalize_immediately: None,
        cancel_running_task: None,
        next_retry_count: None,
        backoff_seconds: None,
        wait_timeout_seconds: None,
        wait_poll_interval_seconds: None,
        sleep_seconds: None,
    }
}
fn normalize_multitask_strategy(strategy: Option<&str>) -> String {
    strategy.unwrap_or("reject").trim().to_lowercase()
}

fn compute_backoff_seconds(
    base: f64,
    multiplier: f64,
    retry_count: usize,
    maximum: Option<f64>,
) -> f64 {
    if retry_count == 0 || base <= 0.0 {
        return 0.0;
    }
    let normalized_multiplier = if multiplier > 0.0 { multiplier } else { 1.0 };
    let mut delay = base * normalized_multiplier.powi((retry_count.saturating_sub(1)) as i32);
    if let Some(maximum) = maximum {
        delay = delay.min(maximum);
    }
    delay
}

fn compute_release_poll_interval_seconds(retry_count: Option<usize>) -> f64 {
    const SESSION_RELEASE_BASE_POLL_SECONDS: f64 = 0.02;
    const SESSION_RELEASE_BACKOFF_MULTIPLIER: f64 = 1.5;
    const SESSION_RELEASE_MAX_POLL_SECONDS: f64 = 0.25;
    let retry_step = retry_count.unwrap_or(0).saturating_add(1);
    compute_backoff_seconds(
        SESSION_RELEASE_BASE_POLL_SECONDS,
        SESSION_RELEASE_BACKOFF_MULTIPLIER,
        retry_step,
        Some(SESSION_RELEASE_MAX_POLL_SECONDS),
    )
}

fn next_background_parallel_group_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("pgroup_{nanos:x}")
}

/// 构建一个填充了所有默认 `None`/空值字段的 `BackgroundControlResponsePayload` 基础壳。
/// 各 handler 只需覆盖业务上有意义的字段。
fn base_response(
    operation: &str,
    supported_multitask_strategies: Vec<String>,
) -> BackgroundControlResponsePayload {
    BackgroundControlResponsePayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
        operation: operation.to_string(),
        resolved_parallel_group_id: None,
        lane_ids: None,
        normalized_multitask_strategy: None,
        supported_multitask_strategies,
        strategy_supported: true,
        accepted: None,
        requires_takeover: None,
        error: None,
        should_retry: None,
        next_retry_count: None,
        backoff_seconds: None,
        terminal_status: None,
        resolved_status: None,
        finalize_immediately: None,
        cancel_running_task: None,
        reason: String::new(),
        effect_plan: background_effect_plan("noop"),
    }
}

// ─── per-operation handlers ──────────────────────────────────────────

fn handle_batch_plan(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let batch_size = payload.batch_size.unwrap_or(0);
    if batch_size == 0 {
        let mut effect_plan = background_effect_plan("reject");
        effect_plan.terminal_status = Some("failed".to_string());
        let mut resp = base_response("batch-plan", supported_multitask_strategies);
        resp.strategy_supported = true;
        resp.accepted = Some(false);
        resp.requires_takeover = Some(false);
        resp.error = Some("enqueue_background_batch requires at least one request.".to_string());
        resp.terminal_status = Some("failed".to_string());
        resp.finalize_immediately = Some(true);
        resp.cancel_running_task = Some(false);
        resp.reason = "batch-plan-empty".to_string();
        resp.effect_plan = effect_plan;
        return Ok(resp);
    }

    let requested_group_id = payload
        .requested_parallel_group_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let mut request_group_ids = HashSet::new();
    if let Some(values) = payload.request_parallel_group_ids.as_ref() {
        for value in values {
            if let Some(group_id) = value
                .as_deref()
                .map(str::trim)
                .filter(|candidate| !candidate.is_empty())
            {
                request_group_ids.insert(group_id.to_string());
            }
        }
    }
    if request_group_ids.len() > 1 {
        let mut effect_plan = background_effect_plan("reject");
        effect_plan.terminal_status = Some("failed".to_string());
        let mut resp = base_response("batch-plan", supported_multitask_strategies);
        resp.accepted = Some(false);
        resp.requires_takeover = Some(false);
        resp.error = Some(
            "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
                .to_string(),
        );
        resp.terminal_status = Some("failed".to_string());
        resp.finalize_immediately = Some(true);
        resp.cancel_running_task = Some(false);
        resp.reason = "batch-plan-misaligned-parallel-group".to_string();
        resp.effect_plan = effect_plan;
        return Ok(resp);
    }
    if let Some(requested) = requested_group_id.as_ref()
        && let Some(existing) = request_group_ids.iter().next()
            && existing != requested {
                let mut effect_plan = background_effect_plan("reject");
                effect_plan.terminal_status = Some("failed".to_string());
                let mut resp = base_response("batch-plan", supported_multitask_strategies);
                resp.accepted = Some(false);
                resp.requires_takeover = Some(false);
                resp.error = Some(
                    "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
                        .to_string(),
                );
                resp.terminal_status = Some("failed".to_string());
                resp.finalize_immediately = Some(true);
                resp.cancel_running_task = Some(false);
                resp.reason = "batch-plan-misaligned-parallel-group".to_string();
                resp.effect_plan = effect_plan;
                return Ok(resp);
            }

    let resolved_parallel_group_id = requested_group_id
        .or_else(|| request_group_ids.into_iter().next())
        .unwrap_or_else(next_background_parallel_group_id);
    let lane_id_prefix = payload
        .lane_id_prefix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("lane");
    let lane_ids = (0..batch_size)
        .map(|index| {
            payload
                .request_lane_ids
                .as_ref()
                .and_then(|values| values.get(index))
                .and_then(|value| value.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .unwrap_or_else(|| format!("{lane_id_prefix}-{}", index + 1))
        })
        .collect::<Vec<_>>();
    let effect_plan = background_effect_plan("plan_batch");
    let mut resp = base_response("batch-plan", supported_multitask_strategies);
    resp.resolved_parallel_group_id = Some(resolved_parallel_group_id);
    resp.lane_ids = Some(lane_ids);
    resp.accepted = Some(true);
    resp.requires_takeover = Some(false);
    resp.finalize_immediately = Some(false);
    resp.cancel_running_task = Some(false);
    resp.reason = "batch-plan-resolved".to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_enqueue(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let normalized_multitask_strategy =
        normalize_multitask_strategy(payload.multitask_strategy.as_deref());
    let strategy_supported = supported_multitask_strategies
        .iter()
        .any(|strategy| strategy == &normalized_multitask_strategy);
    if !strategy_supported {
        let mut effect_plan = background_effect_plan("reject");
        effect_plan.terminal_status = Some("failed".to_string());
        let mut resp = base_response("enqueue", supported_multitask_strategies);
        resp.normalized_multitask_strategy = Some(normalized_multitask_strategy);
        resp.strategy_supported = false;
        resp.accepted = Some(false);
        resp.requires_takeover = Some(false);
        resp.error = Some(format!(
            "Unsupported multitask strategy: {}. Supported strategies: interrupt, reject",
            payload.multitask_strategy.as_deref().unwrap_or("reject")
        ));
        resp.reason = "invalid-multitask-strategy".to_string();
        resp.effect_plan = effect_plan;
        return Ok(resp);
    }
    let active_job_count = payload.active_job_count.unwrap_or(0);
    // When capacity_limit is not provided by the caller, treat it as unlimited
    // rather than 0, which would incorrectly reject every enqueue with "0/0 capacity".
    let capacity_limit = payload.capacity_limit.unwrap_or(usize::MAX);
    if active_job_count >= capacity_limit {
        let mut effect_plan = background_effect_plan("reject");
        effect_plan.terminal_status = Some("failed".to_string());
        let mut resp = base_response("enqueue", supported_multitask_strategies);
        resp.normalized_multitask_strategy = Some(normalized_multitask_strategy.clone());
        resp.accepted = Some(false);
        resp.requires_takeover = Some(normalized_multitask_strategy == "interrupt");
        resp.error = Some(format!(
            "Too many admitted background jobs ({}/{})",
            active_job_count, capacity_limit
        ));
        resp.reason = "capacity-rejected".to_string();
        resp.effect_plan = effect_plan;
        return Ok(resp);
    }
    let effect_plan = background_effect_plan("admit");
    let mut resp = base_response("enqueue", supported_multitask_strategies);
    resp.normalized_multitask_strategy = Some(normalized_multitask_strategy.clone());
    resp.accepted = Some(true);
    resp.requires_takeover = Some(normalized_multitask_strategy == "interrupt");
    resp.reason = "accepted".to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_interrupt(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let current_status = payload
        .current_status
        .unwrap_or_else(|| "queued".to_string());
    let task_active = payload.task_active.unwrap_or(false);
    let task_done = payload.task_done.unwrap_or(false);
    let finalize_immediately = matches!(current_status.as_str(), "queued" | "retry_scheduled")
        || !task_active
        || task_done;
    let mut effect_plan = if finalize_immediately {
        background_effect_plan("finalize_interrupted")
    } else {
        background_effect_plan("request_interrupt")
    };
    effect_plan.finalize_immediately = Some(finalize_immediately);
    effect_plan.cancel_running_task = Some(!finalize_immediately && task_active && !task_done);
    effect_plan.resolved_status = Some("interrupt_requested".to_string());
    effect_plan.terminal_status = Some(if finalize_immediately {
        "interrupted".to_string()
    } else {
        "interrupt_requested".to_string()
    });
    let terminal = if finalize_immediately {
        "interrupted"
    } else {
        "interrupt_requested"
    };
    let reason = if finalize_immediately {
        "interrupt-finalized"
    } else {
        "interrupt-cancel-running-task"
    };
    let mut resp = base_response("interrupt", supported_multitask_strategies);
    resp.terminal_status = Some(terminal.to_string());
    resp.resolved_status = Some("interrupt_requested".to_string());
    resp.finalize_immediately = Some(finalize_immediately);
    resp.cancel_running_task = Some(!finalize_immediately && task_active && !task_done);
    resp.reason = reason.to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_claim(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let current_status = payload
        .current_status
        .unwrap_or_else(|| "queued".to_string());
    if matches!(
        current_status.as_str(),
        "interrupt_requested" | "interrupted"
    ) {
        let mut effect_plan = background_effect_plan("finalize_interrupted");
        effect_plan.finalize_immediately = Some(true);
        effect_plan.terminal_status = Some("interrupted".to_string());
        effect_plan.resolved_status = Some("interrupted".to_string());
        let mut resp = base_response("claim", supported_multitask_strategies);
        resp.terminal_status = Some("interrupted".to_string());
        resp.resolved_status = Some("interrupted".to_string());
        resp.finalize_immediately = Some(true);
        resp.cancel_running_task = Some(false);
        resp.reason = "claim-suppressed-interrupted".to_string();
        resp.effect_plan = effect_plan;
        return Ok(resp);
    }
    if matches!(
        current_status.as_str(),
        "completed" | "failed" | "retry_exhausted"
    ) {
        let mut effect_plan = background_effect_plan("finalize_terminal");
        effect_plan.finalize_immediately = Some(true);
        effect_plan.terminal_status = Some(current_status.clone());
        effect_plan.resolved_status = Some(current_status.clone());
        let mut resp = base_response("claim", supported_multitask_strategies);
        resp.terminal_status = Some(current_status.clone());
        resp.resolved_status = Some(current_status);
        resp.finalize_immediately = Some(true);
        resp.cancel_running_task = Some(false);
        resp.reason = "claim-suppressed-terminal".to_string();
        resp.effect_plan = effect_plan;
        return Ok(resp);
    }
    let mut effect_plan = background_effect_plan("claim_execution");
    effect_plan.finalize_immediately = Some(false);
    effect_plan.resolved_status = Some("running".to_string());
    let mut resp = base_response("claim", supported_multitask_strategies);
    resp.resolved_status = Some("running".to_string());
    resp.finalize_immediately = Some(false);
    resp.cancel_running_task = Some(false);
    resp.reason = "claim-running".to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_complete(
    _payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let mut effect_plan = background_effect_plan("finalize_completed");
    effect_plan.finalize_immediately = Some(true);
    effect_plan.terminal_status = Some("completed".to_string());
    effect_plan.resolved_status = Some("completed".to_string());
    let mut resp = base_response("complete", supported_multitask_strategies);
    resp.terminal_status = Some("completed".to_string());
    resp.resolved_status = Some("completed".to_string());
    resp.finalize_immediately = Some(true);
    resp.cancel_running_task = Some(false);
    resp.reason = "complete-finalized".to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_completion_race(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let current_status = payload
        .current_status
        .unwrap_or_else(|| "running".to_string());
    let lost_race = matches!(
        current_status.as_str(),
        "interrupt_requested" | "interrupted"
    );
    let terminal_status = if lost_race {
        "interrupted"
    } else {
        "completed"
    };
    let mut effect_plan = if lost_race {
        background_effect_plan("finalize_interrupted")
    } else {
        background_effect_plan("finalize_completed")
    };
    effect_plan.finalize_immediately = Some(true);
    effect_plan.terminal_status = Some(terminal_status.to_string());
    effect_plan.resolved_status = Some(terminal_status.to_string());
    let reason = if lost_race {
        "completion-race-lost"
    } else {
        "completion-race-won"
    };
    let mut resp = base_response("completion-race", supported_multitask_strategies);
    resp.terminal_status = Some(terminal_status.to_string());
    resp.resolved_status = Some(terminal_status.to_string());
    resp.finalize_immediately = Some(true);
    resp.cancel_running_task = Some(false);
    resp.reason = reason.to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_retry_claim(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let current_status = payload
        .current_status
        .unwrap_or_else(|| "retry_scheduled".to_string());
    let interrupted = matches!(
        current_status.as_str(),
        "interrupt_requested" | "interrupted"
    );
    let terminal_status = if interrupted {
        "interrupted"
    } else {
        "retry_claimed"
    };
    let mut effect_plan = if interrupted {
        background_effect_plan("finalize_interrupted")
    } else {
        background_effect_plan("claim_retry")
    };
    effect_plan.finalize_immediately = Some(interrupted);
    effect_plan.terminal_status = Some(terminal_status.to_string());
    effect_plan.resolved_status = Some(terminal_status.to_string());
    let reason = if interrupted {
        "retry-claim-interrupted"
    } else {
        "retry-claim-granted"
    };
    let mut resp = base_response("retry-claim", supported_multitask_strategies);
    resp.terminal_status = Some(terminal_status.to_string());
    resp.resolved_status = Some(terminal_status.to_string());
    resp.finalize_immediately = Some(interrupted);
    resp.cancel_running_task = Some(false);
    resp.reason = reason.to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_interrupt_finalize(
    _payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let mut effect_plan = background_effect_plan("finalize_interrupted");
    effect_plan.finalize_immediately = Some(true);
    effect_plan.terminal_status = Some("interrupted".to_string());
    effect_plan.resolved_status = Some("interrupted".to_string());
    let mut resp = base_response("interrupt-finalize", supported_multitask_strategies);
    resp.terminal_status = Some("interrupted".to_string());
    resp.resolved_status = Some("interrupted".to_string());
    resp.finalize_immediately = Some(true);
    resp.cancel_running_task = Some(false);
    resp.reason = "interrupt-finalized".to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_retry(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let attempt = payload.attempt.unwrap_or(1).max(1);
    let retry_count = payload.retry_count.unwrap_or(0);
    let max_attempts = payload.max_attempts.unwrap_or(1).max(1);
    if attempt >= max_attempts {
        let mut effect_plan = background_effect_plan("finalize_terminal");
        let terminal = if max_attempts > 1 {
            "retry_exhausted"
        } else {
            "failed"
        };
        effect_plan.terminal_status = Some(terminal.to_string());
        let mut resp = base_response("retry", supported_multitask_strategies);
        resp.should_retry = Some(false);
        resp.next_retry_count = Some(retry_count);
        resp.backoff_seconds = Some(0.0);
        resp.terminal_status = Some(terminal.to_string());
        resp.reason = "attempt-budget-exhausted".to_string();
        resp.effect_plan = effect_plan;
        return Ok(resp);
    }
    let next_retry_count = retry_count + 1;
    let backoff_seconds = compute_backoff_seconds(
        payload.backoff_base_seconds.unwrap_or(0.0),
        payload.backoff_multiplier.unwrap_or(2.0),
        next_retry_count,
        payload.max_backoff_seconds,
    );
    let mut effect_plan = background_effect_plan("schedule_retry");
    effect_plan.next_retry_count = Some(next_retry_count);
    effect_plan.backoff_seconds = Some(backoff_seconds);
    effect_plan.terminal_status = Some("retry_scheduled".to_string());
    let mut resp = base_response("retry", supported_multitask_strategies);
    resp.should_retry = Some(true);
    resp.next_retry_count = Some(next_retry_count);
    resp.backoff_seconds = Some(backoff_seconds);
    resp.terminal_status = Some("retry_scheduled".to_string());
    resp.reason = "retry-scheduled".to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

fn handle_session_release(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, String> {
    let mut effect_plan = background_effect_plan("wait_for_release");
    effect_plan.wait_timeout_seconds = Some(5.0);
    effect_plan.wait_poll_interval_seconds =
        Some(compute_release_poll_interval_seconds(payload.retry_count));
    let mut resp = base_response("session-release", supported_multitask_strategies);
    resp.reason = "session-release-wait".to_string();
    resp.effect_plan = effect_plan;
    Ok(resp)
}

pub fn build_background_control_response(
    payload: BackgroundControlRequestPayload,
) -> Result<BackgroundControlResponsePayload, String> {
    let supported_multitask_strategies = vec!["interrupt".to_string(), "reject".to_string()];
    match payload.operation.as_str() {
        "batch-plan" => handle_batch_plan(payload, supported_multitask_strategies),
        "enqueue" => handle_enqueue(payload, supported_multitask_strategies),
        "interrupt" => handle_interrupt(payload, supported_multitask_strategies),
        "claim" => handle_claim(payload, supported_multitask_strategies),
        "complete" => handle_complete(payload, supported_multitask_strategies),
        "completion-race" => handle_completion_race(payload, supported_multitask_strategies),
        "retry-claim" => handle_retry_claim(payload, supported_multitask_strategies),
        "interrupt-finalize" => handle_interrupt_finalize(payload, supported_multitask_strategies),
        "retry" => handle_retry(payload, supported_multitask_strategies),
        "session-release" => handle_session_release(payload, supported_multitask_strategies),
        other => Err(format!("unsupported background control operation: {other}")),
    }
}

pub fn build_runtime_control_plane_payload() -> Value {
    let concurrency_defaults = runtime_concurrency_defaults_payload();
    let services = serde_json::json!({
        "router": {
            "authority": ROUTE_AUTHORITY,
            "role": "route-selection",
            "projection": "rust-owned-live-route",
            "delegate_kind": "rust-route-core",
        },
        "skill_loader": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "skill-registry-projection",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-runtime-control-plane",
        },
        "prompt_builder": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "prompt-contract-projection",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-execution-cli",
        },
        "middleware": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "middleware-policy-projection",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-runtime-control-plane",
            "subagent_limit_contract": {
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "owner": "rust-runtime-control-plane",
                "projection": "rust-native-projection",
                "limit_owner": "rust-control-plane",
                "max_concurrent_subagents": concurrency_defaults.max_concurrent_subagents,
                "max_concurrent_subagents_limit": concurrency_defaults.max_concurrent_subagents_limit,
                "timeout_seconds": concurrency_defaults.subagent_timeout_seconds,
                "enforcement_mode": "rust-owned-policy-native-enforced",
            },
        },
        "state": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "durable-background-state",
            "projection": "rust-native-projection",
            "delegate_kind": "filesystem-state-store",
        },
        "trace": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "trace-and-handoff",
            "projection": "rust-native-projection",
            "delegate_kind": "filesystem-trace-store",
        },
        "checkpoint": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "checkpoint-artifact-projection",
            "projection": "rust-native-projection",
            "delegate_kind": "filesystem-checkpointer",
            "backend_family_catalog": runtime_backend_family_catalog_payload(),
            "backend_family_parity": runtime_backend_family_parity_payload(
                Some("filesystem"),
                Some("filesystem"),
                Some("filesystem"),
                Some("filesystem"),
            )
            .expect("hardcoded 'filesystem' is a valid built-in backend family"),
        },
        "execution": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "execution-kernel-control",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-execution-kernel-slice",
            "kernel_contract": Value::Object(build_steady_state_execution_kernel_metadata(
                EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
            )),
            "kernel_contract_by_mode": Value::Object(build_execution_kernel_contracts_by_mode()),
            "kernel_metadata_contract": build_execution_kernel_metadata_contract(),
            "kernel_adapter_kind": "rust-execution-kernel-slice",
            "kernel_authority": "rust-execution-kernel-authority",
            "kernel_owner_family": "rust",
            "kernel_owner_impl": "execution-kernel-slice",
            "kernel_contract_mode": "rust-live-primary",
            "kernel_replace_ready": true,
            "kernel_in_process_replacement_complete": true,
            "kernel_live_backend_family": "rust-cli",
            "kernel_live_backend_impl": "router-rs",
            "kernel_live_delegate_kind": "router-rs",
            "kernel_live_delegate_authority": "rust-execution-cli",
            "kernel_live_delegate_family": "rust-cli",
            "kernel_live_delegate_impl": "router-rs",
            "kernel_live_delegate_mode": "rust-primary",
            "kernel_mode_support": ["dry_run", "live"],
            "execution_schema_version": EXECUTION_SCHEMA_VERSION,
        },
        "background": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "background-orchestration",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-background-control-policy",
            "orchestration_contract": {
                "schema_version": RUNTIME_BACKGROUND_ORCHESTRATION_SCHEMA_VERSION,
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "role": "background-orchestration-control",
                "projection": "rust-native-projection",
                "delegate_kind": "rust-background-control-policy",
                "policy_schema_version": BACKGROUND_CONTROL_SCHEMA_VERSION,
                "queue_model": "bounded-async-host",
                "session_takeover_model": "state-store-lease-arbitration",
                "state_artifact": "runtime_background_jobs.json",
                "active_statuses": [
                    "queued",
                    "running",
                    "interrupt_requested",
                    "retry_scheduled",
                    "retry_claimed"
                ],
                "terminal_statuses": [
                    "completed",
                    "failed",
                    "interrupted",
                    "retry_exhausted"
                ],
                "policy_operations": [
                    "batch-plan",
                    "enqueue",
                    "claim",
                    "interrupt",
                    "interrupt-finalize",
                    "retry",
                    "retry-claim",
                    "complete",
                    "completion-race",
                    "session-release"
                ],
                "max_background_jobs": concurrency_defaults.max_background_jobs,
                "max_background_jobs_limit": concurrency_defaults.max_background_jobs_limit,
                "background_job_timeout_seconds": concurrency_defaults.background_job_timeout_seconds,
                "admission_owner": "rust-background-control-policy",
                "queue_concurrency_owner": "rust-control-plane",
            },
        },
    });
    let rust_owned_service_count = services
        .as_object()
        .map(|service_map| service_map.len())
        .unwrap_or(0);

    serde_json::json!({
        "schema_version": RUNTIME_CONTROL_PLANE_SCHEMA_VERSION,
        "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
        "default_route_mode": "rust",
        "default_route_authority": ROUTE_AUTHORITY,
        "runtime_status": {
            "runtime_primary_owner": "rust-control-plane",
            "runtime_primary_owner_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "hot_path_projection_mode": "descriptor-driven",
            "framework_runtime_replacement": "router-rs::framework_runtime",
            "framework_runtime_replacement_authority": FRAMEWORK_RUNTIME_AUTHORITY,
        },
        "runtime_host": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "runtime-orchestration",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-runtime-control-plane",
            "startup_order": ["router", "state", "trace", "execution", "background"],
            "shutdown_order": ["background", "execution", "trace", "state", "router"],
            "health_sections": [
                "router",
                "state",
                "trace",
                "execution_environment",
                "background",
                "checkpoint"
            ],
            "rust_owned_service_count": rust_owned_service_count,
            "concurrency_contract": {
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "owner": "rust-control-plane",
                "router_stdio_pool_owner": "rust-control-plane",
                "router_stdio_pool_default_size": concurrency_defaults.router_stdio.default_pool_size,
                "router_stdio_pool_max_size": concurrency_defaults.router_stdio.max_pool_size,
                "router_stdio_pool_env_keys": concurrency_defaults.router_stdio.env_keys,
                "router_stdio_pool_scheduling": concurrency_defaults.router_stdio.scheduling,
                "router_stdio_backpressure": concurrency_defaults.router_stdio.backpressure,
                "stdio_max_concurrency_arg": concurrency_defaults.router_stdio.stdio_max_concurrency_arg,
                "request_concurrency_field": concurrency_defaults.router_stdio.request_concurrency_field,
                "compute_threads_owner": "rust-control-plane",
                "compute_threads_default": concurrency_defaults.compute.default_threads,
                "compute_threads_max": concurrency_defaults.compute.max_threads,
                "compute_threads_env_keys": concurrency_defaults.compute.env_keys,
                "compute_threads_arg": concurrency_defaults.compute.cli_arg,
                "compute_threads_scheduling": concurrency_defaults.compute.scheduling,
                "max_background_jobs": concurrency_defaults.max_background_jobs,
                "max_background_jobs_limit": concurrency_defaults.max_background_jobs_limit,
                "max_concurrent_subagents": concurrency_defaults.max_concurrent_subagents,
                "max_concurrent_subagents_limit": concurrency_defaults.max_concurrent_subagents_limit,
                "background_job_timeout_seconds": concurrency_defaults.background_job_timeout_seconds,
                "subagent_timeout_seconds": concurrency_defaults.subagent_timeout_seconds,
            },
        },
        "services": services,
    })
}

pub fn build_runtime_integrator_payload() -> Value {
    let control_plane = build_runtime_control_plane_payload();
    let runtime_host = control_plane.get("runtime_host").unwrap_or(&Value::Null);
    let services = control_plane.get("services").unwrap_or(&Value::Null);
    let runtime_status = control_plane.get("runtime_status").unwrap_or(&Value::Null);
    let concurrency_contract = runtime_host
        .get("concurrency_contract")
        .unwrap_or(&Value::Null);
    let subagent_limit_contract = services
        .get("middleware")
        .and_then(Value::as_object)
        .and_then(|middleware| middleware.get("subagent_limit_contract"))
        .unwrap_or(&Value::Null);
    let observability_exporter = build_runtime_observability_exporter_descriptor();
    let observability_metric_catalog = build_runtime_observability_metric_catalog_payload();
    let observability_dashboard = runtime_observability_dashboard_schema();
    let metric_names = observability_metric_catalog["metrics"]
        .as_array()
        .map(|metrics| {
            metrics
                .iter()
                .filter_map(|metric| metric.get("metric_name").cloned())
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default();
    let dashboard_panel_count = observability_dashboard["panels"]
        .as_array()
        .map(|items| items.len())
        .unwrap_or(0);
    let dashboard_alert_count = observability_dashboard["alerts"]
        .as_array()
        .map(|items| items.len())
        .unwrap_or(0);
    json!({
        "schema_version": RUNTIME_INTEGRATOR_SCHEMA_VERSION,
        "authority": RUNTIME_INTEGRATOR_AUTHORITY,
        "mode": "rust-owned-thin-orchestration",
        "control_plane": control_plane,
        "runtime_host": runtime_host,
        "services": services,
        "runtime_status": runtime_status,
        "concurrency_contract": concurrency_contract,
        "subagent_limit_contract": subagent_limit_contract,
        "observability": {
            "schema_version": RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
            "ownership_lane": observability_exporter["ownership_lane"],
            "metric_catalog_version": observability_exporter["metric_catalog_version"],
            "dashboard_schema_version": observability_dashboard["schema_version"],
            "resource_dimensions": observability_dashboard["resource_dimensions"],
            "metric_catalog_schema_version": observability_metric_catalog["schema_version"],
            "metric_names": metric_names,
            "dashboard_panel_count": dashboard_panel_count,
            "dashboard_alert_count": dashboard_alert_count,
            "exporter": observability_exporter,
        },
    })
}

fn runtime_observability_resource_dimensions() -> Vec<&'static str> {
    vec![
        "service.name",
        "service.version",
        "runtime.instance.id",
        "route_engine_mode",
    ]
}

fn runtime_observability_base_dimensions() -> Vec<&'static str> {
    vec![
        "runtime.job_id",
        "runtime.session_id",
        "runtime.attempt",
        "runtime.worker_id",
        "runtime.generation",
        "runtime.schema_version",
    ]
}

fn runtime_observability_dashboard_dimensions() -> Vec<String> {
    runtime_observability_resource_dimensions()
        .into_iter()
        .chain(runtime_observability_base_dimensions())
        .map(|value| value.to_string())
        .collect()
}

fn runtime_observability_metric_catalog() -> Vec<Value> {
    let base_dimensions = runtime_observability_dashboard_dimensions();
    vec![
        json!({
            "intent": "route mismatch rate",
            "metric_name": "runtime.route_mismatch_total",
            "metric_type": "counter",
            "unit": "1",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "rate(route_mismatch_total) / rate(route_evaluation_total)",
        }),
        json!({
            "intent": "replay resume success rate",
            "metric_name": "runtime.replay_resume_success_total",
            "metric_type": "counter",
            "unit": "1",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "rate(replay_resume_success_total) / rate(replay_resume_attempt_total)",
        }),
        json!({
            "intent": "lease takeover latency",
            "metric_name": "runtime.lease_takeover_latency_ms",
            "metric_type": "histogram",
            "unit": "ms",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "p50 / p95 / p99",
        }),
        json!({
            "intent": "interrupt completion latency",
            "metric_name": "runtime.interrupt_completion_latency_ms",
            "metric_type": "histogram",
            "unit": "ms",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "p50 / p95 / p99",
        }),
        json!({
            "intent": "compression offload rate",
            "metric_name": "runtime.compression_offload_total",
            "metric_type": "counter",
            "unit": "1",
            "base_dimensions": base_dimensions,
            "dashboard_derivation": "rate(compression_offload_total) / rate(compression_candidate_total)",
        }),
    ]
}

pub fn build_runtime_observability_metric_catalog_payload() -> Value {
    let metrics = runtime_observability_metric_catalog()
        .into_iter()
        .map(|metric| {
            let mut metric_object = metric;
            if let Some(base_dimensions) = metric_object.get("base_dimensions").cloned()
                && let Some(object) = metric_object.as_object_mut() {
                    object.remove("base_dimensions");
                    object.insert("dimensions".to_string(), base_dimensions);
                }
            metric_object
        })
        .collect::<Vec<Value>>();

    json!({
        "schema_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
        "metric_catalog_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
        "resource_dimensions": runtime_observability_resource_dimensions(),
        "base_dimensions": runtime_observability_base_dimensions(),
        "metrics": metrics,
    })
}

pub fn build_runtime_observability_exporter_descriptor() -> Value {
    json!({
        "schema_version": RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
        "metric_catalog_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
        "dashboard_schema_version": RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
        "signal_vocabulary": RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY,
        "export_path": "jsonl-plus-otel",
        "jsonl_sink_schema_version": RUNTIME_EVENT_SINK_SCHEMA_VERSION,
        "trace_stream_schema_version": RUNTIME_EVENT_STREAM_SCHEMA_VERSION,
        "trace_handoff_schema_version": RUNTIME_EVENT_HANDOFF_SCHEMA_VERSION,
        "ownership_lane": "rust-contract-lane",
        "producer_owner": "rust-control-plane",
        "producer_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
        "exporter_owner": "rust-control-plane",
        "exporter_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
    })
}

pub fn build_runtime_observability_health_snapshot() -> Value {
    let exporter = build_runtime_observability_exporter_descriptor();
    let dashboard = runtime_observability_dashboard_schema();
    let catalog = build_runtime_observability_metric_catalog_payload();
    let metric_names = catalog
        .get("metrics")
        .and_then(Value::as_array)
        .map(|metrics| {
            metrics
                .iter()
                .filter_map(|metric| metric.get("metric_name").cloned())
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default();
    let dashboard_panel_count = dashboard
        .get("panels")
        .and_then(Value::as_array)
        .map(|panels| panels.len())
        .unwrap_or(0);
    let dashboard_alert_count = dashboard
        .get("alerts")
        .and_then(Value::as_array)
        .map(|alerts| alerts.len())
        .unwrap_or(0);

    json!({
        "schema_version": RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
        "ownership_lane": exporter["ownership_lane"].clone(),
        "metric_catalog_version": exporter["metric_catalog_version"].clone(),
        "dashboard_schema_version": dashboard["schema_version"].clone(),
        "resource_dimensions": dashboard["resource_dimensions"].clone(),
        "metric_catalog_schema_version": catalog["schema_version"].clone(),
        "metric_names": metric_names,
        "dashboard_panel_count": dashboard_panel_count,
        "dashboard_alert_count": dashboard_alert_count,
        "exporter": exporter,
    })
}

pub fn runtime_observability_dashboard_schema() -> Value {
    json!({
        "schema_version": RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
        "title": "Runtime Observability",
        "resource_dimensions": runtime_observability_dashboard_dimensions(),
        "panels": [
            {
                "name": "Route mismatch rate",
                "metric": "runtime.route_mismatch_total",
                "visualization": "timeseries",
                "group_by": ["service.name", "service.version", "route_engine_mode"],
            },
            {
                "name": "Replay resume success rate",
                "metric": "runtime.replay_resume_success_total",
                "visualization": "timeseries",
                "group_by": ["service.name", "service.version", "runtime.session_id"],
            },
            {
                "name": "Lease takeover latency",
                "metric": "runtime.lease_takeover_latency_ms",
                "visualization": "histogram",
                "group_by": ["service.name", "service.version", "runtime.worker_id"],
            },
            {
                "name": "Interrupt completion latency",
                "metric": "runtime.interrupt_completion_latency_ms",
                "visualization": "histogram",
                "group_by": ["service.name", "service.version", "runtime.session_id"],
            },
            {
                "name": "Compression offload rate",
                "metric": "runtime.compression_offload_total",
                "visualization": "timeseries",
                "group_by": ["service.name", "service.version", "runtime.generation"],
            },
        ],
        "alerts": [
            {
                "name": "route-mismatch-burst",
                "metric": "runtime.route_mismatch_total",
                "severity": "warning",
            },
            {
                "name": "lease-takeover-latency-regression",
                "metric": "runtime.lease_takeover_latency_ms",
                "severity": "critical",
            },
        ],
    })
}

pub fn build_runtime_metric_record(payload: Value) -> Result<Value, FrameworkError> {
    let metric_name = required_non_empty_string(&payload, "metric_name", "runtime metric record")?;
    let spec = runtime_observability_metric_catalog()
        .into_iter()
        .find(|entry| {
            entry.get("metric_name").and_then(Value::as_str) == Some(metric_name.as_str())
        })
        .ok_or_else(|| FrameworkError::unsupported(format!("unsupported runtime metric: {metric_name}")))?;

    let value = payload
        .get("value")
        .cloned()
        .ok_or_else(|| FrameworkError::validation("runtime metric record requires a numeric value".to_string()))?;
    let numeric_value = value
        .as_f64()
        .ok_or_else(|| FrameworkError::validation("runtime metric record requires a numeric value".to_string()))?;
    if !numeric_value.is_finite() {
        return Err(FrameworkError::validation("metric value must be finite".to_string()));
    }

    let service_name =
        required_non_empty_string(&payload, "service_name", "runtime metric record")?;
    let service_version =
        required_non_empty_string(&payload, "service_version", "runtime metric record")?;
    let runtime_instance_id =
        required_non_empty_string(&payload, "runtime_instance_id", "runtime metric record")?;
    let route_engine_mode =
        required_non_empty_string(&payload, "route_engine_mode", "runtime metric record")?;
    let job_id = required_non_empty_string(&payload, "job_id", "runtime metric record")?;
    let session_id = required_non_empty_string(&payload, "session_id", "runtime metric record")?;
    let worker_id = required_non_empty_string(&payload, "worker_id", "runtime metric record")?;
    let generation = required_non_empty_string(&payload, "generation", "runtime metric record")?;
    let attempt = payload
        .get("attempt")
        .and_then(Value::as_i64)
        .ok_or_else(|| FrameworkError::validation("runtime metric record requires integer field attempt".to_string()))?;
    if attempt < 0 {
        return Err(
            FrameworkError::validation("runtime metric record requires non-negative integer field attempt".to_string()),
        );
    }

    Ok(json!({
        "schema_version": RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION,
        "metric_name": metric_name,
        "metric_type": spec.get("metric_type").cloned().unwrap_or(Value::Null),
        "unit": spec.get("unit").cloned().unwrap_or(Value::Null),
        "value": value,
        "resource_attributes": {
            "service.name": service_name,
            "service.version": service_version,
            "runtime.instance.id": runtime_instance_id,
            "route_engine_mode": route_engine_mode,
        },
        "dimensions": {
            "runtime.job_id": job_id,
            "runtime.session_id": session_id,
            "runtime.attempt": attempt,
            "runtime.worker_id": worker_id,
            "runtime.generation": generation,
            "runtime.schema_version": RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION,
            "runtime.stage": "runtime.metric",
            "runtime.status": "ok",
        },
        "ownership": build_runtime_observability_exporter_descriptor(),
    }))
}
