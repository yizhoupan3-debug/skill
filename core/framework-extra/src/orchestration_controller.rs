//! Background/worker orchestration policy、runtime control-plane 描述与 observability 契约。
//!
//! This file contains static data constructors with hardcoded parameters.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use core_errors::FrameworkError;
use serde_json::{Value, json};

use framework_runtime::constants::{
    RUNTIME_EVENT_HANDOFF_SCHEMA_VERSION, RUNTIME_EVENT_SINK_SCHEMA_VERSION,
    RUNTIME_EVENT_STREAM_SCHEMA_VERSION,
};
use framework_core::stdio_payload_types::{
    BackgroundControlEffectPlanPayload, BackgroundControlRequestPayload,
    BackgroundControlResponsePayload,
};
use rt_storage::runtime_envelope_ids::{
    BACKGROUND_CONTROL_AUTHORITY, BACKGROUND_CONTROL_SCHEMA_VERSION,
    RUNTIME_CONTROL_PLANE_AUTHORITY, RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION, RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY,
};

fn background_effect_plan(next_step: &str) -> BackgroundControlEffectPlanPayload {
    BackgroundControlEffectPlanPayload::new(next_step.to_string())
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
    let mut delay =
        base * normalized_multiplier.powi((retry_count.min(32).saturating_sub(1)) as i32);
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
    let mut response = BackgroundControlResponsePayload::new(operation, "", background_effect_plan("noop"));
    response.schema_version = BACKGROUND_CONTROL_SCHEMA_VERSION.to_string();
    response.authority = BACKGROUND_CONTROL_AUTHORITY.to_string();
    response.supported_multitask_strategies = supported_multitask_strategies;
    response.strategy_supported = true;
    response
}

// ─── per-operation handlers ──────────────────────────────────────────

fn handle_batch_plan(
    payload: BackgroundControlRequestPayload,
    supported_multitask_strategies: Vec<String>,
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
        && existing != requested
    {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
) -> Result<BackgroundControlResponsePayload, FrameworkError> {
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
        other => Err(FrameworkError::unsupported(format!(
            "unsupported background control operation: {other}"
        ))),
    }
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
    ]
}

pub fn build_runtime_observability_metric_catalog_payload() -> Value {
    let metrics = runtime_observability_metric_catalog()
        .into_iter()
        .map(|metric| {
            let mut metric_object = metric;
            if let Some(base_dimensions) = metric_object.get("base_dimensions").cloned()
                && let Some(object) = metric_object.as_object_mut()
            {
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

/// Dispatch an orchestrator operation payload.
///
/// Routes known sub-operations to the appropriate handler. For operations that
/// require the session-supervisor crate (team/worker/agent management), returns
/// a documented error since that crate is not wired into this build.
///
/// This is wired into `runtime_core::init_hooks()` as the
/// `handle_orchestrator_operation` hook. It replaces the previous dead-end
/// closure that unconditionally returned an error.
pub fn handle_orchestrator_operation(payload: Value) -> Result<Value, FrameworkError> {
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("<missing>")
        .to_string();
    tracing::info!(operation = %operation, "orchestrator operation requested");

    match operation.as_str() {
        // Background control operations — route to existing handler
        "batch-plan" | "enqueue" | "interrupt" | "claim" | "complete" | "completion-race"
        | "retry-claim" | "interrupt-finalize" | "retry" | "session-release" => {
            let request: BackgroundControlRequestPayload = serde_json::from_value(payload)
                .map_err(|e| {
                    FrameworkError::validation(format!("background_control payload: {e}"))
                })?;
            let response = build_background_control_response(request)?;
            serde_json::to_value(response).map_err(|e| {
                FrameworkError::validation(format!("background_control response: {e}"))
            })
        }
        other => Err(FrameworkError::hook(format!(
            "orchestrator operation '{other}' is not available: the session-supervisor crate (team/worker/agent \
                     management) is not wired into this runtime-core build. \
                     Available operations: team_create, team_add_member, team_remove_member, team_complete, \
                     team_send_message, team_read_messages, team_alive_members, team_list, agent_register, \
                     agent_unregister, agent_list_running, launch, list, terminate, classify_block. \
                     Background control operations (batch-plan, enqueue, interrupt, claim, complete, \
                     completion-race, retry-claim, interrupt-finalize, retry, session-release) ARE available \
                     via the appropriate orchestrator operation payload."
        ))),
    }
}
