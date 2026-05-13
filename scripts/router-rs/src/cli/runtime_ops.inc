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

pub(crate) const EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV: &str =
    "ROUTER_RS_EXECUTE_AGGREGATOR_HOST_ALLOWLIST";

fn normalize_allowlisted_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn parse_execute_aggregator_host_allowlist() -> Result<Option<HashSet<String>>, String> {
    let raw_value = match std::env::var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!(
                "router-rs live execute allowlist env {EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV} is not valid UTF-8"
            ));
        }
    };

    let hosts = raw_value
        .split(',')
        .map(normalize_allowlisted_host)
        .filter(|entry| !entry.is_empty())
        .collect::<HashSet<_>>();

    if hosts.is_empty() {
        return Err(format!(
            "router-rs live execute allowlist env {EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV} is configured but empty"
        ));
    }

    for entry in &hosts {
        if entry.eq_ignore_ascii_case("localhost") {
            return Err(format!(
                "router-rs live execute allowlist env {EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV} only accepts domain entries (localhost is forbidden)"
            ));
        }
        if entry.parse::<IpAddr>().is_ok() {
            return Err(format!(
                "router-rs live execute allowlist env {EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV} only accepts domain entries (IP literals are forbidden): {entry}"
            ));
        }
    }

    Ok(Some(hosts))
}

pub(crate) fn execute_request(payload: ExecuteRequestPayload) -> Result<ExecuteResponsePayload, String> {
    let prompt_preview = payload
        .prompt_preview
        .clone()
        .filter(|value| !value.trim().is_empty());
    if payload.dry_run {
        let dry_run_prompt_preview =
            Some(prompt_preview.unwrap_or_else(|| build_live_execute_prompt(&payload)));
        return Ok(build_dry_run_execute_response(
            &payload,
            dry_run_prompt_preview,
        ));
    }
    let live_prompt_preview = build_live_execute_prompt(&payload);
    if payload.aggregator_base_url.trim().is_empty() {
        return Err("router-rs execute requires a non-empty aggregator_base_url".to_string());
    }
    if payload.aggregator_api_key.trim().is_empty() {
        return Err("router-rs execute requires a non-empty aggregator_api_key".to_string());
    }
    let live_result = perform_live_execute(&payload, &live_prompt_preview)?;
    Ok(build_live_execute_response(
        &payload,
        Some(live_prompt_preview),
        live_result,
    ))
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

pub(crate) fn build_background_control_response(
    payload: BackgroundControlRequestPayload,
) -> Result<BackgroundControlResponsePayload, String> {
    let supported_multitask_strategies = vec!["interrupt".to_string(), "reject".to_string()];
    match payload.operation.as_str() {
        "batch-plan" => {
            let batch_size = payload.batch_size.unwrap_or(0);
            if batch_size == 0 {
                let mut effect_plan = background_effect_plan("reject");
                effect_plan.terminal_status = Some("failed".to_string());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: None,
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: Some(false),
                    requires_takeover: Some(false),
                    error: Some(
                        "enqueue_background_batch requires at least one request.".to_string(),
                    ),
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: Some("failed".to_string()),
                    resolved_status: None,
                    finalize_immediately: Some(true),
                    cancel_running_task: Some(false),
                    reason: "batch-plan-empty".to_string(),
                    effect_plan,
                });
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
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: None,
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: Some(false),
                    requires_takeover: Some(false),
                    error: Some(
                        "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
                            .to_string(),
                    ),
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: Some("failed".to_string()),
                    resolved_status: None,
                    finalize_immediately: Some(true),
                    cancel_running_task: Some(false),
                    reason: "batch-plan-misaligned-parallel-group".to_string(),
                    effect_plan,
                });
            }
            if let Some(requested) = requested_group_id.as_ref() {
                if let Some(existing) = request_group_ids.iter().next() {
                    if existing != requested {
                        let mut effect_plan = background_effect_plan("reject");
                        effect_plan.terminal_status = Some("failed".to_string());
                        return Ok(BackgroundControlResponsePayload {
                            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                            authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                            operation: payload.operation,
                            resolved_parallel_group_id: None,
                            lane_ids: None,
                            normalized_multitask_strategy: None,
                            supported_multitask_strategies,
                            strategy_supported: true,
                            accepted: Some(false),
                            requires_takeover: Some(false),
                            error: Some(
                                "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
                                    .to_string(),
                            ),
                            should_retry: None,
                            next_retry_count: None,
                            backoff_seconds: None,
                            terminal_status: Some("failed".to_string()),
                            resolved_status: None,
                            finalize_immediately: Some(true),
                            cancel_running_task: Some(false),
                            reason: "batch-plan-misaligned-parallel-group".to_string(),
                            effect_plan,
                        });
                    }
                }
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
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: Some(resolved_parallel_group_id),
                lane_ids: Some(lane_ids),
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: Some(true),
                requires_takeover: Some(false),
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: None,
                resolved_status: None,
                finalize_immediately: Some(false),
                cancel_running_task: Some(false),
                reason: "batch-plan-resolved".to_string(),
                effect_plan,
            })
        }
        "enqueue" => {
            let normalized_multitask_strategy =
                normalize_multitask_strategy(payload.multitask_strategy.as_deref());
            let strategy_supported = supported_multitask_strategies
                .iter()
                .any(|strategy| strategy == &normalized_multitask_strategy);
            if !strategy_supported {
                let mut effect_plan = background_effect_plan("reject");
                effect_plan.terminal_status = Some("failed".to_string());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: Some(normalized_multitask_strategy),
                    supported_multitask_strategies,
                    strategy_supported: false,
                    accepted: Some(false),
                    requires_takeover: Some(false),
                    error: Some(format!(
                        "Unsupported multitask strategy: {}. Supported strategies: interrupt, reject",
                        payload.multitask_strategy.as_deref().unwrap_or("reject")
                    )),
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: None,
                    resolved_status: None,
                    finalize_immediately: None,
                    cancel_running_task: None,
                    reason: "invalid-multitask-strategy".to_string(),
                    effect_plan,
                });
            }
            let active_job_count = payload.active_job_count.unwrap_or(0);
            let capacity_limit = payload.capacity_limit.unwrap_or(0);
            if active_job_count >= capacity_limit {
                let mut effect_plan = background_effect_plan("reject");
                effect_plan.terminal_status = Some("failed".to_string());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: Some(normalized_multitask_strategy.clone()),
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: Some(false),
                    requires_takeover: Some(normalized_multitask_strategy == "interrupt"),
                    error: Some(format!(
                        "Too many admitted background jobs ({}/{})",
                        active_job_count, capacity_limit
                    )),
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: None,
                    resolved_status: None,
                    finalize_immediately: None,
                    cancel_running_task: None,
                    reason: "capacity-rejected".to_string(),
                    effect_plan,
                });
            }
            let effect_plan = background_effect_plan("admit");
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: Some(normalized_multitask_strategy.clone()),
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: Some(true),
                requires_takeover: Some(normalized_multitask_strategy == "interrupt"),
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: None,
                resolved_status: None,
                finalize_immediately: None,
                cancel_running_task: None,
                reason: "accepted".to_string(),
                effect_plan,
            })
        }
        "interrupt" => {
            let current_status = payload
                .current_status
                .unwrap_or_else(|| "queued".to_string());
            let task_active = payload.task_active.unwrap_or(false);
            let task_done = payload.task_done.unwrap_or(false);
            let finalize_immediately =
                matches!(current_status.as_str(), "queued" | "retry_scheduled")
                    || !task_active
                    || task_done;
            let mut effect_plan = if finalize_immediately {
                background_effect_plan("finalize_interrupted")
            } else {
                background_effect_plan("request_interrupt")
            };
            effect_plan.finalize_immediately = Some(finalize_immediately);
            effect_plan.cancel_running_task =
                Some(!finalize_immediately && task_active && !task_done);
            effect_plan.resolved_status = Some("interrupt_requested".to_string());
            effect_plan.terminal_status = Some(if finalize_immediately {
                "interrupted".to_string()
            } else {
                "interrupt_requested".to_string()
            });
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
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
                terminal_status: Some(if finalize_immediately {
                    "interrupted".to_string()
                } else {
                    "interrupt_requested".to_string()
                }),
                resolved_status: Some("interrupt_requested".to_string()),
                finalize_immediately: Some(finalize_immediately),
                cancel_running_task: Some(!finalize_immediately && task_active && !task_done),
                reason: if finalize_immediately {
                    "interrupt-finalized".to_string()
                } else {
                    "interrupt-cancel-running-task".to_string()
                },
                effect_plan,
            })
        }
        "claim" => {
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
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
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
                    terminal_status: Some("interrupted".to_string()),
                    resolved_status: Some("interrupted".to_string()),
                    finalize_immediately: Some(true),
                    cancel_running_task: Some(false),
                    reason: "claim-suppressed-interrupted".to_string(),
                    effect_plan,
                });
            }
            if matches!(
                current_status.as_str(),
                "completed" | "failed" | "retry_exhausted"
            ) {
                let mut effect_plan = background_effect_plan("finalize_terminal");
                effect_plan.finalize_immediately = Some(true);
                effect_plan.terminal_status = Some(current_status.clone());
                effect_plan.resolved_status = Some(current_status.clone());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
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
                    terminal_status: Some(current_status.clone()),
                    resolved_status: Some(current_status),
                    finalize_immediately: Some(true),
                    cancel_running_task: Some(false),
                    reason: "claim-suppressed-terminal".to_string(),
                    effect_plan,
                });
            }
            let mut effect_plan = background_effect_plan("claim_execution");
            effect_plan.finalize_immediately = Some(false);
            effect_plan.resolved_status = Some("running".to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
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
                resolved_status: Some("running".to_string()),
                finalize_immediately: Some(false),
                cancel_running_task: Some(false),
                reason: "claim-running".to_string(),
                effect_plan,
            })
        }
        "complete" => {
            let mut effect_plan = background_effect_plan("finalize_completed");
            effect_plan.finalize_immediately = Some(true);
            effect_plan.terminal_status = Some("completed".to_string());
            effect_plan.resolved_status = Some("completed".to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
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
                terminal_status: Some("completed".to_string()),
                resolved_status: Some("completed".to_string()),
                finalize_immediately: Some(true),
                cancel_running_task: Some(false),
                reason: "complete-finalized".to_string(),
                effect_plan,
            })
        }
        "completion-race" => {
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
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
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
                terminal_status: Some(terminal_status.to_string()),
                resolved_status: Some(terminal_status.to_string()),
                finalize_immediately: Some(true),
                cancel_running_task: Some(false),
                reason: if lost_race {
                    "completion-race-lost".to_string()
                } else {
                    "completion-race-won".to_string()
                },
                effect_plan,
            })
        }
        "retry-claim" => {
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
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
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
                terminal_status: Some(terminal_status.to_string()),
                resolved_status: Some(terminal_status.to_string()),
                finalize_immediately: Some(interrupted),
                cancel_running_task: Some(false),
                reason: if interrupted {
                    "retry-claim-interrupted".to_string()
                } else {
                    "retry-claim-granted".to_string()
                },
                effect_plan,
            })
        }
        "interrupt-finalize" => {
            let mut effect_plan = background_effect_plan("finalize_interrupted");
            effect_plan.finalize_immediately = Some(true);
            effect_plan.terminal_status = Some("interrupted".to_string());
            effect_plan.resolved_status = Some("interrupted".to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
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
                terminal_status: Some("interrupted".to_string()),
                resolved_status: Some("interrupted".to_string()),
                finalize_immediately: Some(true),
                cancel_running_task: Some(false),
                reason: "interrupt-finalized".to_string(),
                effect_plan,
            })
        }
        "retry" => {
            let attempt = payload.attempt.unwrap_or(1).max(1);
            let retry_count = payload.retry_count.unwrap_or(0);
            let max_attempts = payload.max_attempts.unwrap_or(1).max(1);
            if attempt >= max_attempts {
                let mut effect_plan = background_effect_plan("finalize_terminal");
                effect_plan.terminal_status = Some(if max_attempts > 1 {
                    "retry_exhausted".to_string()
                } else {
                    "failed".to_string()
                });
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: None,
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: None,
                    requires_takeover: None,
                    error: None,
                    should_retry: Some(false),
                    next_retry_count: Some(retry_count),
                    backoff_seconds: Some(0.0),
                    terminal_status: Some(if max_attempts > 1 {
                        "retry_exhausted".to_string()
                    } else {
                        "failed".to_string()
                    }),
                    resolved_status: None,
                    finalize_immediately: None,
                    cancel_running_task: None,
                    reason: "attempt-budget-exhausted".to_string(),
                    effect_plan,
                });
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
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: Some(true),
                next_retry_count: Some(next_retry_count),
                backoff_seconds: Some(backoff_seconds),
                terminal_status: Some("retry_scheduled".to_string()),
                resolved_status: None,
                finalize_immediately: None,
                cancel_running_task: None,
                reason: "retry-scheduled".to_string(),
                effect_plan,
            })
        }
        "session-release" => {
            let mut effect_plan = background_effect_plan("wait_for_release");
            effect_plan.wait_timeout_seconds = Some(5.0);
            effect_plan.wait_poll_interval_seconds =
                Some(compute_release_poll_interval_seconds(payload.retry_count));
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
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
                reason: "session-release-wait".to_string(),
                effect_plan,
            })
        }
        other => Err(format!("unsupported background control operation: {other}")),
    }
}

fn sandbox_transition_allowed(current_state: &str, next_state: &str) -> bool {
    matches!(
        (current_state, next_state),
        ("created", "warm")
            | ("warm", "busy")
            | ("busy", "draining")
            | ("draining", "recycled")
            | ("draining", "failed")
            | ("warm", "failed")
            | ("busy", "failed")
            | ("recycled", "warm")
    )
}

#[allow(clippy::too_many_arguments)]
fn sandbox_response(
    request: &SandboxControlRequestPayload,
    current_state: Option<String>,
    next_state: Option<String>,
    allowed: bool,
    resolved_state: Option<String>,
    reason: &str,
    error: Option<String>,
    failure_reason: Option<String>,
    budget_violation: Option<String>,
    cleanup_required: Option<bool>,
    quarantined: Option<bool>,
    effective_capabilities: Option<Vec<String>>,
    event_kind: Option<&str>,
) -> SandboxControlResponsePayload {
    SandboxControlResponsePayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        authority: SANDBOX_CONTROL_AUTHORITY.to_string(),
        operation: request.operation.clone(),
        current_state,
        next_state,
        allowed,
        resolved_state,
        reason: reason.to_string(),
        error,
        failure_reason,
        budget_violation,
        cleanup_required,
        quarantined,
        effective_capabilities,
        sandbox_id: request.sandbox_id.clone(),
        profile_id: request.profile_id.clone(),
        event_schema_version: Some(SANDBOX_EVENT_SCHEMA_VERSION.to_string()),
        event_log_path: request.event_log_path.clone(),
        event_written: false,
        event_kind: event_kind.map(|value| value.to_string()),
    }
}

fn maybe_record_sandbox_event(
    response: &mut SandboxControlResponsePayload,
    request: &SandboxControlRequestPayload,
) -> Result<(), String> {
    if request.trace_event != Some(true) {
        return Ok(());
    }
    let path = request
        .event_log_path
        .as_deref()
        .ok_or_else(|| "sandbox event tracing requires event_log_path".to_string())?;
    let event = json!({
        "schema_version": SANDBOX_EVENT_SCHEMA_VERSION,
        "authority": SANDBOX_CONTROL_AUTHORITY,
        "ts": Utc::now().to_rfc3339(),
        "kind": response.event_kind,
        "operation": response.operation,
        "sandbox_id": response.sandbox_id,
        "profile_id": response.profile_id,
        "current_state": response.current_state,
        "next_state": response.next_state,
        "resolved_state": response.resolved_state,
        "allowed": response.allowed,
        "reason": response.reason,
        "failure_reason": response.failure_reason,
        "budget_violation": response.budget_violation,
        "cleanup_required": response.cleanup_required,
        "quarantined": response.quarantined,
        "effective_capabilities": response.effective_capabilities,
    });
    let serialized = serde_json::to_string(&event)
        .map_err(|err| format!("serialize sandbox event failed: {err}"))?
        + "\n";
    let path = PathBuf::from(path);
    append_text_with_process_lock(&path, &serialized, "sandbox event log")?;
    response.event_log_path = Some(path.display().to_string());
    response.event_written = true;
    Ok(())
}

pub(crate) fn build_sandbox_control_response(
    payload: SandboxControlRequestPayload,
) -> Result<SandboxControlResponsePayload, String> {
    let mut response = match payload.operation.as_str() {
        "transition" => {
            let current_state = payload
                .current_state
                .clone()
                .ok_or_else(|| "sandbox control transition requires current_state".to_string())?;
            let next_state = payload
                .next_state
                .clone()
                .ok_or_else(|| "sandbox control transition requires next_state".to_string())?;
            let allowed = sandbox_transition_allowed(&current_state, &next_state);
            sandbox_response(
                &payload,
                Some(current_state.clone()),
                Some(next_state.clone()),
                allowed,
                Some(next_state.clone()),
                if allowed {
                    "transition-accepted"
                } else {
                    "invalid-transition"
                },
                if allowed {
                    None
                } else {
                    Some(format!(
                        "invalid sandbox transition: {:?} -> {:?}",
                        current_state, next_state
                    ))
                },
                None,
                None,
                None,
                None,
                payload.capability_categories.clone(),
                None,
            )
        }
        "cleanup" => {
            let current_state = payload
                .current_state
                .clone()
                .unwrap_or_else(|| "draining".to_string());
            let cleanup_failed = payload.cleanup_failed.unwrap_or(false);
            let resolved_state = if cleanup_failed { "failed" } else { "recycled" };
            let allowed = matches!(current_state.as_str(), "draining");
            sandbox_response(
                &payload,
                Some(current_state.clone()),
                Some(resolved_state.to_string()),
                allowed,
                Some(resolved_state.to_string()),
                if !allowed {
                    "cleanup-invalid-state"
                } else if cleanup_failed {
                    "cleanup-failed"
                } else {
                    "cleanup-completed"
                },
                if allowed {
                    None
                } else {
                    Some(format!(
                        "invalid sandbox cleanup state: {:?} -> {:?}",
                        current_state, resolved_state
                    ))
                },
                payload
                    .error_kind
                    .clone()
                    .or_else(|| cleanup_failed.then(|| "cleanup_failed".to_string())),
                None,
                Some(false),
                Some(cleanup_failed),
                payload.capability_categories.clone(),
                Some(if cleanup_failed {
                    "sandbox.cleanup_failed"
                } else {
                    "sandbox.cleanup_completed"
                }),
            )
        }
        "admit" => {
            let current_state = payload
                .current_state
                .clone()
                .unwrap_or_else(|| "warm".to_string());
            let categories = payload.capability_categories.clone().unwrap_or_default();
            let tool_category = payload
                .tool_category
                .clone()
                .unwrap_or_else(|| "workspace_mutating".to_string());
            let dedicated_profile = payload.dedicated_profile.unwrap_or(false);
            let failure_reason = if categories.is_empty() {
                Some("policy_violation:missing_capability_declaration".to_string())
            } else if let Some(unknown) = categories.iter().find(|category| {
                !matches!(
                    category.as_str(),
                    "read_only" | "workspace_mutating" | "networked" | "high_risk"
                )
            }) {
                Some(format!("policy_violation:unknown_capability:{unknown}"))
            } else if !matches!(
                tool_category.as_str(),
                "read_only" | "workspace_mutating" | "networked" | "high_risk"
            ) {
                Some(format!(
                    "policy_violation:unknown_tool_category:{tool_category}"
                ))
            } else if !categories.iter().any(|category| category == &tool_category) {
                Some(format!(
                    "policy_violation:capability_denied:{tool_category}"
                ))
            } else if tool_category == "high_risk" && !dedicated_profile {
                Some("policy_violation:high_risk_requires_dedicated_profile".to_string())
            } else if payload.budget_cpu.unwrap_or(0.0) <= 0.0 {
                Some("budget_admission_failed:cpu_non_positive".to_string())
            } else if payload.budget_memory.unwrap_or(0) <= 0 {
                Some("budget_admission_failed:memory_non_positive".to_string())
            } else if payload.budget_wall_clock.unwrap_or(0.0) <= 0.0 {
                Some("budget_admission_failed:wall_clock_non_positive".to_string())
            } else if payload.budget_output_size.unwrap_or(0) <= 0 {
                Some("budget_admission_failed:output_size_non_positive".to_string())
            } else {
                None
            };
            if let Some(reason) = failure_reason.or_else(|| {
                (!sandbox_transition_allowed(&current_state, "busy")).then(|| {
                    format!("invalid sandbox admission state: {current_state:?} -> \"busy\"")
                })
            }) {
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some("failed".to_string()),
                    false,
                    Some("failed".to_string()),
                    "admission-rejected",
                    Some(reason.clone()),
                    Some(reason),
                    None,
                    Some(false),
                    Some(true),
                    Some(categories.clone()),
                    Some("sandbox.failed"),
                )
            } else {
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some("busy".to_string()),
                    true,
                    Some("busy".to_string()),
                    "admission-accepted",
                    None,
                    None,
                    None,
                    Some(false),
                    Some(false),
                    Some(categories.clone()),
                    Some("sandbox.execution_started"),
                )
            }
        }
        "execution_result" => {
            let current_state = payload
                .current_state
                .clone()
                .unwrap_or_else(|| "busy".to_string());
            let budget_violation = [
                (
                    "cpu_exceeded",
                    payload
                        .probe_cpu
                        .zip(payload.budget_cpu)
                        .is_some_and(|(observed, limit)| observed > limit),
                ),
                (
                    "memory_exceeded",
                    payload
                        .probe_memory
                        .zip(payload.budget_memory)
                        .is_some_and(|(observed, limit)| observed > limit),
                ),
                (
                    "wall_clock_exceeded",
                    payload
                        .probe_wall_clock
                        .zip(payload.budget_wall_clock)
                        .is_some_and(|(observed, limit)| observed > limit),
                ),
                (
                    "output_size_exceeded",
                    payload
                        .probe_output_size
                        .zip(payload.budget_output_size)
                        .is_some_and(|(observed, limit)| observed > limit),
                ),
            ]
            .into_iter()
            .find_map(|(reason, exceeded)| exceeded.then(|| reason.to_string()));
            if let Some(reason) = payload.error_kind.clone() {
                let resolved_state = if reason == "wall_clock_exceeded" {
                    "draining"
                } else {
                    "failed"
                };
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some(resolved_state.to_string()),
                    sandbox_transition_allowed(&current_state, resolved_state),
                    Some(resolved_state.to_string()),
                    if resolved_state == "draining" {
                        "execution-timeout"
                    } else {
                        "execution-failed"
                    },
                    Some(reason.clone()),
                    Some(reason),
                    None,
                    Some(resolved_state == "draining"),
                    Some(resolved_state == "failed"),
                    payload.capability_categories.clone(),
                    Some(if resolved_state == "draining" {
                        "sandbox.timeout"
                    } else {
                        "sandbox.failed"
                    }),
                )
            } else if let Some(violation) = budget_violation {
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some("draining".to_string()),
                    sandbox_transition_allowed(&current_state, "draining"),
                    Some("draining".to_string()),
                    "budget-exceeded",
                    Some(violation.clone()),
                    Some(violation.clone()),
                    Some(violation),
                    Some(true),
                    Some(false),
                    payload.capability_categories.clone(),
                    Some("sandbox.budget_exceeded"),
                )
            } else {
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some("draining".to_string()),
                    sandbox_transition_allowed(&current_state, "draining"),
                    Some("draining".to_string()),
                    "execution-completed",
                    None,
                    None,
                    None,
                    Some(true),
                    Some(false),
                    payload.capability_categories.clone(),
                    Some("sandbox.execution_completed"),
                )
            }
        }
        other => return Err(format!("unsupported sandbox control operation: {other}")),
    };
    maybe_record_sandbox_event(&mut response, &payload)?;
    Ok(response)
}

pub(crate) fn build_live_execute_prompt(payload: &ExecuteRequestPayload) -> String {
    let research_mode = infer_research_mode(payload);
    let native_runtime = payload.selected_skill == "none";
    let mut lines = vec![
        "Help with the user's request directly. The route is already chosen, so stay on it."
            .to_string(),
    ];
    if native_runtime {
        lines.push(
            "Primary focus: native runtime instructions; no skill body was selected.".to_string(),
        );
    } else {
        lines.push(format!("Primary focus: {}", payload.selected_skill));
    }
    if let Some(overlay) = payload
        .overlay_skill
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("Extra guidance: {overlay}"));
    }
    lines.push("How to reply:".to_string());
    lines.push("- Lead with the answer or result.".to_string());
    lines.push(
        "- Use plain Chinese unless the user asks otherwise, and keep the wording natural."
            .to_string(),
    );
    if research_mode == ResearchMode::Quick {
        lines.push("- Keep the default reply short; only use a list when the content is naturally list-shaped.".to_string());
    } else {
        lines.push("- Use a deep-research structure with explicit sections: Key findings, Evidence, Counter-evidence, Confidence, Open risks.".to_string());
    }
    lines.push("- For closeouts, say what was done, what effect was achieved, and what needs to happen next or that the work is finished.".to_string());
    if research_mode == ResearchMode::Quick {
        lines.push("- Do not default to file inventories, evidence dumps, or step-by-step process retellings unless the user asks for them.".to_string());
    } else {
        lines.push("- For each major claim, include at least two independent evidence anchors and one uncertainty note when evidence is incomplete.".to_string());
        lines.push("- If verification_required or evidence_required is true, treat missing evidence as an explicit blocker instead of silently concluding.".to_string());
        lines.push("- Auditable multi-round external research belongs in ledger `RFV_LOOP_STATE.json` via stdio op `framework_rfv_loop`; read `docs/rfv_loop_harness.md` / `docs/references/rfv-loop/external-research-harness.md`; hooks never auto-create that ledger.".to_string());
    }
    let prompt_reasons = payload
        .reasons
        .iter()
        .map(|reason| reason.trim())
        .filter(|reason| !reason.is_empty())
        .take(5)
        .collect::<Vec<_>>();
    if !prompt_reasons.is_empty() {
        lines.push("Task cues:".to_string());
        for reason in prompt_reasons {
            lines.push(format!("- {reason}"));
        }
    }
    if native_runtime {
        lines.push(
            "No skill body is required; solve the user's actual task with the native runtime instructions already in context."
                .to_string(),
        );
    } else {
        lines.push("Use the selected skill to solve the user's actual task.".to_string());
    }
    lines.push(format!("Execution mode: {}.", research_mode.as_str()));
    lines.join("\n")
}

fn build_dry_run_execute_response(
    payload: &ExecuteRequestPayload,
    prompt_preview: Option<String>,
) -> ExecuteResponsePayload {
    let prompt = prompt_preview.clone().unwrap_or_default();
    let input_tokens = estimate_tokens(&format!("{}\n{}", payload.task, prompt));
    let output_tokens = payload.default_output_tokens.min(96);
    let content = format!(
        "[dry-run] Routed to `{}` on {}. Session `{}` is ready for Rust-owned execution.",
        payload.selected_skill, payload.layer, payload.session_id
    );
    let mut metadata =
        build_steady_state_execution_kernel_metadata(EXECUTION_RESPONSE_SHAPE_DRY_RUN);
    metadata.insert(
        "reason".to_string(),
        Value::String("router-rs returned a deterministic dry-run payload.".to_string()),
    );
    metadata.insert(
        "trace_event_count".to_string(),
        json!(payload.trace_event_count),
    );
    metadata.insert(
        "trace_output_path".to_string(),
        json!(payload.trace_output_path),
    );
    metadata.insert(
        "execution_mode".to_string(),
        Value::String("dry_run".to_string()),
    );
    metadata.insert("route_engine".to_string(), json!(payload.route_engine));
    metadata.insert(
        "diagnostic_route_mode".to_string(),
        json!(payload.diagnostic_route_mode),
    );
    ExecuteResponsePayload {
        execution_schema_version: EXECUTION_SCHEMA_VERSION.to_string(),
        authority: EXECUTION_AUTHORITY.to_string(),
        session_id: payload.session_id.clone(),
        user_id: payload.user_id.clone(),
        skill: payload.selected_skill.clone(),
        overlay: payload.overlay_skill.clone(),
        live_run: false,
        content,
        usage: ExecuteUsagePayload {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
            mode: "estimated".to_string(),
        },
        prompt_preview,
        model_id: None,
        metadata: Value::Object(metadata),
    }
}

#[derive(Debug)]
pub struct LiveExecuteResult {
    pub content: String,
    pub model_id: Option<String>,
    pub run_id: Option<String>,
    pub status: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub finish_reason: Option<String>,
    pub continuation_attempted: bool,
    pub continuation_status: Option<String>,
    pub continuation_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResearchMode {
    Quick,
    Deep,
}

impl ResearchMode {
    fn as_str(self) -> &'static str {
        match self {
            ResearchMode::Quick => "quick",
            ResearchMode::Deep => "deep",
        }
    }
}

/// `external research` alone matches many integration/API strings; require a second research cue.
fn external_research_phrase_signals_deep(lower: &str) -> bool {
    if !lower.contains("external research") {
        return false;
    }
    lower.contains("调研")
        || lower.contains("文献")
        || lower.contains("审计")
        || lower.contains("ledger")
        || lower.contains("rfv")
        || lower.contains("外研")
        || lower.contains("literature")
        || lower.contains("unknowns")
        || lower.contains("contradiction")
        || lower.contains("auditable")
        || lower.contains("research-grade")
        || lower.contains("research grade")
        || lower.contains("科研级")
        || lower.contains("deep dive")
}

/// Narrow host-neutral cues for Execute deep shaping (substring match; ASCII segments may be lowercased).
pub(crate) fn payload_text_signals_deep_research(text: &str) -> bool {
    text.contains("/autopilot-deep")
        || text.contains("/autopilot deep")
        || text.contains("深度调研")
        || text.contains("深度研究")
        || text.contains("deep research")
        || text.contains("deep dive")
        || text.contains("literature review")
        || text.contains("literature-review")
        || text.contains("文献调研")
        || external_research_phrase_signals_deep(text)
        || text.contains("research-grade")
        || text.contains("research grade")
        || text.contains("科研级调研")
}

fn normalize_research_mode_token(value: &str) -> Option<ResearchMode> {
    let lowered = value.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return None;
    }
    match lowered.as_str() {
        "quick" | "fast" | "lite" | "shallow" | "autopilot-quick" => Some(ResearchMode::Quick),
        "deep" | "deep_research" | "deep-research" | "autopilot-deep" => Some(ResearchMode::Deep),
        _ => None,
    }
}

fn infer_research_mode(payload: &ExecuteRequestPayload) -> ResearchMode {
    if let Some(mode) = payload
        .research_mode
        .as_deref()
        .and_then(normalize_research_mode_token)
    {
        return mode;
    }
    if let Some(mode) = payload
        .execution_protocol
        .as_deref()
        .and_then(normalize_research_mode_token)
    {
        return mode;
    }
    let task = payload.task.trim().to_ascii_lowercase();
    if payload_text_signals_deep_research(&task) {
        return ResearchMode::Deep;
    }
    if task.contains("/autopilot-quick")
        || task.contains("/autopilot quick")
        || task.contains("快查")
        || task.contains("快速调研")
    {
        return ResearchMode::Quick;
    }
    for reason in &payload.reasons {
        if let Some(mode) = normalize_research_mode_token(reason) {
            return mode;
        }
        let lowered = reason.to_ascii_lowercase();
        if payload_text_signals_deep_research(&lowered) {
            return ResearchMode::Deep;
        }
    }
    if payload.selected_skill == "autopilot" {
        return ResearchMode::Quick;
    }
    ResearchMode::Quick
}

fn usage_total(usage: Option<&serde_json::Map<String, Value>>, key: &str) -> u64 {
    usage
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn merge_usage_totals(
    base: Option<&serde_json::Map<String, Value>>,
    extra: Option<&serde_json::Map<String, Value>>,
) -> Option<serde_json::Map<String, Value>> {
    if base.is_none() && extra.is_none() {
        return None;
    }
    let mut merged = base.cloned().unwrap_or_default();
    merged.insert(
        "prompt_tokens".to_string(),
        json!(usage_total(base, "prompt_tokens") + usage_total(extra, "prompt_tokens")),
    );
    merged.insert(
        "completion_tokens".to_string(),
        json!(usage_total(base, "completion_tokens") + usage_total(extra, "completion_tokens")),
    );
    merged.insert(
        "total_tokens".to_string(),
        json!(usage_total(base, "total_tokens") + usage_total(extra, "total_tokens")),
    );
    Some(merged)
}

pub(crate) const DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS: usize = 1600;
const DEEP_CONTINUATION_ANCHOR_CHARS: usize = 200;

fn build_compact_anchor(raw: &str, max_chars: usize) -> String {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return "(empty)".to_string();
    }
    let count = compact.chars().count();
    if count <= max_chars {
        compact
    } else {
        format!(
            "{}...",
            compact
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn build_assistant_tail_window(raw: &str, max_chars: usize) -> String {
    let total = raw.chars().count();
    if total <= max_chars {
        return raw.to_string();
    }
    let omitted = total.saturating_sub(max_chars);
    let tail = raw.chars().skip(omitted).collect::<String>();
    format!("[...omitted {omitted} chars...]\n{tail}")
}

pub(crate) fn perform_live_execute_with_sender<F>(
    payload: &ExecuteRequestPayload,
    prompt_preview: &str,
    mut send_request: F,
) -> Result<LiveExecuteResult, String>
where
    F: FnMut(&Value) -> Result<(u16, String), String>,
{
    let mut messages = Vec::new();
    if !prompt_preview.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": prompt_preview,
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": payload.task,
    }));
    let research_mode = infer_research_mode(payload);
    let mut max_tokens = payload.default_output_tokens;
    if research_mode == ResearchMode::Deep {
        max_tokens = max_tokens.max(1200);
    }
    let request_body = serde_json::json!({
        "model": payload.model_id,
        "messages": messages,
        "max_tokens": max_tokens,
    });
    let mut response_payload = Value::Null;
    let mut last_error = "router-rs live execute request failed".to_string();
    for attempt in 0..=1usize {
        match send_request(&request_body) {
            Ok((status_code, response_body)) => {
                if !(200..300).contains(&status_code) {
                    last_error = format!(
                        "router-rs live execute returned HTTP {}: {}",
                        status_code,
                        truncate_for_error(&response_body)
                    );
                    if attempt == 0 {
                        continue;
                    }
                    return Err(last_error);
                }
                response_payload =
                    serde_json::from_str::<Value>(&response_body).map_err(|err| {
                        format!("parse router-rs live execute response failed: {err}")
                    })?;
                break;
            }
            Err(err) => {
                last_error = format!("router-rs live execute request failed: {err}");
                if attempt == 0 {
                    continue;
                }
                return Err(last_error);
            }
        }
    }
    if response_payload.is_null() {
        return Err(last_error);
    }
    let mut content = extract_chat_completion_content(&response_payload)?;
    let first_usage = response_payload
        .get("usage")
        .and_then(Value::as_object)
        .cloned();
    let mut usage = first_usage.clone();
    let mut finish_reason = response_payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let mut continuation_attempted = false;
    let mut continuation_status = None;
    let mut continuation_error = None;
    if research_mode == ResearchMode::Deep && finish_reason.as_deref() == Some("length") {
        continuation_attempted = true;
        let system_anchor = build_compact_anchor(prompt_preview, DEEP_CONTINUATION_ANCHOR_CHARS);
        let task_anchor = build_compact_anchor(&payload.task, DEEP_CONTINUATION_ANCHOR_CHARS);
        let assistant_tail =
            build_assistant_tail_window(&content, DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS);
        let continuation_messages = vec![
            serde_json::json!({
                "role": "system",
                "content": format!(
                    "Deep continuation. Keep the same objective and style. System anchor: {system_anchor}. Task anchor: {task_anchor}."
                )
            }),
            serde_json::json!({"role": "assistant", "content": assistant_tail}),
            serde_json::json!({"role": "user", "content": "Continue exactly from the cutoff. Do not repeat prior text. Prioritize unresolved evidence gaps and open risks."}),
        ];
        let continuation_body = serde_json::json!({
            "model": payload.model_id,
            "messages": continuation_messages,
            "max_tokens": max_tokens,
        });
        match send_request(&continuation_body) {
            Ok((status_code, continuation_text)) => {
                if !(200..300).contains(&status_code) {
                    continuation_status = Some(format!("http_{status_code}"));
                    continuation_error = Some(format!(
                        "router-rs continuation returned HTTP {}: {}",
                        status_code,
                        truncate_for_error(&continuation_text)
                    ));
                } else {
                    match serde_json::from_str::<Value>(&continuation_text) {
                        Ok(continuation_payload) => {
                            match extract_chat_completion_content(&continuation_payload) {
                                Ok(continuation_content) => {
                                    if !continuation_content.trim().is_empty() {
                                        content = format!(
                                            "{}\n\n{}",
                                            content.trim_end(),
                                            continuation_content.trim_start()
                                        );
                                    }
                                    usage = merge_usage_totals(
                                        first_usage.as_ref(),
                                        continuation_payload
                                            .get("usage")
                                            .and_then(Value::as_object),
                                    );
                                    finish_reason = continuation_payload
                                        .get("choices")
                                        .and_then(Value::as_array)
                                        .and_then(|choices| choices.first())
                                        .and_then(|choice| choice.get("finish_reason"))
                                        .and_then(Value::as_str)
                                        .map(|value| value.to_string())
                                        .or(finish_reason);
                                    continuation_status = Some("success".to_string());
                                }
                                Err(err) => {
                                    continuation_status = Some("content_error".to_string());
                                    continuation_error = Some(format!(
                                        "router-rs continuation content extraction failed: {err}"
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            continuation_status = Some("parse_error".to_string());
                            continuation_error = Some(format!(
                                "parse router-rs continuation response failed: {err}"
                            ));
                        }
                    }
                }
            }
            Err(err) => {
                continuation_status = Some("request_error".to_string());
                continuation_error = Some(format!(
                    "router-rs live execute continuation request failed: {err}"
                ));
            }
        }
    }
    let input_tokens = usage
        .as_ref()
        .and_then(|usage| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(&content) as u64) as usize;
    let output_tokens = usage
        .as_ref()
        .and_then(|usage| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(&content) as u64) as usize;
    let total_tokens = usage
        .as_ref()
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or((input_tokens + output_tokens) as u64) as usize;
    Ok(LiveExecuteResult {
        content,
        model_id: response_payload
            .get("model")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        run_id: response_payload
            .get("id")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        status: finish_reason.clone(),
        input_tokens,
        output_tokens,
        total_tokens,
        finish_reason,
        continuation_attempted,
        continuation_status,
        continuation_error,
    })
}

pub(crate) fn perform_live_execute(
    payload: &ExecuteRequestPayload,
    prompt_preview: &str,
) -> Result<LiveExecuteResult, String> {
    validate_live_execute_aggregator_base_url(&payload.aggregator_base_url)?;
    let endpoint = normalize_chat_completions_endpoint(&payload.aggregator_base_url);
    let client = live_execute_http_client()?;
    perform_live_execute_with_sender(payload, prompt_preview, |request_body| {
        let response = client
            .post(endpoint.clone())
            .bearer_auth(payload.aggregator_api_key.as_str())
            .json(request_body)
            .send()
            .map_err(|err| format!("router-rs live execute request failed: {err}"))?;
        let status = response.status().as_u16();
        let response_body = response
            .text()
            .map_err(|err| format!("read router-rs live execute response failed: {err}"))?;
        Ok((status, response_body))
    })
}

pub(crate) fn live_execute_http_client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("build reqwest client failed: {err}"))?;
    let _ = CLIENT.set(client);
    CLIENT
        .get()
        .ok_or_else(|| "build reqwest client failed: client cache was not initialized".to_string())
}

pub(crate) fn build_live_execute_response(
    payload: &ExecuteRequestPayload,
    prompt_preview: Option<String>,
    live_result: LiveExecuteResult,
) -> ExecuteResponsePayload {
    let mut metadata =
        build_steady_state_execution_kernel_metadata(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY);
    metadata.insert("run_id".to_string(), json!(live_result.run_id));
    metadata.insert("status".to_string(), json!(live_result.status));
    metadata.insert(
        "research_mode".to_string(),
        Value::String(infer_research_mode(payload).as_str().to_string()),
    );
    metadata.insert(
        "finish_reason".to_string(),
        json!(live_result.finish_reason),
    );
    if live_result.continuation_attempted
        || live_result.continuation_status.is_some()
        || live_result.continuation_error.is_some()
    {
        metadata.insert(
            "continuation_attempted".to_string(),
            json!(live_result.continuation_attempted),
        );
        metadata.insert(
            "continuation_status".to_string(),
            json!(live_result.continuation_status),
        );
        metadata.insert(
            "continuation_error".to_string(),
            json!(live_result.continuation_error),
        );
    }
    metadata.insert(
        "trace_event_count".to_string(),
        json!(payload.trace_event_count),
    );
    metadata.insert(
        "trace_output_path".to_string(),
        json!(payload.trace_output_path),
    );
    metadata.insert(
        "execution_mode".to_string(),
        Value::String("live".to_string()),
    );
    metadata.insert("route_engine".to_string(), json!(payload.route_engine));
    metadata.insert(
        "diagnostic_route_mode".to_string(),
        json!(payload.diagnostic_route_mode),
    );
    metadata.insert(
        "execution_kernel_model_id_source".to_string(),
        Value::String(EXECUTION_MODEL_ID_SOURCE.to_string()),
    );
    ExecuteResponsePayload {
        execution_schema_version: EXECUTION_SCHEMA_VERSION.to_string(),
        authority: EXECUTION_AUTHORITY.to_string(),
        session_id: payload.session_id.clone(),
        user_id: payload.user_id.clone(),
        skill: payload.selected_skill.clone(),
        overlay: payload.overlay_skill.clone(),
        live_run: true,
        content: live_result.content,
        usage: ExecuteUsagePayload {
            input_tokens: live_result.input_tokens,
            output_tokens: live_result.output_tokens,
            total_tokens: live_result.total_tokens,
            mode: "live".to_string(),
        },
        prompt_preview,
        model_id: live_result.model_id.clone(),
        metadata: Value::Object(metadata),
    }
}

pub(crate) fn required_non_empty_string(
    payload: &Value,
    key: &str,
    context: &str,
) -> Result<String, String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("{context} requires non-empty {key}"))
}

// Retained for integration tests (`main_tests.rs::copy_text_to_clipboard`); retired top-level
// continuity refresh CLI path was removed. Gated `#[cfg(test)]` to make the "test-only" intent
// machine-checked: prod builds neither emit nor link it.
#[cfg(test)]
pub(crate) fn copy_text_to_clipboard(text: &str) -> Result<Value, String> {
    if let Ok(path_value) = std::env::var("ROUTER_RS_CLIPBOARD_PATH") {
        let path = PathBuf::from(path_value);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create clipboard file parent failed: {err}"))?;
        }
        fs::write(&path, text).map_err(|err| format!("write clipboard file failed: {err}"))?;
        return Ok(json!({
            "backend": "file",
            "target_path": path.display().to_string(),
        }));
    }

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("launch pbcopy failed: {err}"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "pbcopy stdin is unavailable".to_string())?;
        stdin
            .write_all(text.as_bytes())
            .map_err(|err| format!("write pbcopy stdin failed: {err}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("wait for pbcopy failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "pbcopy exited with a non-zero status".to_string()
        } else {
            format!("pbcopy failed: {stderr}")
        });
    }
    Ok(json!({
        "backend": "pbcopy",
    }))
}

pub(crate) fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

fn nested_value<'a>(payload: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = payload;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn nested_non_empty_string(payload: &Value, path: &[&str]) -> Option<String> {
    nested_value(payload, path)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn nested_bool(payload: &Value, path: &[&str]) -> Option<bool> {
    nested_value(payload, path).and_then(Value::as_bool)
}

fn build_attach_target_payload(
    session_id: &str,
    job_id: Option<&str>,
    endpoint_kind: &str,
    subscribe_method: &str,
    describe_method: &str,
    cleanup_method: &str,
    handoff_method: &str,
) -> Value {
    serde_json::json!({
        "endpoint_kind": endpoint_kind,
        "subscribe_method": subscribe_method,
        "describe_method": describe_method,
        "cleanup_method": cleanup_method,
        "handoff_method": handoff_method,
        "session_id": session_id,
        "job_id": job_id,
    })
}

fn build_replay_anchor_payload(
    latest_cursor: Value,
    resume_mode: &str,
    replay_supported: bool,
) -> Value {
    serde_json::json!({
        "anchor_kind": "trace_replay_cursor",
        "cursor_schema_version": "runtime-trace-cursor-v1",
        "resume_mode": resume_mode,
        "latest_cursor": latest_cursor,
        "replay_supported": replay_supported,
    })
}

fn build_transport_health_payload(payload: &Value) -> Value {
    if payload.get("transport_health").is_some() {
        return payload
            .get("transport_health")
            .cloned()
            .unwrap_or(Value::Null);
    }
    if payload.get("control_plane").is_none() {
        return Value::Null;
    }
    serde_json::json!({
        "backend_family": nested_non_empty_string(payload, &["control_plane", "backend_family"]),
        "supports_atomic_replace": nested_bool(payload, &["control_plane", "supports_atomic_replace"]),
        "supports_compaction": nested_bool(payload, &["control_plane", "supports_compaction"]),
        "supports_snapshot_delta": nested_bool(payload, &["control_plane", "supports_snapshot_delta"]),
        "supports_remote_event_transport": nested_bool(payload, &["control_plane", "supports_remote_event_transport"]),
    })
}

fn build_trace_transport_payload(
    payload: &Value,
    session_id: String,
    job_id: Option<String>,
) -> Value {
    let stream_key = job_id.as_deref().unwrap_or(session_id.as_str());
    let endpoint_kind = optional_non_empty_string(payload, "endpoint_kind")
        .unwrap_or_else(|| "runtime_method".to_string());
    let subscribe_method = optional_non_empty_string(payload, "subscribe_method")
        .unwrap_or_else(|| "subscribe_runtime_events".to_string());
    let describe_method = optional_non_empty_string(payload, "describe_method")
        .unwrap_or_else(|| "describe_runtime_event_transport".to_string());
    let cleanup_method = optional_non_empty_string(payload, "cleanup_method")
        .unwrap_or_else(|| "cleanup_runtime_events".to_string());
    let handoff_method = optional_non_empty_string(payload, "handoff_method")
        .unwrap_or_else(|| "describe_runtime_event_handoff".to_string());
    let resume_mode = optional_non_empty_string(payload, "resume_mode")
        .unwrap_or_else(|| "after_event_id".to_string());
    let replay_supported = optional_bool(payload, "replay_supported").unwrap_or(true);
    let latest_cursor = payload.get("latest_cursor").cloned().unwrap_or(Value::Null);
    let binding_backend_family = optional_non_empty_string(payload, "binding_backend_family")
        .or_else(|| nested_non_empty_string(payload, &["control_plane", "backend_family"]));
    let control_plane_authority = optional_non_empty_string(payload, "control_plane_authority")
        .or_else(|| {
            nested_non_empty_string(payload, &["control_plane", "trace_service", "authority"])
        });
    let control_plane_role = optional_non_empty_string(payload, "control_plane_role")
        .or_else(|| nested_non_empty_string(payload, &["control_plane", "trace_service", "role"]));
    let control_plane_projection = optional_non_empty_string(payload, "control_plane_projection")
        .or_else(|| {
            nested_non_empty_string(payload, &["control_plane", "trace_service", "projection"])
        });
    let control_plane_delegate_kind =
        optional_non_empty_string(payload, "control_plane_delegate_kind").or_else(|| {
            nested_non_empty_string(
                payload,
                &["control_plane", "trace_service", "delegate_kind"],
            )
        });
    let attach_target = build_attach_target_payload(
        &session_id,
        job_id.as_deref(),
        &endpoint_kind,
        &subscribe_method,
        &describe_method,
        &cleanup_method,
        &handoff_method,
    );
    let replay_anchor =
        build_replay_anchor_payload(latest_cursor.clone(), &resume_mode, replay_supported);

    serde_json::json!({
        "schema_version": "runtime-event-transport-v1",
        "stream_id": format!("stream::{stream_key}"),
        "session_id": session_id,
        "job_id": job_id,
        "transport_contract_kind": optional_non_empty_string(payload, "transport_contract_kind").unwrap_or_else(|| "runtime_event_stream".to_string()),
        "transport_family": optional_non_empty_string(payload, "transport_family").unwrap_or_else(|| "host-facing-transport".to_string()),
        "transport_kind": optional_non_empty_string(payload, "transport_kind").unwrap_or_else(|| "poll".to_string()),
        "endpoint_kind": endpoint_kind,
        "ownership_lane": optional_non_empty_string(payload, "ownership_lane").unwrap_or_else(|| "rust-contract-lane".to_string()),
        "producer_owner": optional_non_empty_string(payload, "producer_owner").unwrap_or_else(|| "rust-control-plane".to_string()),
        "producer_authority": optional_non_empty_string(payload, "producer_authority").unwrap_or_else(|| RUNTIME_CONTROL_PLANE_AUTHORITY.to_string()),
        "exporter_owner": optional_non_empty_string(payload, "exporter_owner").unwrap_or_else(|| "rust-control-plane".to_string()),
        "exporter_authority": optional_non_empty_string(payload, "exporter_authority").unwrap_or_else(|| RUNTIME_CONTROL_PLANE_AUTHORITY.to_string()),
        "remote_capable": optional_bool(payload, "remote_capable")
            .or_else(|| nested_bool(payload, &["control_plane", "supports_remote_event_transport"]))
            .unwrap_or(true),
        "remote_attach_supported": optional_bool(payload, "remote_attach_supported")
            .or_else(|| nested_bool(payload, &["control_plane", "supports_remote_event_transport"]))
            .unwrap_or(true),
        "handoff_supported": optional_bool(payload, "handoff_supported").unwrap_or(true),
        "handoff_method": handoff_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "describe_method": describe_method,
        "handoff_kind": optional_non_empty_string(payload, "handoff_kind").unwrap_or_else(|| "artifact_handoff".to_string()),
        "binding_refresh_mode": optional_non_empty_string(payload, "binding_refresh_mode").unwrap_or_else(|| "describe_or_checkpoint".to_string()),
        "binding_artifact_format": optional_non_empty_string(payload, "binding_artifact_format").unwrap_or_else(|| "json".to_string()),
        "binding_backend_family": binding_backend_family,
        "binding_artifact_path": optional_non_empty_string(payload, "binding_artifact_path"),
        "resume_mode": resume_mode,
        "heartbeat_supported": optional_bool(payload, "heartbeat_supported").unwrap_or(true),
        "cleanup_semantics": optional_non_empty_string(payload, "cleanup_semantics").unwrap_or_else(|| "stream_cache_only".to_string()),
        "cleanup_preserves_replay": optional_bool(payload, "cleanup_preserves_replay").unwrap_or(true),
        "replay_reseed_supported": optional_bool(payload, "replay_reseed_supported").unwrap_or(true),
        "chunk_schema_version": optional_non_empty_string(payload, "chunk_schema_version").unwrap_or_else(|| "runtime-event-stream-v1".to_string()),
        "cursor_schema_version": optional_non_empty_string(payload, "cursor_schema_version").unwrap_or_else(|| "runtime-trace-cursor-v1".to_string()),
        "latest_cursor": latest_cursor,
        "replay_supported": replay_supported,
        "attach_target": payload.get("attach_target").cloned().unwrap_or(attach_target),
        "replay_anchor": payload.get("replay_anchor").cloned().unwrap_or(replay_anchor),
        "control_plane_authority": control_plane_authority,
        "control_plane_role": control_plane_role,
        "control_plane_projection": control_plane_projection,
        "control_plane_delegate_kind": control_plane_delegate_kind,
        "transport_health": build_transport_health_payload(payload),
    })
}

fn build_trace_transport_descriptor(payload: Value) -> Result<Value, String> {
    let session_id = required_non_empty_string(&payload, "session_id", "describe transport")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    Ok(serde_json::json!({
        "schema_version": TRACE_DESCRIPTOR_SCHEMA_VERSION,
        "authority": TRACE_DESCRIPTOR_AUTHORITY,
        "transport": build_trace_transport_payload(&payload, session_id, job_id),
    }))
}

fn build_trace_handoff_descriptor(payload: Value) -> Result<Value, String> {
    let session_id = required_non_empty_string(&payload, "session_id", "describe handoff")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let transport_source = payload.get("transport").cloned().unwrap_or(Value::Null);
    let transport_session_id = optional_non_empty_string(&transport_source, "session_id")
        .unwrap_or_else(|| session_id.clone());
    let transport_job_id =
        optional_non_empty_string(&transport_source, "job_id").or_else(|| job_id.clone());
    let transport = if transport_source.is_object() {
        build_trace_transport_payload(
            &transport_source,
            transport_session_id.clone(),
            transport_job_id.clone(),
        )
    } else {
        build_trace_transport_payload(&payload, session_id.clone(), job_id.clone())
    };
    let checkpoint_backend_family =
        optional_non_empty_string(&payload, "checkpoint_backend_family")
            .or_else(|| optional_non_empty_string(&transport, "binding_backend_family"))
            .or_else(|| nested_non_empty_string(&payload, &["control_plane", "backend_family"]))
            .unwrap_or_else(|| "filesystem".to_string());
    let trace_stream_path = optional_non_empty_string(&payload, "trace_stream_path");
    let resume_manifest_path = optional_non_empty_string(&payload, "resume_manifest_path");
    let recovery_artifacts = payload
        .get("recovery_artifacts")
        .cloned()
        .filter(Value::is_array)
        .unwrap_or_else(|| {
            let mut ordered: Vec<String> = Vec::new();
            if let Some(path) = optional_non_empty_string(&transport, "binding_artifact_path") {
                ordered.push(path);
            }
            if let Some(path) = resume_manifest_path.clone() {
                ordered.push(path);
            }
            if let Some(path) = trace_stream_path.clone() {
                ordered.push(path);
            }
            serde_json::json!(ordered)
        });

    Ok(serde_json::json!({
        "schema_version": TRACE_DESCRIPTOR_SCHEMA_VERSION,
        "authority": TRACE_DESCRIPTOR_AUTHORITY,
        "handoff": {
            "schema_version": "runtime-event-handoff-v1",
            "stream_id": transport.get("stream_id").cloned().unwrap_or_else(|| Value::String(format!("stream::{}", transport_job_id.as_deref().unwrap_or(transport_session_id.as_str())))),
            "session_id": session_id,
            "job_id": job_id,
            "checkpoint_backend_family": checkpoint_backend_family,
            "trace_stream_path": trace_stream_path,
            "resume_manifest_path": resume_manifest_path,
            "remote_attach_strategy": optional_non_empty_string(&payload, "remote_attach_strategy").unwrap_or_else(|| "transport_descriptor_then_replay".to_string()),
            "cleanup_preserves_replay": transport.get("cleanup_preserves_replay").and_then(Value::as_bool).unwrap_or(true),
            "attach_target": transport.get("attach_target").cloned().unwrap_or(Value::Null),
            "replay_anchor": transport.get("replay_anchor").cloned().unwrap_or(Value::Null),
            "recovery_artifacts": recovery_artifacts,
            "control_plane": payload.get("control_plane").cloned().unwrap_or(Value::Null),
            "transport": transport,
        },
    }))
}

fn build_checkpoint_resume_manifest(payload: Value) -> Result<Value, String> {
    let session_id =
        required_non_empty_string(&payload, "session_id", "checkpoint resume manifest")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let status =
        optional_non_empty_string(&payload, "status").unwrap_or_else(|| "running".to_string());
    let generation = payload
        .get("generation")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let mut resume_manifest = serde_json::json!({
        "schema_version": "runtime-resume-manifest-v1",
        "session_id": session_id,
        "job_id": job_id,
        "status": status,
        "generation": generation,
        "trace_output_path": optional_non_empty_string(&payload, "trace_output_path"),
        "trace_stream_path": optional_non_empty_string(&payload, "trace_stream_path"),
        "event_transport_path": optional_non_empty_string(&payload, "event_transport_path"),
        "background_state_path": optional_non_empty_string(&payload, "background_state_path"),
        "latest_cursor": payload.get("latest_cursor").cloned().unwrap_or(Value::Null),
        "artifact_paths": payload
            .get("artifact_paths")
            .cloned()
            .filter(Value::is_array)
            .unwrap_or_else(|| serde_json::json!([])),
        "parallel_group": payload.get("parallel_group").cloned().unwrap_or(Value::Null),
        "supervisor_projection": payload.get("supervisor_projection").cloned().unwrap_or(Value::Null),
        "control_plane": payload.get("control_plane").cloned().unwrap_or(Value::Null),
    });
    if let Some(updated_at) = optional_non_empty_string(&payload, "updated_at") {
        if let Some(map) = resume_manifest.as_object_mut() {
            map.insert("updated_at".to_string(), Value::String(updated_at));
        }
    }
    Ok(serde_json::json!({
        "schema_version": CHECKPOINT_RESUME_MANIFEST_SCHEMA_VERSION,
        "authority": CHECKPOINT_RESUME_MANIFEST_AUTHORITY,
        "resume_manifest": resume_manifest,
    }))
}

fn write_json_payload(path: &Path, payload: &Value) -> Result<usize, String> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| format!("serialize persisted payload failed: {err}"))?
    );
    write_text_payload(path, &serialized)
}

fn write_transport_binding_payload(payload: Value) -> Result<Value, String> {
    let path = required_non_empty_string(&payload, "path", "write transport binding")?;
    let session_id = required_non_empty_string(&payload, "session_id", "write transport binding")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let transport = build_trace_transport_payload(&payload, session_id, job_id);
    let bytes_written = write_json_payload(Path::new(&path), &transport)?;
    Ok(serde_json::json!({
        "schema_version": TRANSPORT_BINDING_WRITE_SCHEMA_VERSION,
        "authority": TRANSPORT_BINDING_WRITE_AUTHORITY,
        "path": path,
        "bytes_written": bytes_written,
    }))
}

fn write_checkpoint_resume_manifest_payload(payload: Value) -> Result<Value, String> {
    let path = required_non_empty_string(&payload, "path", "write checkpoint resume manifest")?;
    let manifest = build_checkpoint_resume_manifest(payload)?
        .get("resume_manifest")
        .cloned()
        .ok_or_else(|| "checkpoint resume manifest payload missing resume_manifest".to_string())?;
    let bytes_written = write_json_payload(Path::new(&path), &manifest)?;
    Ok(serde_json::json!({
        "schema_version": CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION,
        "authority": CHECKPOINT_MANIFEST_WRITE_AUTHORITY,
        "path": path,
        "bytes_written": bytes_written,
    }))
}

pub(crate) fn write_text_payload(path: &Path, payload: &str) -> Result<usize, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create parent directory for {} failed: {err}",
                path.display()
            )
        })?;
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("persist path {} has no file name", path.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = WRITE_TEXT_PAYLOAD_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let thread_id = format!("{:?}", std::thread::current().id())
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    let tmp_path = path.with_file_name(format!(
        "{file_name}.{}.{}.{}.{}.tmp",
        std::process::id(),
        thread_id,
        nonce,
        sequence
    ));
    fs::write(&tmp_path, payload.as_bytes())
        .map_err(|err| format!("write temp payload {} failed: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!("replace payload {} failed: {err}", path.display())
    })?;
    Ok(payload.len())
}

fn descriptor_mapping<'a>(
    attach_descriptor: &'a Value,
    field_name: &str,
) -> Result<Option<&'a Map<String, Value>>, String> {
    match attach_descriptor.get(field_name) {
        None => Ok(None),
        Some(Value::Object(map)) => Ok(Some(map)),
        Some(_) => Err(format!(
            "External runtime event attach descriptor field {field_name:?} must be a mapping."
        )),
    }
}

fn mapping_string(
    mapping: &Map<String, Value>,
    field_name: &str,
) -> Result<Option<String>, String> {
    match mapping.get(field_name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!(
            "External runtime event attach descriptor field {field_name:?} must be a string."
        )),
    }
}

fn merge_attach_path_values(
    explicit_value: Option<String>,
    descriptor_value: Option<String>,
    field_name: &str,
) -> Result<Option<String>, String> {
    match (explicit_value, descriptor_value) {
        (None, descriptor) => Ok(descriptor),
        (Some(explicit), None) => Ok(Some(explicit)),
        (Some(explicit), Some(descriptor)) if explicit == descriptor => Ok(Some(explicit)),
        (Some(_), Some(_)) => Err(format!(
            "External runtime event attach received conflicting {field_name:?} values between direct args and attach_descriptor."
        )),
    }
}

struct NormalizedAttachRequest {
    binding_artifact_path: Option<String>,
    handoff_path: Option<String>,
    resume_manifest_path: Option<String>,
    trace_stream_path: Option<String>,
    binding_artifact_resolution: Option<String>,
    handoff_resolution: Option<String>,
    resume_manifest_resolution: Option<String>,
}

fn normalize_attach_request(payload: &Value) -> Result<NormalizedAttachRequest, String> {
    let explicit_binding_artifact_path =
        optional_non_empty_string(payload, "binding_artifact_path");
    let explicit_handoff_path = optional_non_empty_string(payload, "handoff_path");
    let explicit_resume_manifest_path = optional_non_empty_string(payload, "resume_manifest_path");
    let Some(attach_descriptor) = payload.get("attach_descriptor") else {
        return Ok(NormalizedAttachRequest {
            binding_artifact_path: explicit_binding_artifact_path.clone(),
            handoff_path: explicit_handoff_path.clone(),
            resume_manifest_path: explicit_resume_manifest_path.clone(),
            trace_stream_path: None,
            binding_artifact_resolution: explicit_binding_artifact_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
            handoff_resolution: explicit_handoff_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
            resume_manifest_resolution: explicit_resume_manifest_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
        });
    };
    if attach_descriptor.is_null() {
        return Ok(NormalizedAttachRequest {
            binding_artifact_path: explicit_binding_artifact_path.clone(),
            handoff_path: explicit_handoff_path.clone(),
            resume_manifest_path: explicit_resume_manifest_path.clone(),
            trace_stream_path: None,
            binding_artifact_resolution: explicit_binding_artifact_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
            handoff_resolution: explicit_handoff_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
            resume_manifest_resolution: explicit_resume_manifest_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
        });
    }
    if !attach_descriptor.is_object() {
        return Err("External runtime event attach descriptor must be a mapping.".to_string());
    }
    let schema_version = attach_descriptor
        .get("schema_version")
        .and_then(Value::as_str);
    if let Some(schema_version) = schema_version {
        if schema_version != "runtime-event-attach-descriptor-v1" {
            return Err(format!(
                "Unsupported runtime event attach descriptor schema: {schema_version:?}"
            ));
        }
    }
    let attach_mode = attach_descriptor.get("attach_mode").and_then(Value::as_str);
    if let Some(attach_mode) = attach_mode {
        if attach_mode != "process_external_artifact_replay" {
            return Err(format!(
                "Unsupported runtime event attach mode: {attach_mode:?}"
            ));
        }
    }
    let expected_scalars = [
        (
            "source_transport_method",
            "describe_runtime_event_transport",
        ),
        ("source_handoff_method", "describe_runtime_event_handoff"),
        ("attach_method", "attach_runtime_event_transport"),
        ("subscribe_method", "subscribe_attached_runtime_events"),
        ("cleanup_method", "cleanup_attached_runtime_event_transport"),
        ("resume_mode", "after_event_id"),
    ];
    for (field_name, expected) in expected_scalars {
        if let Some(value) = attach_descriptor.get(field_name).and_then(Value::as_str) {
            if value != expected {
                return Err(format!(
                    "External runtime event attach descriptor must use {field_name}={expected:?}."
                ));
            }
        }
    }
    if let Some(capabilities) = descriptor_mapping(attach_descriptor, "attach_capabilities")? {
        if capabilities.get("artifact_replay").and_then(Value::as_bool) != Some(true) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.artifact_replay=True."
                    .to_string(),
            );
        }
        if !matches!(
            capabilities
                .get("live_remote_stream")
                .and_then(Value::as_bool),
            None | Some(false)
        ) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.live_remote_stream=False."
                    .to_string(),
            );
        }
        if !matches!(
            capabilities
                .get("cleanup_preserves_replay")
                .and_then(Value::as_bool),
            None | Some(true)
        ) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.cleanup_preserves_replay=True."
                    .to_string(),
            );
        }
    }
    let _ = descriptor_mapping(attach_descriptor, "requested_artifacts")?;
    let resolution_mapping = descriptor_mapping(attach_descriptor, "resolution")?;
    let resolved_mapping = descriptor_mapping(attach_descriptor, "resolved_artifacts")?
        .unwrap_or_else(|| {
            attach_descriptor
                .as_object()
                .expect("attach descriptor object")
        });
    let descriptor_binding = mapping_string(resolved_mapping, "binding_artifact_path")?;
    let descriptor_handoff = mapping_string(resolved_mapping, "handoff_path")?;
    let descriptor_resume = mapping_string(resolved_mapping, "resume_manifest_path")?;
    let descriptor_trace_stream = mapping_string(resolved_mapping, "trace_stream_path")?;
    let binding_artifact_path = merge_attach_path_values(
        explicit_binding_artifact_path.clone(),
        descriptor_binding,
        "binding_artifact_path",
    )?;
    let handoff_path = merge_attach_path_values(
        explicit_handoff_path.clone(),
        descriptor_handoff,
        "handoff_path",
    )?;
    let resume_manifest_path = merge_attach_path_values(
        explicit_resume_manifest_path.clone(),
        descriptor_resume,
        "resume_manifest_path",
    )?;
    Ok(NormalizedAttachRequest {
        binding_artifact_path,
        handoff_path,
        resume_manifest_path,
        trace_stream_path: descriptor_trace_stream,
        binding_artifact_resolution: if explicit_binding_artifact_path.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "binding_artifact_path"))
                .transpose()?
                .flatten()
        },
        handoff_resolution: if explicit_handoff_path.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "handoff_path"))
                .transpose()?
                .flatten()
        },
        resume_manifest_resolution: if explicit_resume_manifest_path.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "resume_manifest_path"))
                .transpose()?
                .flatten()
        },
    })
}

fn require_requested_artifact(
    path: &Option<PathBuf>,
    storage_backend: Option<&ResolvedStorageBackend>,
    field_name: &str,
) -> Result<(), String> {
    if let Some(path) = path {
        if !storage_artifact_exists(path, storage_backend) {
            return Err(format!(
                "External runtime event attach requested {field_name:?} that does not exist: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn load_json_artifact(
    path: &Option<PathBuf>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<Option<Value>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !storage_artifact_exists(path, storage_backend) {
        return Ok(None);
    }
    serde_json::from_str::<Value>(&storage_read_text(path, storage_backend)?)
        .map(Some)
        .map_err(|err| {
            format!(
                "parse runtime attach artifact failed for {}: {err}",
                path.display()
            )
        })
}

fn json_path(value: &Value, key: &str) -> Result<Option<PathBuf>, String> {
    normalize_optional_runtime_path(optional_non_empty_string(value, key))
}

fn nested_json_path(value: &Value, path: &[&str]) -> Result<Option<PathBuf>, String> {
    normalize_optional_runtime_path(nested_non_empty_string(value, path))
}

fn normalize_optional_runtime_path(value: Option<String>) -> Result<Option<PathBuf>, String> {
    value
        .map(|path| {
            let candidate = PathBuf::from(path.trim());
            if candidate.as_os_str().is_empty() {
                return Err("runtime attach path must be non-empty".to_string());
            }
            if candidate.is_absolute() {
                Ok(candidate)
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(candidate))
                    .map_err(|err| format!("resolve runtime attach path failed: {err}"))
            }
        })
        .transpose()
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn infer_resume_manifest_path(binding_artifact_path: &Path) -> PathBuf {
    let candidates = [
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json")),
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| {
            binding_artifact_path
                .parent()
                .and_then(Path::parent)
                .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json"))
                .unwrap_or_else(|| PathBuf::from("TRACE_RESUME_MANIFEST.json"))
        })
}

fn infer_trace_stream_from_binding_artifact(
    binding_artifact_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Option<PathBuf> {
    let binding_artifact_path = binding_artifact_path?;
    let candidates = [
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_EVENTS.jsonl")),
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_EVENTS.jsonl")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| storage_artifact_exists(candidate, storage_backend))
}

fn validate_attached_runtime_alignment(
    transport: &Value,
    handoff: Option<&Value>,
    resume_manifest: Option<&Value>,
    binding_artifact_path: Option<&Path>,
    resume_manifest_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<(), String> {
    let transport_stream_id = optional_non_empty_string(transport, "stream_id");
    let transport_session_id = optional_non_empty_string(transport, "session_id");
    let transport_job_id = optional_non_empty_string(transport, "job_id");

    if let Some(handoff) = handoff {
        if optional_non_empty_string(handoff, "stream_id") != transport_stream_id {
            return Err(
                "External runtime event attach rejected mismatched transport/handoff stream ids."
                    .to_string(),
            );
        }
        if optional_non_empty_string(handoff, "session_id") != transport_session_id
            || optional_non_empty_string(handoff, "job_id") != transport_job_id
        {
            return Err(
                "External runtime event attach rejected mismatched transport/handoff stream scope."
                    .to_string(),
            );
        }
        if let (Some(binding_artifact_path), Some(handoff_binding_path)) = (
            binding_artifact_path,
            nested_json_path(handoff, &["transport", "binding_artifact_path"])?,
        ) {
            if normalize_path_for_compare(&handoff_binding_path)
                != normalize_path_for_compare(binding_artifact_path)
            {
                return Err("External runtime event attach rejected mismatched transport/handoff binding artifact paths.".to_string());
            }
        }
        if let (Some(resume_manifest_path), Some(handoff_resume_manifest_path)) = (
            resume_manifest_path,
            json_path(handoff, "resume_manifest_path")?,
        ) {
            if normalize_path_for_compare(&handoff_resume_manifest_path)
                != normalize_path_for_compare(resume_manifest_path)
            {
                return Err("External runtime event attach rejected mismatched handoff/resume manifest paths.".to_string());
            }
        }
    }

    if let Some(resume_manifest) = resume_manifest {
        if optional_non_empty_string(resume_manifest, "session_id") != transport_session_id
            || optional_non_empty_string(resume_manifest, "job_id") != transport_job_id
        {
            return Err(
                "External runtime event attach rejected mismatched transport/resume stream scope."
                    .to_string(),
            );
        }
        if let (Some(binding_artifact_path), Some(resume_binding_path)) = (
            binding_artifact_path,
            json_path(resume_manifest, "event_transport_path")?,
        ) {
            if normalize_path_for_compare(&resume_binding_path)
                != normalize_path_for_compare(binding_artifact_path)
            {
                return Err("External runtime event attach rejected mismatched transport/resume binding artifact paths.".to_string());
            }
        }
        if let (Some(_handoff), Some(handoff_trace_stream_path), Some(resume_trace_stream_path)) = (
            handoff,
            handoff
                .map(|value| json_path(value, "trace_stream_path"))
                .transpose()?
                .flatten(),
            json_path(resume_manifest, "trace_stream_path")?,
        ) {
            if normalize_path_for_compare(&handoff_trace_stream_path)
                != normalize_path_for_compare(&resume_trace_stream_path)
            {
                return Err("External runtime event attach rejected mismatched handoff/resume trace stream paths.".to_string());
            }
        }
    }

    let binding_trace_stream_path =
        infer_trace_stream_from_binding_artifact(binding_artifact_path, storage_backend);
    if let (Some(binding_trace_stream_path), Some(_handoff), Some(handoff_trace_stream_path)) = (
        binding_trace_stream_path.as_ref(),
        handoff,
        handoff
            .map(|value| json_path(value, "trace_stream_path"))
            .transpose()?
            .flatten(),
    ) {
        if normalize_path_for_compare(&handoff_trace_stream_path)
            != normalize_path_for_compare(binding_trace_stream_path)
        {
            return Err("External runtime event attach rejected mismatched binding/handoff trace stream paths.".to_string());
        }
    }
    if let (
        Some(binding_trace_stream_path),
        Some(_resume_manifest),
        Some(resume_trace_stream_path),
    ) = (
        binding_trace_stream_path.as_ref(),
        resume_manifest,
        resume_manifest
            .map(|value| json_path(value, "trace_stream_path"))
            .transpose()?
            .flatten(),
    ) {
        if normalize_path_for_compare(&resume_trace_stream_path)
            != normalize_path_for_compare(binding_trace_stream_path)
        {
            return Err("External runtime event attach rejected mismatched binding/resume trace stream paths.".to_string());
        }
    }
    Ok(())
}

fn trace_stream_resolution(
    handoff: Option<&Value>,
    resume_manifest: Option<&Value>,
    binding_artifact_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<Option<(PathBuf, String)>, String> {
    if let Some(handoff) = handoff {
        if let Some(path) = json_path(handoff, "trace_stream_path")? {
            return Ok(Some((path, "handoff_manifest".to_string())));
        }
    }
    if let Some(resume_manifest) = resume_manifest {
        if let Some(path) = json_path(resume_manifest, "trace_stream_path")? {
            return Ok(Some((path, "resume_manifest".to_string())));
        }
    }
    if let Some(path) =
        infer_trace_stream_from_binding_artifact(binding_artifact_path, storage_backend)
    {
        return Ok(Some((path, "binding_artifact_adjacency".to_string())));
    }
    Ok(None)
}

pub(crate) fn attach_runtime_event_transport(payload: Value) -> Result<Value, String> {
    let normalized_request = normalize_attach_request(&payload)?;
    let binding_artifact_path = normalized_request.binding_artifact_path;
    let handoff_path = normalized_request.handoff_path;
    let resume_manifest_path = normalized_request.resume_manifest_path;
    let descriptor_trace_stream_path =
        normalize_optional_runtime_path(normalized_request.trace_stream_path)?;
    if binding_artifact_path.is_none() && handoff_path.is_none() && resume_manifest_path.is_none() {
        return Err(
            "External runtime event attach requires a binding artifact, handoff manifest, or resume manifest path."
                .to_string(),
        );
    }

    let binding_path = normalize_optional_runtime_path(binding_artifact_path)?;
    let handoff_file = normalize_optional_runtime_path(handoff_path)?;
    let resume_file = normalize_optional_runtime_path(resume_manifest_path)?;
    let mut binding_source = normalized_request.binding_artifact_resolution;
    let handoff_source = normalized_request.handoff_resolution;
    let mut resume_source = normalized_request.resume_manifest_resolution;

    let requested_paths = [
        binding_path.clone(),
        handoff_file.clone(),
        resume_file.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let storage_backend = resolve_storage_backend(&requested_paths);
    require_requested_artifact(
        &binding_path,
        storage_backend.as_ref(),
        "binding_artifact_path",
    )?;
    require_requested_artifact(&handoff_file, storage_backend.as_ref(), "handoff_path")?;
    require_requested_artifact(
        &resume_file,
        storage_backend.as_ref(),
        "resume_manifest_path",
    )?;

    let handoff = load_json_artifact(&handoff_file, storage_backend.as_ref())?;
    let mut resume_manifest = load_json_artifact(&resume_file, storage_backend.as_ref())?;
    let mut resolved_resume_file = resume_file.clone();

    if resume_manifest.is_none() {
        if let Some(handoff_resume_path) = handoff
            .as_ref()
            .map(|payload| json_path(payload, "resume_manifest_path"))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&handoff_resume_path, storage_backend.as_ref()) {
                resolved_resume_file = Some(handoff_resume_path.clone());
                resume_manifest =
                    load_json_artifact(&Some(handoff_resume_path), storage_backend.as_ref())?;
                resume_source = Some("handoff_manifest".to_string());
            }
        }
    }

    let mut transport_path = binding_path.clone();
    if transport_path.is_none() {
        if let Some(resume_transport_path) = resume_manifest
            .as_ref()
            .map(|payload| json_path(payload, "event_transport_path"))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&resume_transport_path, storage_backend.as_ref()) {
                transport_path = Some(resume_transport_path);
                binding_source = Some("resume_manifest".to_string());
            }
        }
    }
    if transport_path.is_none() {
        if let Some(handoff_transport_path) = handoff
            .as_ref()
            .map(|payload| nested_json_path(payload, &["transport", "binding_artifact_path"]))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&handoff_transport_path, storage_backend.as_ref()) {
                transport_path = Some(handoff_transport_path);
                binding_source = Some("handoff_transport".to_string());
            }
        }
    }

    if transport_path.is_none() && handoff.is_none() {
        return Err(
            "External runtime event attach could not resolve a transport binding artifact from the provided manifests."
                .to_string(),
        );
    }

    let transport = if let Some(transport_path) = transport_path.as_ref() {
        load_json_artifact(&Some(transport_path.clone()), storage_backend.as_ref())?.ok_or_else(
            || "External runtime event attach could not load a transport descriptor.".to_string(),
        )?
    } else {
        handoff
            .as_ref()
            .and_then(|payload| payload.get("transport").cloned())
            .ok_or_else(|| {
                "External runtime event attach could not load a transport descriptor.".to_string()
            })?
    };

    if resume_manifest.is_none() {
        if let Some(transport_path) = transport_path.as_ref() {
            let inferred_resume_path = infer_resume_manifest_path(transport_path);
            if storage_artifact_exists(&inferred_resume_path, storage_backend.as_ref()) {
                resolved_resume_file = Some(inferred_resume_path.clone());
                resume_manifest =
                    load_json_artifact(&Some(inferred_resume_path), storage_backend.as_ref())?;
            }
        }
    }

    validate_attached_runtime_alignment(
        &transport,
        handoff.as_ref(),
        resume_manifest.as_ref(),
        transport_path.as_deref(),
        resolved_resume_file.as_deref(),
        storage_backend.as_ref(),
    )?;

    let Some((trace_stream_path, trace_stream_source)) = trace_stream_resolution(
        handoff.as_ref(),
        resume_manifest.as_ref(),
        transport_path.as_deref(),
        storage_backend.as_ref(),
    )?
    else {
        return Err(
            "External runtime event replay requires a handoff or resume manifest with trace_stream_path, or a filesystem binding artifact adjacent to TRACE_EVENTS.jsonl."
                .to_string(),
        );
    };
    if let Some(descriptor_trace_stream_path) = descriptor_trace_stream_path.as_ref() {
        if normalize_path_for_compare(descriptor_trace_stream_path)
            != normalize_path_for_compare(&trace_stream_path)
        {
            return Err(
                "External runtime event attach descriptor must already match canonical 'resolved_artifacts.trace_stream_path'."
                    .to_string(),
            );
        }
    }
    if !storage_artifact_exists(&trace_stream_path, storage_backend.as_ref()) {
        return Err(format!(
            "External runtime event replay trace stream not found: {}",
            trace_stream_path.display()
        ));
    }

    let resume_mode = optional_non_empty_string(&transport, "resume_mode")
        .unwrap_or_else(|| "after_event_id".to_string());
    let artifact_backend_family = optional_non_empty_string(&transport, "binding_backend_family")
        .unwrap_or_else(|| "filesystem".to_string());
    let source_transport_method = "describe_runtime_event_transport";
    let source_handoff_method = "describe_runtime_event_handoff";
    let attach_method = "attach_runtime_event_transport";
    let subscribe_method = "subscribe_attached_runtime_events";
    let cleanup_method = "cleanup_attached_runtime_event_transport";
    let cleanup_semantics = "no_persisted_state";
    let recommended_entrypoint = "describe_runtime_event_handoff";
    let attach_descriptor = json!({
        "schema_version": "runtime-event-attach-descriptor-v1",
        "attach_mode": "process_external_artifact_replay",
        "artifact_backend_family": artifact_backend_family.clone(),
        "source_transport_method": source_transport_method,
        "source_handoff_method": source_handoff_method,
        "attach_method": attach_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "resume_mode": resume_mode.clone(),
        "cleanup_semantics": cleanup_semantics,
        "attach_capabilities": {
            "artifact_replay": true,
            "live_remote_stream": false,
            "cleanup_preserves_replay": true,
        },
        "recommended_entrypoint": recommended_entrypoint,
        "requested_artifacts": {
            "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
            "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
            "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
        },
        "resolved_artifacts": {
            "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
            "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
            "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
            "trace_stream_path": trace_stream_path.display().to_string(),
        },
        "resolution": {
            "binding_artifact_path": binding_source,
            "handoff_path": handoff_source,
            "resume_manifest_path": resume_source,
            "trace_stream_path": trace_stream_source,
        },
    });

    Ok(json!({
        "attach_mode": "process_external_artifact_replay",
        "artifact_backend_family": artifact_backend_family,
        "source_handoff_method": source_handoff_method,
        "source_transport_method": source_transport_method,
        "attach_method": attach_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "resume_mode": resume_mode,
        "transport": transport,
        "handoff": handoff,
        "resume_manifest": resume_manifest,
        "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
        "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
        "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
        "trace_stream_path": trace_stream_path.display().to_string(),
        "replay_supported": true,
        "cleanup_semantics": cleanup_semantics,
        "cleanup_preserves_replay": true,
        "authority": ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY,
        "attach_descriptor": attach_descriptor,
    }))
}

pub(crate) fn subscribe_attached_runtime_events(payload: Value) -> Result<Value, String> {
    let attached = attach_runtime_event_transport(payload.clone())?;
    let transport = attached
        .get("transport")
        .ok_or_else(|| "attached runtime transport payload missing transport".to_string())?;
    let session_id = optional_non_empty_string(transport, "session_id")
        .ok_or_else(|| "attached runtime transport payload missing session_id".to_string())?;
    let job_id = optional_non_empty_string(transport, "job_id");
    let trace_stream_path = attached
        .get("trace_stream_path")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "attached runtime transport payload missing trace_stream_path".to_string()
        })?;
    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_stream_path.to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some(session_id.clone()),
        job_id: job_id.clone(),
        stream_scope_fields: None,
        after_event_id: optional_non_empty_string(&payload, "after_event_id"),
        limit: payload
            .get("limit")
            .and_then(Value::as_u64)
            .map(|value| value as usize),
    })?;
    let heartbeat = optional_bool(&payload, "heartbeat").unwrap_or(false);
    let events = replay.events.clone();
    let has_more = replay.has_more;
    let next_cursor = serde_json::to_value(&replay.next_cursor)
        .map_err(|err| format!("serialize attached runtime cursor failed: {err}"))?;
    Ok(json!({
        "schema_version": "runtime-event-stream-v1",
        "session_id": session_id,
        "job_id": job_id,
        "events": events,
        "next_cursor": next_cursor,
        "has_more": has_more,
        "after_event_id": optional_non_empty_string(&payload, "after_event_id"),
        "heartbeat": if heartbeat && replay.events.is_empty() {
            json!({
                "schema_version": "runtime-event-stream-heartbeat-v1",
                "kind": "runtime.stream.heartbeat",
                "status": "idle",
            })
        } else {
            Value::Null
        },
    }))
}

fn cleanup_attached_runtime_event_transport(payload: Value) -> Result<Value, String> {
    let attached = attach_runtime_event_transport(payload)?;
    Ok(json!({
        "authority": ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY,
        "cleanup_semantics": "no_persisted_state",
        "cleanup_preserves_replay": true,
        "cleanup_method": "cleanup_attached_runtime_event_transport",
        "binding_artifact_path": attached.get("binding_artifact_path").cloned().unwrap_or(Value::Null),
        "trace_stream_path": attached.get("trace_stream_path").cloned().unwrap_or(Value::Null),
    }))
}

pub(crate) fn build_runtime_control_plane_payload() -> Value {
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
            ).expect("default backend family parity is valid"),
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
            "sandbox_lifecycle_contract": {
                "schema_version": "runtime-sandbox-lifecycle-v1",
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "role": "sandbox-lifecycle-control",
                "projection": "rust-native-projection",
                "delegate_kind": "rust-runtime-control-plane",
                "lifecycle_states": [
                    "created",
                    "warm",
                    "busy",
                    "draining",
                    "recycled",
                    "failed"
                ],
                "allowed_transitions": [
                    ["busy", "draining"],
                    ["busy", "failed"],
                    ["created", "warm"],
                    ["draining", "failed"],
                    ["draining", "recycled"],
                    ["recycled", "warm"],
                    ["warm", "busy"],
                    ["warm", "failed"]
                ],
                "capability_categories": [
                    "read_only",
                    "workspace_mutating",
                    "networked",
                    "high_risk"
                ],
                "cleanup_mode": "async-drain-and-recycle",
                "event_log_artifact": "runtime_sandbox_events.jsonl",
                "event_schema_version": SANDBOX_EVENT_SCHEMA_VERSION,
                "event_tracing": {
                    "request_flag": "trace_event",
                    "path_field": "event_log_path",
                    "response_flag": "event_written",
                    "effective_capabilities_field": "effective_capabilities"
                },
                "control_operations": ["transition", "cleanup", "admit", "execution_result"],
                "runtime_probe_dimensions": ["cpu", "memory", "wall_clock", "output_size"],
            },
        },
        "background": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "background-orchestration",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-background-control-policy",
            "orchestration_contract": {
                "schema_version": "runtime-background-orchestration-v1",
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
            "framework_runtime_replacement_authority": framework_runtime::FRAMEWORK_RUNTIME_AUTHORITY,
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

pub(crate) fn build_runtime_integrator_payload() -> Value {
    let control_plane = build_runtime_control_plane_payload();
    let runtime_host = control_plane
        .get("runtime_host")
        .cloned()
        .unwrap_or(Value::Null);
    let services = control_plane
        .get("services")
        .cloned()
        .unwrap_or(Value::Null);
    let runtime_status = control_plane
        .get("runtime_status")
        .cloned()
        .unwrap_or(Value::Null);
    let concurrency_contract = runtime_host
        .get("concurrency_contract")
        .cloned()
        .unwrap_or(Value::Null);
    let subagent_limit_contract = services
        .get("middleware")
        .and_then(Value::as_object)
        .and_then(|middleware| middleware.get("subagent_limit_contract"))
        .cloned()
        .unwrap_or(Value::Null);
    let observability_exporter = build_runtime_observability_exporter_descriptor();
    let observability_metric_catalog = build_runtime_observability_metric_catalog_payload();
    let observability_dashboard = runtime_observability_dashboard_schema();
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
            "ownership_lane": observability_exporter["ownership_lane"].clone(),
            "metric_catalog_version": observability_exporter["metric_catalog_version"].clone(),
            "dashboard_schema_version": observability_dashboard["schema_version"].clone(),
            "resource_dimensions": observability_dashboard["resource_dimensions"].clone(),
            "metric_catalog_schema_version": observability_metric_catalog["schema_version"].clone(),
            "metric_names": observability_metric_catalog["metrics"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|metric| metric.get("metric_name").cloned())
                .collect::<Vec<Value>>(),
            "dashboard_panel_count": observability_dashboard["panels"]
                .as_array()
                .map(|items| items.len())
                .unwrap_or(0),
            "dashboard_alert_count": observability_dashboard["alerts"]
                .as_array()
                .map(|items| items.len())
                .unwrap_or(0),
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
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "rate(compression_offload_total) / rate(compression_candidate_total)",
        }),
        json!({
            "intent": "sandbox timeout rate",
            "metric_name": "runtime.sandbox_timeout_total",
            "metric_type": "counter",
            "unit": "1",
            "base_dimensions": base_dimensions,
            "dashboard_derivation": "rate(sandbox_timeout_total) / rate(sandbox_execution_total)",
        }),
    ]
}

pub(crate) fn build_runtime_observability_metric_catalog_payload() -> Value {
    let metrics = runtime_observability_metric_catalog()
        .into_iter()
        .map(|metric| {
            let mut metric_object = metric;
            if let Some(base_dimensions) = metric_object.get("base_dimensions").cloned() {
                if let Some(object) = metric_object.as_object_mut() {
                    object.remove("base_dimensions");
                    object.insert("dimensions".to_string(), base_dimensions);
                }
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

pub(crate) fn build_runtime_observability_exporter_descriptor() -> Value {
    json!({
        "schema_version": RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
        "metric_catalog_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
        "dashboard_schema_version": RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
        "signal_vocabulary": RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY,
        "export_path": "jsonl-plus-otel",
        "jsonl_sink_schema_version": "runtime-event-sink-v1",
        "trace_stream_schema_version": "runtime-event-stream-v1",
        "trace_handoff_schema_version": "runtime-event-handoff-v1",
        "ownership_lane": "rust-contract-lane",
        "producer_owner": "rust-control-plane",
        "producer_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
        "exporter_owner": "rust-control-plane",
        "exporter_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
    })
}

pub(crate) fn build_runtime_observability_health_snapshot() -> Value {
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

pub(crate) fn runtime_observability_dashboard_schema() -> Value {
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
            {
                "name": "Sandbox timeout rate",
                "metric": "runtime.sandbox_timeout_total",
                "visualization": "timeseries",
                "group_by": ["service.name", "service.version", "runtime.worker_id"],
            }
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
            {
                "name": "sandbox-timeout-spike",
                "metric": "runtime.sandbox_timeout_total",
                "severity": "warning",
            }
        ],
    })
}

pub(crate) fn build_runtime_metric_record(payload: Value) -> Result<Value, String> {
    let metric_name = required_non_empty_string(&payload, "metric_name", "runtime metric record")?;
    let spec = runtime_observability_metric_catalog()
        .into_iter()
        .find(|entry| {
            entry.get("metric_name").and_then(Value::as_str) == Some(metric_name.as_str())
        })
        .ok_or_else(|| format!("unsupported runtime metric: {metric_name}"))?;

    let value = payload
        .get("value")
        .cloned()
        .ok_or_else(|| "runtime metric record requires a numeric value".to_string())?;
    let numeric_value = value
        .as_f64()
        .ok_or_else(|| "runtime metric record requires a numeric value".to_string())?;
    if !numeric_value.is_finite() {
        return Err("metric value must be finite".to_string());
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
        .ok_or_else(|| "runtime metric record requires integer field attempt".to_string())?;
    if attempt < 0 {
        return Err(
            "runtime metric record requires non-negative integer field attempt".to_string(),
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

pub(crate) fn normalize_chat_completions_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

pub(crate) fn validate_live_execute_aggregator_base_url(base_url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(base_url).map_err(|err| {
        format!("router-rs live execute requires valid aggregator_base_url: {err}")
    })?;

    if parsed.scheme() != "https" {
        return Err(
            "router-rs live execute requires https aggregator_base_url (http is not allowed)"
                .to_string(),
        );
    }

    let host = parsed.host_str().ok_or_else(|| {
        "router-rs live execute requires aggregator_base_url with a host".to_string()
    })?;

    if host.eq_ignore_ascii_case("localhost") || host.eq_ignore_ascii_case("localhost.") {
        return Err("router-rs live execute blocks localhost aggregator_base_url".to_string());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_forbidden_live_execute_ip(&ip) {
            return Err(format!(
                "router-rs live execute blocks unsafe aggregator_base_url host IP: {host}"
            ));
        }
        return Err(
            "router-rs live execute requires domain-based aggregator_base_url (IP literals are not allowed)"
                .to_string(),
        );
    }

    if let Some(allowlisted_hosts) = parse_execute_aggregator_host_allowlist()? {
        let normalized_host = normalize_allowlisted_host(host);
        if !allowlisted_hosts.contains(&normalized_host) {
            return Err(format!(
                "router-rs live execute blocks aggregator_base_url host not in allowlist: {host}"
            ));
        }
    }

    Ok(())
}

fn is_forbidden_live_execute_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_forbidden_live_execute_ipv4(*ipv4),
        IpAddr::V6(ipv6) => is_forbidden_live_execute_ipv6(*ipv6),
    }
}

fn is_forbidden_live_execute_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.octets()[0] == 127
        || ip.is_unspecified()
}

fn is_forbidden_live_execute_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback() || ip.is_unspecified() || ip.is_unicast_link_local() || ip.is_unique_local()
}

fn truncate_for_error(raw: &str) -> String {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 240 {
        compact
    } else {
        compact.chars().take(237).collect::<String>() + "..."
    }
}

pub(crate) fn extract_chat_completion_content(payload: &Value) -> Result<String, String> {
    let message_content = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .ok_or_else(|| {
            "router-rs live execute response missing choices[0].message.content".to_string()
        })?;

    if let Some(content) = message_content.as_str() {
        return Ok(content.to_string());
    }

    if let Some(parts) = message_content.as_array() {
        let joined = parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
                    .or_else(|| {
                        part.get("content")
                            .and_then(Value::as_str)
                            .map(|value| value.to_string())
                    })
            })
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Ok(joined);
        }
    }

    Err("router-rs live execute response content had an unsupported shape".to_string())
}

fn estimate_tokens(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    trimmed.chars().count().div_ceil(4)
}

fn extract_trace_event_payload(payload: Value) -> Result<Value, String> {
    let event_payload = match payload {
        Value::Object(mut object) => match object.remove("event") {
            Some(Value::Object(event)) => Value::Object(event),
            Some(other) => {
                return Err(format!(
                    "trace stream line contained non-object event wrapper: {other}"
                ))
            }
            None => Value::Object(object),
        },
        other => {
            return Err(format!(
                "trace stream line must decode to a JSON object: {other}"
            ))
        }
    };
    Ok(event_payload)
}

fn trace_event_object(payload: Value) -> Result<Map<String, Value>, String> {
    match extract_trace_event_payload(payload)? {
        Value::Object(object) => Ok(object),
        other => Err(format!(
            "trace stream payload must resolve to a JSON object: {other}"
        )),
    }
}

fn trace_event_string_field(payload: &Map<String, Value>, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn trace_event_usize_field(payload: &Map<String, Value>, field: &str) -> Option<usize> {
    payload
        .get(field)
        .and_then(|value| value.as_u64().map(|number| number as usize))
}

fn build_trace_cursor(generation: usize, seq: usize, event_id: &str) -> String {
    format!("g{generation}:s{seq}:{event_id}")
}

fn hydrate_trace_event_object(
    mut payload: Map<String, Value>,
    line_number: usize,
) -> Map<String, Value> {
    let seq = trace_event_usize_field(&payload, "seq").unwrap_or(line_number);
    let generation = trace_event_usize_field(&payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(&payload, "event_id")
        .unwrap_or_else(|| format!("evt_replay_{line_number:06}"));
    let cursor = trace_event_string_field(&payload, "cursor")
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));

    payload
        .entry("seq".to_string())
        .or_insert_with(|| json!(seq));
    payload
        .entry("generation".to_string())
        .or_insert_with(|| json!(generation));
    payload
        .entry("event_id".to_string())
        .or_insert_with(|| Value::String(event_id));
    payload
        .entry("cursor".to_string())
        .or_insert_with(|| Value::String(cursor));
    payload
        .entry("status".to_string())
        .or_insert_with(|| Value::String("ok".to_string()));
    payload
        .entry("schema_version".to_string())
        .or_insert_with(|| Value::String("runtime-trace-v2".to_string()));
    Value::Object(payload)
        .as_object()
        .cloned()
        .unwrap_or_default()
}

fn trace_event_matches_scope(
    payload: &Map<String, Value>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: Option<&[String]>,
) -> bool {
    let session_scoped = stream_scope_fields
        .map(|fields| fields.iter().any(|field| field == "session_id"))
        .unwrap_or(true);
    let job_scoped = stream_scope_fields
        .map(|fields| fields.iter().any(|field| field == "job_id"))
        .unwrap_or(true);
    if session_scoped {
        if let Some(expected_session_id) = session_id {
            if trace_event_string_field(payload, "session_id").as_deref()
                != Some(expected_session_id)
            {
                return false;
            }
        }
    }
    if job_scoped {
        if let Some(expected_job_id) = job_id {
            if trace_event_string_field(payload, "job_id").as_deref() != Some(expected_job_id) {
                return false;
            }
        }
    }
    true
}

fn trace_scope_fields(payload: &Option<Vec<String>>) -> Option<&[String]> {
    payload.as_deref().filter(|fields| !fields.is_empty())
}

fn trace_event_matches_request_scope(
    payload: &Map<String, Value>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> bool {
    trace_event_matches_scope(
        payload,
        session_id,
        job_id,
        trace_scope_fields(stream_scope_fields),
    )
}

fn load_trace_stream_events(
    path: &Path,
    event_stream_text: Option<&str>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> Result<Vec<Map<String, Value>>, String> {
    let mut events = Vec::new();
    let raw_payload = match event_stream_text {
        Some(value) => value.to_string(),
        None => {
            let storage_backend = resolve_storage_backend(&[path.to_path_buf()]);
            storage_read_text(path, storage_backend.as_ref())?
        }
    };

    for (line_number, raw_line) in raw_payload.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let event_payload = hydrate_trace_event_object(
            trace_event_object(serde_json::from_str::<Value>(raw_line).map_err(|err| {
                format!("parse trace stream line {} failed: {err}", line_number + 1)
            })?)?,
            line_number + 1,
        );
        if trace_event_matches_request_scope(
            &event_payload,
            session_id,
            job_id,
            stream_scope_fields,
        ) {
            events.push(event_payload);
        }
    }
    Ok(events)
}

fn latest_cursor_from_trace_event(payload: &Map<String, Value>) -> Option<Value> {
    let session_id = trace_event_string_field(payload, "session_id")?;
    let seq = trace_event_usize_field(payload, "seq")?;
    let generation = trace_event_usize_field(payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(payload, "event_id")?;
    let cursor = trace_event_string_field(payload, "cursor")
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));
    Some(json!({
        "schema_version": "runtime-trace-cursor-v1",
        "session_id": session_id,
        "job_id": trace_event_string_field(payload, "job_id"),
        "generation": generation,
        "seq": seq,
        "event_id": event_id,
        "cursor": cursor,
    }))
}

fn compaction_delta_to_trace_event(
    payload: Value,
    line_number: usize,
) -> Result<Map<String, Value>, String> {
    let object = payload.as_object().cloned().ok_or_else(|| {
        format!(
            "trace compaction delta line {} must decode to a JSON object",
            line_number
        )
    })?;
    let generation = trace_event_usize_field(&object, "generation").unwrap_or(0);
    let seq = trace_event_usize_field(&object, "seq")
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing seq"))?;
    let applies_to = object
        .get("applies_to")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing applies_to"))?;
    let session_id = applies_to
        .get("session_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!("trace compaction delta line {line_number} missing applies_to.session_id")
        })?;
    let payload_object = object
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing payload"))?;
    let event_id = payload_object
        .get("event_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("evt_replay_{line_number:06}"));
    let cursor = payload_object
        .get("cursor")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));

    let mut event = Map::new();
    event.insert("event_id".to_string(), Value::String(event_id));
    event.insert("seq".to_string(), json!(seq));
    event.insert("generation".to_string(), json!(generation));
    event.insert("cursor".to_string(), Value::String(cursor));
    event.insert(
        "ts".to_string(),
        object
            .get("ts")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    event.insert(
        "session_id".to_string(),
        Value::String(session_id.to_string()),
    );
    event.insert(
        "job_id".to_string(),
        applies_to.get("job_id").cloned().unwrap_or(Value::Null),
    );
    event.insert(
        "kind".to_string(),
        object
            .get("kind")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    event.insert(
        "stage".to_string(),
        payload_object
            .get("stage")
            .cloned()
            .unwrap_or_else(|| Value::String("background".to_string())),
    );
    event.insert(
        "status".to_string(),
        payload_object
            .get("status")
            .cloned()
            .unwrap_or_else(|| Value::String("ok".to_string())),
    );
    event.insert(
        "payload".to_string(),
        payload_object
            .get("payload")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    event.insert(
        "schema_version".to_string(),
        Value::String("runtime-trace-v2".to_string()),
    );
    Ok(event)
}

fn validate_compaction_artifact_digest(
    artifact_ref: &Map<String, Value>,
    payload_text: &str,
    label: &str,
) -> Result<(), String> {
    let expected = artifact_ref
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("compaction {label} artifact ref is missing digest"))?;
    let actual = sha256_hex(payload_text.as_bytes());
    if expected != actual {
        return Err(format!(
            "Compaction recovery failed closed because {label} artifact digest mismatched."
        ));
    }
    Ok(())
}

pub(crate) fn sha256_hex(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    format!("{:x}", hasher.finalize())
}

struct ResolvedTraceSource {
    path: PathBuf,
    source_kind: &'static str,
    events: Vec<Map<String, Value>>,
    latest_cursor: Option<Value>,
    latest_event_id: Option<String>,
    latest_event_kind: Option<String>,
    latest_event_timestamp: Option<String>,
    recovery: Option<Value>,
}

struct TraceSourceRequest<'a> {
    path: Option<&'a str>,
    event_stream_text: Option<&'a str>,
    compaction_manifest_path: Option<&'a str>,
    compaction_manifest_text: Option<&'a str>,
    compaction_state_text: Option<&'a str>,
    compaction_artifact_index_text: Option<&'a str>,
    compaction_delta_text: Option<&'a str>,
    session_id: Option<&'a str>,
    job_id: Option<&'a str>,
    stream_scope_fields: &'a Option<Vec<String>>,
}

struct CompactionRecoveryRequest<'a> {
    manifest_path: &'a Path,
    manifest_text: Option<&'a str>,
    state_text: Option<&'a str>,
    artifact_index_text: Option<&'a str>,
    delta_text: Option<&'a str>,
    session_id: Option<&'a str>,
    job_id: Option<&'a str>,
    stream_scope_fields: &'a Option<Vec<String>>,
}

impl<'a> TraceSourceRequest<'a> {
    fn from_metadata_payload(payload: &'a TraceMetadataWriteRequestPayload) -> Self {
        Self {
            path: payload.event_stream_path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }

    fn from_inspect_payload(payload: &'a TraceStreamInspectRequestPayload) -> Self {
        Self {
            path: payload.path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }

    fn from_replay_payload(payload: &'a TraceStreamReplayRequestPayload) -> Self {
        Self {
            path: payload.path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }
}

fn load_compaction_recovery(
    request: CompactionRecoveryRequest<'_>,
) -> Result<ResolvedTraceSource, String> {
    let CompactionRecoveryRequest {
        manifest_path,
        manifest_text,
        state_text,
        artifact_index_text,
        delta_text,
        session_id,
        job_id,
        stream_scope_fields,
    } = request;
    let storage_backend = resolve_storage_backend(&[manifest_path.to_path_buf()]);
    let manifest_raw = match manifest_text {
        Some(value) => value.to_string(),
        None => storage_read_text(manifest_path, storage_backend.as_ref())?,
    };
    let manifest_payload = serde_json::from_str::<Value>(&manifest_raw).map_err(|err| {
        format!(
            "parse compaction manifest failed for {}: {err}",
            manifest_path.display()
        )
    })?;
    let manifest = manifest_payload.as_object().ok_or_else(|| {
        format!(
            "compaction manifest must decode to a JSON object: {}",
            manifest_path.display()
        )
    })?;
    let snapshot = manifest
        .get("latest_stable_snapshot")
        .and_then(Value::as_object)
        .ok_or_else(|| "compaction manifest is missing latest_stable_snapshot".to_string())?;
    let state_ref = snapshot
        .get("state_ref")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let artifact_index_ref = snapshot
        .get("artifact_index_ref")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let state_ref_uri = state_ref
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let artifact_index_uri = artifact_index_ref
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let state_path = PathBuf::from(state_ref_uri);
    let artifact_index_path = PathBuf::from(artifact_index_uri);
    if state_text.is_none()
        && artifact_index_text.is_none()
        && (!storage_artifact_exists(&state_path, storage_backend.as_ref())
            || !storage_artifact_exists(&artifact_index_path, storage_backend.as_ref()))
    {
        return Err(
            "Compaction recovery failed closed because a referenced artifact is missing."
                .to_string(),
        );
    }
    let state_raw = match state_text {
        Some(value) => value.to_string(),
        None => storage_read_text(&state_path, storage_backend.as_ref())?,
    };
    validate_compaction_artifact_digest(state_ref, &state_raw, "state_ref")?;
    let state_payload = serde_json::from_str::<Value>(&state_raw).map_err(|err| {
        format!(
            "parse compaction state failed for {}: {err}",
            state_path.display()
        )
    })?;
    let artifact_index_raw = match artifact_index_text {
        Some(value) => value.to_string(),
        None => storage_read_text(&artifact_index_path, storage_backend.as_ref())?,
    };
    validate_compaction_artifact_digest(
        artifact_index_ref,
        &artifact_index_raw,
        "artifact_index_ref",
    )?;
    let artifact_index_payload =
        serde_json::from_str::<Value>(&artifact_index_raw).map_err(|err| {
            format!(
                "parse compaction artifact index failed for {}: {err}",
                artifact_index_path.display()
            )
        })?;

    let delta_path = manifest
        .get("delta_path")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let mut deltas = Vec::new();
    let mut events = Vec::new();
    if let Some(delta_path) = delta_path.as_ref() {
        let raw_delta_payload = delta_text.map(str::to_string).or_else(|| {
            if storage_artifact_exists(delta_path, storage_backend.as_ref()) {
                storage_read_text(delta_path, storage_backend.as_ref()).ok()
            } else {
                None
            }
        });
        if let Some(raw_delta_payload) = raw_delta_payload {
            for (line_number, raw_line) in raw_delta_payload.lines().enumerate() {
                if raw_line.trim().is_empty() {
                    continue;
                }
                let delta_payload = serde_json::from_str::<Value>(raw_line).map_err(|err| {
                    format!(
                        "parse compaction delta line {} failed: {err}",
                        line_number + 1
                    )
                })?;
                let event_payload =
                    compaction_delta_to_trace_event(delta_payload.clone(), line_number + 1)?;
                if trace_event_matches_request_scope(
                    &event_payload,
                    session_id,
                    job_id,
                    stream_scope_fields,
                ) {
                    deltas.push(delta_payload);
                    events.push(event_payload);
                }
            }
        }
    }

    let latest_cursor = events
        .last()
        .and_then(latest_cursor_from_trace_event)
        .or_else(|| state_payload.get("latest_cursor").cloned());
    let latest_event = events.last().cloned().or_else(|| {
        state_payload
            .get("latest_event")
            .and_then(Value::as_object)
            .cloned()
    });
    let latest_event_id = latest_cursor
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("event_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            latest_event
                .as_ref()
                .and_then(|payload| trace_event_string_field(payload, "event_id"))
        });
    let latest_event_kind = latest_event
        .as_ref()
        .and_then(|payload| trace_event_string_field(payload, "kind"));
    let latest_event_timestamp = latest_event
        .as_ref()
        .and_then(|payload| trace_event_string_field(payload, "ts"));
    let latest_recoverable_generation = latest_cursor
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("generation"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .or_else(|| {
            manifest
                .get("active_generation")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
        })
        .unwrap_or(0);
    let recovery = Some(json!({
        "schema_version": "runtime-trace-compaction-recovery-v1",
        "session_id": manifest.get("session_id").cloned().unwrap_or(Value::Null),
        "job_id": manifest.get("job_id").cloned().unwrap_or(Value::Null),
        "latest_recoverable_generation": latest_recoverable_generation,
        "snapshot": Value::Object(snapshot.clone()),
        "deltas": deltas,
        "artifact_index": artifact_index_payload,
        "state": state_payload,
        "latest_cursor": latest_cursor.clone(),
    }));
    Ok(ResolvedTraceSource {
        path: manifest_path.to_path_buf(),
        source_kind: "compaction_manifest",
        events,
        latest_cursor,
        latest_event_id,
        latest_event_kind,
        latest_event_timestamp,
        recovery,
    })
}

fn resolve_trace_source(request: TraceSourceRequest<'_>) -> Result<ResolvedTraceSource, String> {
    if request.compaction_manifest_path.is_some() || request.compaction_manifest_text.is_some() {
        let compaction_path = request
            .compaction_manifest_path
            .unwrap_or("<inline-compaction-manifest>");
        return load_compaction_recovery(CompactionRecoveryRequest {
            manifest_path: &PathBuf::from(compaction_path),
            manifest_text: request.compaction_manifest_text,
            state_text: request.compaction_state_text,
            artifact_index_text: request.compaction_artifact_index_text,
            delta_text: request.compaction_delta_text,
            session_id: request.session_id,
            job_id: request.job_id,
            stream_scope_fields: request.stream_scope_fields,
        });
    }
    let path = request
        .path
        .or(if request.event_stream_text.is_some() {
            Some("<inline-trace-stream>")
        } else {
            None
        })
        .ok_or_else(|| {
            "trace stream replay requires path, event_stream_text, compaction_manifest_path, or compaction_manifest_text".to_string()
        })?;
    let path_buf = PathBuf::from(path);
    let events = load_trace_stream_events(
        &path_buf,
        request.event_stream_text,
        request.session_id,
        request.job_id,
        request.stream_scope_fields,
    )?;
    let latest_event = events.last();
    Ok(ResolvedTraceSource {
        path: path_buf,
        source_kind: "trace_stream",
        latest_cursor: latest_event.and_then(latest_cursor_from_trace_event),
        latest_event_id: latest_event
            .and_then(|payload| trace_event_string_field(payload, "event_id")),
        latest_event_kind: latest_event
            .and_then(|payload| trace_event_string_field(payload, "kind")),
        latest_event_timestamp: latest_event
            .and_then(|payload| trace_event_string_field(payload, "ts")),
        recovery: None,
        events,
    })
}

pub(crate) fn inspect_trace_stream(
    payload: TraceStreamInspectRequestPayload,
) -> Result<TraceStreamInspectResponsePayload, String> {
    let resolved = resolve_trace_source(TraceSourceRequest::from_inspect_payload(&payload))?;
    let reroute_count = trace_reroute_count(&resolved.events);
    let retry_count = trace_retry_count(&resolved.events);

    Ok(TraceStreamInspectResponsePayload {
        schema_version: TRACE_STREAM_INSPECT_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: resolved.path.display().to_string(),
        source_kind: resolved.source_kind.to_string(),
        event_count: resolved.events.len(),
        latest_event_id: resolved.latest_event_id,
        latest_event_kind: resolved.latest_event_kind,
        latest_event_timestamp: resolved.latest_event_timestamp,
        latest_cursor: resolved.latest_cursor,
        recovery: resolved.recovery,
        reroute_count,
        retry_count,
    })
}

pub(crate) fn replay_trace_stream(
    payload: TraceStreamReplayRequestPayload,
) -> Result<TraceStreamReplayResponsePayload, String> {
    let resolved = resolve_trace_source(TraceSourceRequest::from_replay_payload(&payload))?;
    let after_event_id = payload.after_event_id.clone();
    let limit = payload.limit.unwrap_or(usize::MAX);
    let mut anchor_found = after_event_id.is_none();
    let mut anchor_index = None;
    let mut next_cursor = None;
    let mut events = Vec::new();

    for (current_index, event_payload) in resolved.events.iter().enumerate() {
        let event_id = trace_event_string_field(event_payload, "event_id");
        if !anchor_found {
            if event_id.as_deref() == after_event_id.as_deref() {
                anchor_found = true;
                anchor_index = Some(current_index);
                continue;
            }
            continue;
        }
        if events.len() >= limit {
            continue;
        }
        next_cursor = Some(TraceStreamReplayCursorPayload {
            event_id: event_id.clone(),
            event_index: current_index,
        });
        events.push(Value::Object(event_payload.clone()));
    }

    if after_event_id.is_some() && !anchor_found {
        return Err(format!(
            "Unknown event id for stream resume: {}",
            after_event_id.unwrap_or_default()
        ));
    }

    let window_start_index = anchor_index.map_or(0, |index| index + 1);
    let has_more = resolved.events.len() > window_start_index + events.len();
    Ok(TraceStreamReplayResponsePayload {
        schema_version: TRACE_STREAM_REPLAY_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: resolved.path.display().to_string(),
        source_kind: resolved.source_kind.to_string(),
        event_count: resolved.events.len(),
        latest_event_id: resolved.latest_event_id,
        latest_event_kind: resolved.latest_event_kind,
        latest_event_timestamp: resolved.latest_event_timestamp,
        latest_cursor: resolved.latest_cursor,
        after_event_id,
        window_start_index,
        has_more,
        next_cursor,
        events,
    })
}

fn trace_reroute_count(events: &[Map<String, Value>]) -> usize {
    events
        .iter()
        .filter(|event| {
            trace_event_string_field(event, "kind").as_deref() == Some("route.selected")
        })
        .count()
        .saturating_sub(1)
}

fn trace_retry_count(events: &[Map<String, Value>]) -> usize {
    events
        .iter()
        .filter(|event| trace_event_string_field(event, "kind").as_deref() == Some("run.failed"))
        .count()
}

fn build_trace_stream_metadata(
    trace: &ResolvedTraceSource,
    control_plane: Option<&Value>,
) -> Value {
    let latest = trace.events.last();
    let mut stream = Map::new();
    stream.insert(
        "generation".to_string(),
        json!(latest
            .and_then(|event| trace_event_usize_field(event, "generation"))
            .unwrap_or(0)),
    );
    stream.insert("replay_supported".to_string(), Value::Bool(true));
    stream.insert("event_stream_supported".to_string(), Value::Bool(true));
    stream.insert(
        "event_stream_schema_version".to_string(),
        Value::String("runtime-event-stream-v1".to_string()),
    );
    if let Some(control_plane) = control_plane.and_then(Value::as_object) {
        let field_map = [
            ("authority", "control_plane_authority"),
            ("role", "control_plane_role"),
            ("projection", "control_plane_projection"),
            ("delegate_kind", "control_plane_delegate_kind"),
            ("ownership_lane", "ownership_lane"),
            ("producer_owner", "producer_owner"),
            ("producer_authority", "producer_authority"),
            ("exporter_owner", "exporter_owner"),
            ("exporter_authority", "exporter_authority"),
            ("transport_family", "transport_family"),
            ("resume_mode", "resume_mode"),
            ("stream_scope_fields", "stream_scope_fields"),
            ("cleanup_scope_fields", "cleanup_scope_fields"),
        ];
        for (source, target) in field_map {
            if let Some(value) = control_plane.get(source) {
                stream.insert(target.to_string(), value.clone());
            }
        }
    }
    stream.insert(
        "event_stream_path".to_string(),
        Value::String(trace.path.display().to_string()),
    );
    stream.insert("compaction_manifest_path".to_string(), Value::Null);
    stream.insert("event_count".to_string(), json!(trace.events.len()));
    stream.insert(
        "latest_seq".to_string(),
        json!(latest
            .and_then(|event| trace_event_usize_field(event, "seq"))
            .unwrap_or(0)),
    );
    stream.insert(
        "latest_event_id".to_string(),
        trace
            .latest_event_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    stream.insert(
        "latest_cursor".to_string(),
        trace.latest_cursor.clone().unwrap_or(Value::Null),
    );
    Value::Object(stream)
}

pub(crate) fn write_trace_compaction_delta(
    payload: TraceCompactionDeltaWriteRequestPayload,
) -> Result<TraceCompactionDeltaWriteResponsePayload, String> {
    let path = PathBuf::from(&payload.path);
    let serialized = serde_json::to_string(&payload.delta)
        .map_err(|err| format!("serialize trace compaction delta failed: {err}"))?
        + "\n";
    append_text_with_process_lock(&path, &serialized, "trace compaction delta")?;
    Ok(TraceCompactionDeltaWriteResponsePayload {
        schema_version: TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: path.display().to_string(),
        bytes_written: serialized.len(),
    })
}
pub(crate) fn dispatch_stdio_json_request_payload(
    request: StdioJsonRequestPayload,
) -> StdioJsonResponsePayload {
    match dispatch_stdio_json_request(&request.op, request.payload) {
        Ok(payload) => StdioJsonResponsePayload {
            id: request.id,
            ok: true,
            payload: Some(payload),
            error: None,
        },
        Err(error) => StdioJsonResponsePayload {
            id: request.id,
            ok: false,
            payload: None,
            error: Some(error),
        },
    }
}

pub(crate) fn dispatch_stdio_json_request(op: &str, payload: Value) -> Result<Value, String> {
    if handles_runtime_output_stdio_op(op) {
        let Some(result) = dispatch_runtime_output_mode_stdio(op, payload) else {
            return Err(format!("runtime output mode dispatch drifted for {op}"));
        };
        return result;
    }
    match classify_stdio_op(op) {
        Some(StdioOpDomain::Routing) => dispatch_routing_stdio_request(op, payload),
        Some(StdioOpDomain::Runtime) => dispatch_runtime_stdio_request(op, payload),
        Some(StdioOpDomain::Trace) => dispatch_trace_stdio_request(op, payload),
        Some(StdioOpDomain::Framework) => dispatch_framework_stdio_request(op, payload),
        None => Err(format!("unsupported stdio operation: {op}")),
    }
}

const ROUTING_STDIO_OPS: &[&str] = &[
    "route",
    "search_skills",
    "hook_policy",
    "concurrency_defaults",
    "route_report",
    "route_resolution",
    "route_policy",
    "route_snapshot",
    "compile_profile_bundle",
    "compile_codex_profile_artifacts",
    "closeout_evaluate",
    "closeout_contract",
    "eval_route",
    "eval_route_contract",
];

const RUNTIME_STDIO_OPS: &[&str] = &[
    "execute",
    "execution_contract_bundle",
    "normalize_execution_kernel_metadata_contract",
    "normalize_execution_kernel_contract",
    "validate_execution_kernel_steady_state_metadata",
    "decode_execution_response",
    "sandbox_control",
    "runtime_observability_health_snapshot",
    "background_control",
    "background_state",
    "session_supervisor",
    "describe_transport",
    "describe_handoff",
    "checkpoint_resume_manifest",
    "runtime_checkpoint_control_plane",
    "write_transport_binding",
    "write_checkpoint_resume_manifest",
    "attach_runtime_event_transport",
    "subscribe_attached_runtime_events",
    "cleanup_attached_runtime_event_transport",
    "runtime_storage",
    "control_plane_contracts",
];

const TRACE_STDIO_OPS: &[&str] = &[
    "trace_record_event",
    "trace_stream_replay",
    "trace_stream_inspect",
    "trace_compact",
    "write_trace_compaction_delta",
    "write_trace_metadata",
];

const FRAMEWORK_STDIO_OPS: &[&str] = &[
    "framework_runtime_snapshot",
    "framework_contract_summary",
    "framework_prompt_compression",
    "framework_session_artifact_write",
    "framework_hook_evidence_append",
    "framework_autopilot_goal",
    "framework_rfv_loop",
    "framework_alias",
    "task_ledger_dispatch",
];

#[derive(Clone, Copy)]
pub(crate) enum StdioOpDomain {
    Routing,
    Runtime,
    Trace,
    Framework,
}

fn op_in_domain(op: &str, domain_ops: &[&str]) -> bool {
    domain_ops.contains(&op)
}

pub(crate) fn classify_stdio_op(op: &str) -> Option<StdioOpDomain> {
    if op_in_domain(op, ROUTING_STDIO_OPS) {
        Some(StdioOpDomain::Routing)
    } else if op_in_domain(op, RUNTIME_STDIO_OPS) {
        Some(StdioOpDomain::Runtime)
    } else if op_in_domain(op, TRACE_STDIO_OPS) {
        Some(StdioOpDomain::Trace)
    } else if op_in_domain(op, FRAMEWORK_STDIO_OPS) {
        Some(StdioOpDomain::Framework)
    } else {
        None
    }
}

#[cfg(test)]
pub(crate) fn is_routing_stdio_op(op: &str) -> bool {
    op_in_domain(op, ROUTING_STDIO_OPS)
}

#[cfg(test)]
pub(crate) fn is_runtime_stdio_op(op: &str) -> bool {
    op_in_domain(op, RUNTIME_STDIO_OPS)
}

#[cfg(test)]
pub(crate) fn is_trace_stdio_op(op: &str) -> bool {
    op_in_domain(op, TRACE_STDIO_OPS)
}

#[cfg(test)]
pub(crate) fn is_framework_stdio_op(op: &str) -> bool {
    op_in_domain(op, FRAMEWORK_STDIO_OPS)
}

fn parse_payload<T: DeserializeOwned>(payload: Value, context: &str) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|err| format!("parse {context} input failed: {err}"))
}

fn serialize_payload<T: Serialize>(value: T, context: &str) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|err| format!("serialize {context} output failed: {err}"))
}

fn parse_and_dispatch<T, R, F>(payload: Value, context: &str, handler: F) -> Result<Value, String>
where
    T: DeserializeOwned,
    R: Serialize,
    F: FnOnce(T) -> Result<R, String>,
{
    let request = parse_payload::<T>(payload, context)?;
    serialize_payload(handler(request)?, context)
}

fn dispatch_stdio_closeout_evaluate(payload: Value) -> Result<Value, String> {
    let repo_root = optional_non_empty_string(&payload, "repo_root");
    let task_id = optional_non_empty_string(&payload, "task_id");
    if let (Some(repo_root), Some(task_id), Some(record_path)) = (
        repo_root.as_deref(),
        task_id.as_deref(),
        optional_non_empty_string(&payload, "record_path").as_deref(),
    ) {
        return framework_runtime::evaluate_closeout_record_file_for_task(
            Path::new(repo_root),
            task_id,
            Path::new(record_path),
        );
    }
    let record_value = payload.get("record").cloned().unwrap_or_else(|| payload.clone());
    if let (Some(repo_root), Some(task_id)) = (repo_root.as_deref(), task_id.as_deref()) {
        let (rows_non_empty, has_success) =
            autopilot_goal::task_evidence_artifacts_summary_for_task(Path::new(repo_root), task_id);
        let ctx = CloseoutEvidenceContext {
            task_id: Some(task_id.trim().to_string()),
            evidence_rows_non_empty: rows_non_empty,
            has_successful_verification: has_success,
        };
        return evaluate_closeout_record_value_with_context(record_value, &ctx);
    }
    evaluate_closeout_record_value(record_value)
}

fn dispatch_routing_stdio_request(op: &str, payload: Value) -> Result<Value, String> {
    match op {
        "route" => dispatch_stdio_route(payload),
        "search_skills" => dispatch_stdio_search_skills(payload),
        "hook_policy" => evaluate_hook_policy_value(payload),
        "concurrency_defaults" => serde_json::to_value(runtime_concurrency_defaults_payload())
            .map_err(|err| format!("serialize concurrency defaults output failed: {err}")),
        "route_report" => dispatch_stdio_route_report(payload),
        "route_resolution" => dispatch_stdio_route_resolution(payload),
        "route_policy" => dispatch_stdio_route_policy(payload),
        "route_snapshot" => dispatch_stdio_route_snapshot(payload),
        "compile_profile_bundle" => dispatch_stdio_compile_profile_bundle(payload),
        "compile_codex_profile_artifacts" => {
            dispatch_stdio_compile_codex_profile_artifacts(payload)
        }
        "closeout_evaluate" => dispatch_stdio_closeout_evaluate(payload),
        "closeout_contract" => Ok(closeout_enforcement_contract()),
        "eval_route" => dispatch_stdio_eval_route(payload),
        "eval_route_contract" => Ok(eval_route_contract()),
        _ => Err(format!("unsupported routing stdio operation: {op}")),
    }
}

fn dispatch_runtime_stdio_request(op: &str, payload: Value) -> Result<Value, String> {
    match op {
        "execute" => {
            parse_and_dispatch::<ExecuteRequestPayload, _, _>(payload, "execute", execute_request)
        }
        "execution_contract_bundle" => Ok(Value::Object(build_execution_contract_bundle())),
        "normalize_execution_kernel_metadata_contract" => {
            if payload.as_object().is_some_and(Map::is_empty) || payload.is_null() {
                normalize_execution_kernel_metadata_contract_value(None)
            } else {
                normalize_execution_kernel_metadata_contract_value(Some(&payload))
            }
        }
        "normalize_execution_kernel_contract" => {
            let kernel_contract = payload.get("kernel_contract").ok_or_else(|| {
                "execution-kernel contract payload is missing kernel_contract.".to_string()
            })?;
            let response_shape = payload.get("response_shape").and_then(Value::as_str);
            normalize_execution_kernel_contract_value(kernel_contract, response_shape)
        }
        "validate_execution_kernel_steady_state_metadata" => {
            let metadata = payload.get("metadata").ok_or_else(|| {
                "execution-kernel validation payload is missing metadata.".to_string()
            })?;
            let kernel_contract = payload.get("kernel_contract");
            let response_shape = payload.get("response_shape").and_then(Value::as_str);
            validate_execution_kernel_steady_state_metadata_value(
                metadata,
                kernel_contract,
                response_shape,
            )
        }
        "decode_execution_response" => {
            let execution_payload = payload.get("payload").ok_or_else(|| {
                "execution response decode payload is missing payload.".to_string()
            })?;
            let kernel_contract = payload.get("kernel_contract");
            let dry_run = payload.get("dry_run").and_then(Value::as_bool);
            decode_execution_response_value(execution_payload, kernel_contract, dry_run)
        }
        "sandbox_control" => parse_and_dispatch::<SandboxControlRequestPayload, _, _>(
            payload,
            "sandbox control",
            build_sandbox_control_response,
        ),
        "runtime_observability_health_snapshot" => {
            serde_json::to_value(build_runtime_observability_health_snapshot()).map_err(|err| {
                format!("serialize runtime observability health snapshot output failed: {err}")
            })
        }
        "background_control" => parse_and_dispatch::<BackgroundControlRequestPayload, _, _>(
            payload,
            "background control",
            build_background_control_response,
        ),
        "background_state" => handle_background_state_operation(payload),
        "session_supervisor" => handle_session_supervisor_operation(payload),
        "describe_transport" => build_trace_transport_descriptor(payload),
        "describe_handoff" => build_trace_handoff_descriptor(payload),
        "checkpoint_resume_manifest" => build_checkpoint_resume_manifest(payload),
        "runtime_checkpoint_control_plane" => {
            build_checkpoint_control_plane_compiler_payload(payload)
        }
        "write_transport_binding" => write_transport_binding_payload(payload),
        "write_checkpoint_resume_manifest" => write_checkpoint_resume_manifest_payload(payload),
        "attach_runtime_event_transport" => attach_runtime_event_transport(payload),
        "subscribe_attached_runtime_events" => subscribe_attached_runtime_events(payload),
        "cleanup_attached_runtime_event_transport" => {
            cleanup_attached_runtime_event_transport(payload)
        }
        "runtime_storage" => parse_and_dispatch::<RuntimeStorageRequestPayload, _, _>(
            payload,
            "runtime storage",
            runtime_storage_operation,
        ),
        "control_plane_contracts" => {
            serde_json::to_value(build_control_plane_contract_descriptors())
                .map_err(|err| format!("serialize control plane contracts output failed: {err}"))
        }
        _ => Err(format!("unsupported runtime stdio operation: {op}")),
    }
}

fn dispatch_trace_stdio_request(op: &str, payload: Value) -> Result<Value, String> {
    match op {
        "trace_record_event" => parse_and_dispatch::<TraceRecordEventRequestPayload, _, _>(
            payload,
            "trace record event",
            record_trace_event,
        ),
        "trace_stream_replay" => parse_and_dispatch::<TraceStreamReplayRequestPayload, _, _>(
            payload,
            "trace stream replay",
            replay_trace_stream,
        ),
        "trace_stream_inspect" => parse_and_dispatch::<TraceStreamInspectRequestPayload, _, _>(
            payload,
            "trace stream inspect",
            inspect_trace_stream,
        ),
        "trace_compact" => parse_and_dispatch::<TraceCompactRequestPayload, _, _>(
            payload,
            "trace compact",
            compact_trace_stream,
        ),
        "write_trace_compaction_delta" => {
            parse_and_dispatch::<TraceCompactionDeltaWriteRequestPayload, _, _>(
                payload,
                "trace compaction delta",
                write_trace_compaction_delta,
            )
        }
        "write_trace_metadata" => parse_and_dispatch::<TraceMetadataWriteRequestPayload, _, _>(
            payload,
            "trace metadata write",
            write_trace_metadata,
        ),
        _ => Err(format!("unsupported trace stdio operation: {op}")),
    }
}

fn dispatch_framework_stdio_request(op: &str, payload: Value) -> Result<Value, String> {
    match op {
        "framework_runtime_snapshot" => dispatch_stdio_framework_runtime_snapshot(payload),
        "framework_contract_summary" => dispatch_stdio_framework_contract_summary(payload),
        "framework_prompt_compression" => build_framework_prompt_compression_envelope(payload),
        "framework_session_artifact_write" => write_framework_session_artifacts(payload),
        "framework_hook_evidence_append" => framework_hook_evidence_append(payload),
        "framework_autopilot_goal" => autopilot_goal::framework_autopilot_goal(payload),
        "framework_rfv_loop" => rfv_loop::framework_rfv_loop(payload),
        "framework_alias" => dispatch_stdio_framework_alias(payload),
        "task_ledger_dispatch" => task_command::dispatch_task_ledger_command_envelope(payload),
        _ => Err(format!("unsupported framework stdio operation: {op}")),
    }
}

fn dispatch_stdio_route(payload: Value) -> Result<Value, String> {
    let query = required_non_empty_string(&payload, "query", "stdio route")?;
    let session_id = optional_non_empty_string(&payload, "session_id")
        .unwrap_or_else(|| "route-cli".to_string());
    let allow_overlay = payload
        .get("allow_overlay")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let first_turn = payload
        .get("first_turn")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let owned_inline_records = if payload.get("skills").is_some() {
        Some(load_inline_records(&payload)?)
    } else {
        None
    };
    let runtime_path = optional_non_empty_string(&payload, "runtime_path").map(PathBuf::from);
    let manifest_path = optional_non_empty_string(&payload, "manifest_path").map(PathBuf::from);
    let host_id = optional_non_empty_string(&payload, "host_id");
    let cached_records;
    let using_inline_records = owned_inline_records.is_some();
    let records: &[SkillRecord] = if let Some(items) = owned_inline_records.as_ref() {
        items.as_slice()
    } else {
        cached_records =
            load_records_cached_for_stdio(runtime_path.as_deref(), manifest_path.as_deref())?;
        cached_records.as_ref()
    };
    let owned_host_records = if host_id.is_some() {
        Some(filter_records_for_host(records.to_vec(), host_id.as_deref())?)
    } else {
        None
    };
    let route_records = if let Some(items) = owned_host_records.as_ref() {
        items.as_slice()
    } else {
        records
    };
    let decision = if using_inline_records {
        route_task(route_records, &query, &session_id, allow_overlay, first_turn)?
    } else {
        route_task_with_manifest_fallback(
            route_records,
            runtime_path.as_deref(),
            manifest_path.as_deref(),
            host_id.as_deref(),
            &query,
            &session_id,
            allow_overlay,
            first_turn,
        )?
    };
    serde_json::to_value(decision).map_err(|err| format!("serialize route output failed: {err}"))
}

fn dispatch_stdio_search_skills(payload: Value) -> Result<Value, String> {
    let query = required_non_empty_string(&payload, "query", "stdio search_skills")?;
    let limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(5);
    let runtime_path = optional_non_empty_string(&payload, "runtime_path").map(PathBuf::from);
    let manifest_path = optional_non_empty_string(&payload, "manifest_path").map(PathBuf::from);
    let host_id = optional_non_empty_string(&payload, "host_id");
    let manifest_fallback =
        manifest_fallback_path(runtime_path.as_deref(), manifest_path.as_deref())?;
    let records = if let Some(path) = manifest_fallback.as_deref() {
        load_records_from_manifest(path)?
    } else {
        load_records_cached_for_stdio(runtime_path.as_deref(), manifest_path.as_deref())?
            .as_ref()
            .clone()
    };
    let records = filter_records_for_host(records, host_id.as_deref())?;
    let matches = search_skills(&records, &query, limit);
    let resolved = build_search_results_payload(&query, matches);
    serde_json::to_value(resolved).map_err(|err| format!("serialize search output failed: {err}"))
}

fn dispatch_stdio_route_report(payload: Value) -> Result<Value, String> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route report")?;
    let route_decision = payload
        .get("route_decision")
        .cloned()
        .filter(|value| !value.is_null())
        .map(serde_json::from_value::<RouteDecision>)
        .transpose()
        .map_err(|err| format!("parse route decision contract failed: {err}"))?;
    let rust_snapshot = match payload.get("rust_route_snapshot").cloned() {
        Some(raw) if !raw.is_null() => serde_json::from_value::<RouteDecisionSnapshotPayload>(raw)
            .map_err(|err| format!("parse rust route snapshot failed: {err}"))?,
        _ => route_decision
            .as_ref()
            .map(|decision| decision.route_snapshot.clone())
            .ok_or_else(|| {
                "route_report requires rust_route_snapshot or route_decision".to_string()
            })?,
    };
    serde_json::to_value(build_route_diff_report(
        &mode,
        rust_snapshot,
        route_decision.as_ref(),
    )?)
    .map_err(|err| format!("serialize route report output failed: {err}"))
}

fn dispatch_stdio_route_resolution(payload: Value) -> Result<Value, String> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route resolution")?;
    let decision_value = payload
        .get("route_decision")
        .cloned()
        .ok_or_else(|| "route_resolution requires route_decision".to_string())?;
    let decision = serde_json::from_value::<RouteDecision>(decision_value)
        .map_err(|err| format!("parse route resolution input failed: {err}"))?;
    serde_json::to_value(build_route_resolution(&mode, &decision)?)
        .map_err(|err| format!("serialize route resolution output failed: {err}"))
}

fn dispatch_stdio_route_policy(payload: Value) -> Result<Value, String> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route policy")?;
    serde_json::to_value(build_route_policy(&mode)?)
        .map_err(|err| format!("serialize route policy output failed: {err}"))
}

fn dispatch_stdio_route_snapshot(payload: Value) -> Result<Value, String> {
    let request = serde_json::from_value::<RouteSnapshotRequestPayload>(payload)
        .map_err(|err| format!("parse route snapshot input failed: {err}"))?;
    let snapshot = build_route_snapshot(
        &request.engine,
        &request.selected_skill,
        request.overlay_skill.as_deref(),
        &request.layer,
        request.score,
        &request.reasons,
    );
    serde_json::to_value(RouteSnapshotEnvelopePayload {
        snapshot_schema_version: ROUTE_SNAPSHOT_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        route_snapshot: snapshot,
    })
    .map_err(|err| format!("serialize route snapshot output failed: {err}"))
}

fn dispatch_stdio_framework_runtime_snapshot(payload: Value) -> Result<Value, String> {
    let repo_root =
        required_non_empty_string(&payload, "repo_root", "stdio framework runtime snapshot")?;
    let repo_root = resolve_repo_root_arg(Some(Path::new(&repo_root)))?;
    let artifact_root = optional_non_empty_string(&payload, "artifact_source_dir");
    let task_id = optional_non_empty_string(&payload, "task_id");
    serde_json::to_value(build_framework_runtime_snapshot_envelope(
        repo_root.as_path(),
        artifact_root.as_deref().map(Path::new),
        task_id.as_deref(),
    )?)
    .map_err(|err| format!("serialize framework runtime snapshot output failed: {err}"))
}

fn dispatch_stdio_framework_contract_summary(payload: Value) -> Result<Value, String> {
    let repo_root =
        required_non_empty_string(&payload, "repo_root", "stdio framework contract summary")?;
    let repo_root = resolve_repo_root_arg(Some(Path::new(&repo_root)))?;
    serde_json::to_value(build_framework_contract_summary_envelope(
        repo_root.as_path(),
    )?)
    .map_err(|err| format!("serialize framework contract summary output failed: {err}"))
}

fn dispatch_stdio_framework_alias(payload: Value) -> Result<Value, String> {
    let repo_root = required_non_empty_string(&payload, "repo_root", "stdio framework alias")?;
    let repo_root = resolve_repo_root_arg(Some(Path::new(&repo_root)))?;
    let alias_name = required_non_empty_string(&payload, "alias", "stdio framework alias")?;
    let max_lines = payload
        .get("max_lines")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(4);
    let compact = payload
        .get("compact")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let host_id = payload.get("host_id").and_then(Value::as_str);
    serde_json::to_value(build_framework_alias_envelope(
        repo_root.as_path(),
        &alias_name,
        FrameworkAliasBuildOptions {
            max_lines,
            compact,
            host_id,
        },
    )?)
    .map_err(|err| format!("serialize framework alias output failed: {err}"))
}

fn dispatch_stdio_compile_profile_bundle(payload: Value) -> Result<Value, String> {
    let profile_path = required_non_empty_string(&payload, "profile_path", "stdio profile bundle")?;
    let profile = load_framework_profile(Path::new(&profile_path))?;
    let bundle = build_profile_bundle(&profile)?;
    serde_json::to_value(bundle)
        .map_err(|err| format!("serialize profile bundle output failed: {err}"))
}

fn dispatch_stdio_compile_codex_profile_artifacts(payload: Value) -> Result<Value, String> {
    let profile_path =
        required_non_empty_string(&payload, "profile_path", "stdio codex profile artifacts")?;
    let full = optional_bool(&payload, "full").unwrap_or(false);
    let profile = load_framework_profile(Path::new(&profile_path))?;
    let artifacts = build_codex_artifact_bundle(&profile, full)?;
    serde_json::to_value(artifacts)
        .map_err(|err| format!("serialize codex profile artifacts output failed: {err}"))
}

fn dispatch_stdio_eval_route(payload: Value) -> Result<Value, String> {
    let cases_path = required_non_empty_string(&payload, "cases", "stdio eval route")?;
    let runtime = optional_non_empty_string(&payload, "runtime");
    let manifest = optional_non_empty_string(&payload, "manifest");
    let report = run_eval_route(
        Path::new(&cases_path),
        runtime.as_deref().map(Path::new),
        manifest.as_deref().map(Path::new),
    )?;
    serde_json::to_value(report).map_err(|err| format!("serialize eval route report failed: {err}"))
}


fn default_trace_metadata_schema_version() -> String {
    "trace-metadata-v2".to_string()
}


fn default_trace_framework_version() -> String {
    "phase1".to_string()
}

fn timestamp_now() -> String {
    Utc::now().to_rfc3339()
}

fn default_trace_runtime_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("skills")
        .join("SKILL_ROUTING_RUNTIME.json")
}

fn load_trace_routing_runtime_version(runtime_path: Option<&str>) -> u64 {
    let resolved_path = runtime_path
        .map(PathBuf::from)
        .unwrap_or_else(default_trace_runtime_path);
    let raw = match fs::read_to_string(&resolved_path) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(payload)) => payload.get("version").and_then(Value::as_u64).unwrap_or(1),
        _ => 1,
    }
}

fn trace_source_explicitly_provided(payload: &TraceMetadataWriteRequestPayload) -> bool {
    payload.event_stream_path.is_some()
        || payload.event_stream_text.is_some()
        || payload.compaction_manifest_path.is_some()
        || payload.compaction_manifest_text.is_some()
}

pub(crate) fn write_trace_metadata(
    payload: TraceMetadataWriteRequestPayload,
) -> Result<TraceMetadataWriteResponsePayload, String> {
    let routing_runtime_version = payload
        .routing_runtime_version
        .unwrap_or_else(|| load_trace_routing_runtime_version(payload.runtime_path.as_deref()));
    let metadata_schema_version = payload
        .metadata_schema_version
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(default_trace_metadata_schema_version);
    let framework_version = payload
        .framework_version
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(default_trace_framework_version);
    let timestamp = payload.ts.clone().unwrap_or_else(timestamp_now);
    let should_resolve_trace = payload.events.is_none()
        || payload.stream.is_none()
        || payload.reroute_count.is_none()
        || payload.retry_count.is_none();
    let resolved_trace = if trace_source_explicitly_provided(&payload) {
        Some(resolve_trace_source(
            TraceSourceRequest::from_metadata_payload(&payload),
        )?)
    } else if should_resolve_trace {
        resolve_trace_source(TraceSourceRequest::from_metadata_payload(&payload)).ok()
    } else {
        None
    };
    let resolved_events = payload.events.clone().unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| {
                trace
                    .events
                    .iter()
                    .cloned()
                    .map(Value::Object)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let reroute_count = payload.reroute_count.unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| trace_reroute_count(&trace.events))
            .unwrap_or(0)
    });
    let retry_count = payload.retry_count.unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| trace_retry_count(&trace.events))
            .unwrap_or(0)
    });
    let resolved_stream = payload.stream.clone().or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| build_trace_stream_metadata(trace, payload.control_plane.as_ref()))
    });

    let mut document = Map::new();
    document.insert("version".to_string(), json!(1));
    document.insert(
        "schema_version".to_string(),
        json!(metadata_schema_version.clone()),
    );
    document.insert(
        "metadata_schema_version".to_string(),
        json!(metadata_schema_version),
    );
    document.insert("ts".to_string(), json!(timestamp));
    document.insert("task".to_string(), json!(payload.task));
    document.insert("framework_version".to_string(), json!(framework_version));
    document.insert(
        "routing_runtime_version".to_string(),
        json!(routing_runtime_version),
    );
    document.insert("matched_skills".to_string(), json!(payload.matched_skills));
    document.insert(
        "decision".to_string(),
        json!({
            "owner": payload.owner,
            "gate": payload.gate,
            "overlay": payload.overlay,
        }),
    );
    document.insert("reroute_count".to_string(), json!(reroute_count));
    document.insert("retry_count".to_string(), json!(retry_count));
    document.insert("artifact_paths".to_string(), json!(payload.artifact_paths));
    document.insert(
        "verification_status".to_string(),
        json!(payload.verification_status),
    );
    if let Some(value) = payload.trace_event_schema_version {
        document.insert("trace_event_schema_version".to_string(), json!(value));
    }
    if let Some(value) = payload.trace_event_sink_schema_version {
        document.insert("trace_event_sink_schema_version".to_string(), json!(value));
    }
    if let Some(value) = payload.parallel_group {
        document.insert("parallel_group".to_string(), value);
    }
    if let Some(value) = payload.supervisor_projection {
        document.insert("supervisor_projection".to_string(), value);
    }
    if let Some(value) = payload.control_plane {
        document.insert("control_plane".to_string(), value);
    }
    if let Some(value) = resolved_stream {
        document.insert("stream".to_string(), value);
    }
    if !resolved_events.is_empty() {
        document.insert("events".to_string(), Value::Array(resolved_events));
    }

    let serialized = serde_json::to_string_pretty(&Value::Object(document))
        .map_err(|err| format!("serialize trace metadata failed: {err}"))?
        + "\n";
    if payload.write_outputs {
        let outputs =
            std::iter::once(payload.output_path.clone()).chain(payload.mirror_paths.clone());
        for output in outputs {
            let path = PathBuf::from(&output);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    format!(
                        "create trace metadata parent failed for {}: {err}",
                        parent.display()
                    )
                })?;
            }
            fs::write(&path, &serialized).map_err(|err| {
                format!("write trace metadata failed for {}: {err}", path.display())
            })?;
        }
    }

    Ok(TraceMetadataWriteResponsePayload {
        schema_version: TRACE_METADATA_WRITE_SCHEMA_VERSION.to_string(),
        authority: TRACE_METADATA_WRITE_AUTHORITY.to_string(),
        output_path: payload.output_path,
        mirror_paths: payload.mirror_paths,
        bytes_written: serialized.len(),
        routing_runtime_version,
        payload_text: serialized,
    })
}
