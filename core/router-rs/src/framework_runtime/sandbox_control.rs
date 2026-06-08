//! Sandbox 控制面状态机（Roadmap v5 P7：自 `cli/runtime_ops.inc` 下沉至 B3）。

use chrono::Utc;
use serde_json::json;
use std::path::PathBuf;

use crate::cli::common::append_text_with_process_lock;
use crate::runtime_envelope_ids::{
    SANDBOX_CONTROL_AUTHORITY, SANDBOX_CONTROL_SCHEMA_VERSION, SANDBOX_EVENT_SCHEMA_VERSION,
};

use crate::stdio_payload_types::{SandboxControlRequestPayload, SandboxControlResponsePayload};

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

struct SandboxResponseBuilder<'a> {
    request: &'a SandboxControlRequestPayload,
    current_state: Option<String>,
    next_state: Option<String>,
    allowed: bool,
    resolved_state: Option<String>,
    reason: String,
    error: Option<String>,
    failure_reason: Option<String>,
    budget_violation: Option<String>,
    cleanup_required: Option<bool>,
    quarantined: Option<bool>,
    effective_capabilities: Option<Vec<String>>,
    event_kind: Option<String>,
}

impl<'a> SandboxResponseBuilder<'a> {
    fn new(request: &'a SandboxControlRequestPayload, allowed: bool, reason: &str) -> Self {
        Self {
            request,
            current_state: None,
            next_state: None,
            allowed,
            resolved_state: None,
            reason: reason.to_string(),
            error: None,
            failure_reason: None,
            budget_violation: None,
            cleanup_required: None,
            quarantined: None,
            effective_capabilities: None,
            event_kind: None,
        }
    }

    fn current_state(mut self, v: Option<String>) -> Self {
        self.current_state = v;
        self
    }

    fn next_state(mut self, v: Option<String>) -> Self {
        self.next_state = v;
        self
    }

    fn resolved_state(mut self, v: Option<String>) -> Self {
        self.resolved_state = v;
        self
    }

    fn error(mut self, v: Option<String>) -> Self {
        self.error = v;
        self
    }

    fn failure_reason(mut self, v: Option<String>) -> Self {
        self.failure_reason = v;
        self
    }

    fn budget_violation(mut self, v: Option<String>) -> Self {
        self.budget_violation = v;
        self
    }

    fn cleanup_required(mut self, v: Option<bool>) -> Self {
        self.cleanup_required = v;
        self
    }

    fn quarantined(mut self, v: Option<bool>) -> Self {
        self.quarantined = v;
        self
    }

    fn effective_capabilities(mut self, v: Option<Vec<String>>) -> Self {
        self.effective_capabilities = v;
        self
    }

    fn event_kind(mut self, v: Option<&str>) -> Self {
        self.event_kind = v.map(str::to_string);
        self
    }

    fn build(self) -> SandboxControlResponsePayload {
        SandboxControlResponsePayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            authority: SANDBOX_CONTROL_AUTHORITY.to_string(),
            operation: self.request.operation.clone(),
            current_state: self.current_state,
            next_state: self.next_state,
            allowed: self.allowed,
            resolved_state: self.resolved_state,
            reason: self.reason,
            error: self.error,
            failure_reason: self.failure_reason,
            budget_violation: self.budget_violation,
            cleanup_required: self.cleanup_required,
            quarantined: self.quarantined,
            effective_capabilities: self.effective_capabilities,
            sandbox_id: self.request.sandbox_id.clone(),
            profile_id: self.request.profile_id.clone(),
            event_schema_version: Some(SANDBOX_EVENT_SCHEMA_VERSION.to_string()),
            event_log_path: self.request.event_log_path.clone(),
            event_written: false,
            event_kind: self.event_kind,
        }
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
            SandboxResponseBuilder::new(&payload, allowed, if allowed {
                "transition-accepted"
            } else {
                "invalid-transition"
            })
            .current_state(Some(current_state.clone()))
            .next_state(Some(next_state.clone()))
            .resolved_state(Some(next_state))
            .error(if allowed {
                None
            } else {
                Some(format!(
                    "invalid sandbox transition: {:?} -> {:?}",
                    current_state, payload.next_state.as_deref().unwrap_or("")
                ))
            })
            .effective_capabilities(payload.capability_categories.clone())
            .build()
        }
        "cleanup" => {
            let current_state = payload
                .current_state
                .clone()
                .unwrap_or_else(|| "draining".to_string());
            let cleanup_failed = payload.cleanup_failed.unwrap_or(false);
            let resolved_state = if cleanup_failed { "failed" } else { "recycled" };
            let allowed = matches!(current_state.as_str(), "draining");
            SandboxResponseBuilder::new(
                &payload,
                allowed,
                if !allowed {
                    "cleanup-invalid-state"
                } else if cleanup_failed {
                    "cleanup-failed"
                } else {
                    "cleanup-completed"
                },
            )
            .current_state(Some(current_state.clone()))
            .next_state(Some(resolved_state.to_string()))
            .resolved_state(Some(resolved_state.to_string()))
            .error(if allowed {
                None
            } else {
                Some(format!(
                    "invalid sandbox cleanup state: {:?} -> {:?}",
                    current_state, resolved_state
                ))
            })
            .failure_reason(
                payload
                    .error_kind
                    .clone()
                    .or_else(|| cleanup_failed.then(|| "cleanup_failed".to_string())),
            )
            .cleanup_required(Some(false))
            .quarantined(Some(cleanup_failed))
            .effective_capabilities(payload.capability_categories.clone())
            .event_kind(Some(if cleanup_failed {
                "sandbox.cleanup_failed"
            } else {
                "sandbox.cleanup_completed"
            }))
            .build()
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
                SandboxResponseBuilder::new(&payload, false, "admission-rejected")
                    .current_state(Some(current_state))
                    .next_state(Some("failed".to_string()))
                    .resolved_state(Some("failed".to_string()))
                    .error(Some(reason.clone()))
                    .failure_reason(Some(reason))
                    .cleanup_required(Some(false))
                    .quarantined(Some(true))
                    .effective_capabilities(Some(categories))
                    .event_kind(Some("sandbox.failed"))
                    .build()
            } else {
                SandboxResponseBuilder::new(&payload, true, "admission-accepted")
                    .current_state(Some(current_state))
                    .next_state(Some("busy".to_string()))
                    .resolved_state(Some("busy".to_string()))
                    .cleanup_required(Some(false))
                    .quarantined(Some(false))
                    .effective_capabilities(Some(categories))
                    .event_kind(Some("sandbox.execution_started"))
                    .build()
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
                SandboxResponseBuilder::new(
                    &payload,
                    sandbox_transition_allowed(&current_state, resolved_state),
                    if resolved_state == "draining" {
                        "execution-timeout"
                    } else {
                        "execution-failed"
                    },
                )
                .current_state(Some(current_state))
                .next_state(Some(resolved_state.to_string()))
                .resolved_state(Some(resolved_state.to_string()))
                .error(Some(reason.clone()))
                .failure_reason(Some(reason))
                .cleanup_required(Some(resolved_state == "draining"))
                .quarantined(Some(resolved_state == "failed"))
                .effective_capabilities(payload.capability_categories.clone())
                .event_kind(Some(if resolved_state == "draining" {
                    "sandbox.timeout"
                } else {
                    "sandbox.failed"
                }))
                .build()
            } else if let Some(violation) = budget_violation {
                SandboxResponseBuilder::new(
                    &payload,
                    sandbox_transition_allowed(&current_state, "draining"),
                    "budget-exceeded",
                )
                .current_state(Some(current_state))
                .next_state(Some("draining".to_string()))
                .resolved_state(Some("draining".to_string()))
                .error(Some(violation.clone()))
                .failure_reason(Some(violation.clone()))
                .budget_violation(Some(violation))
                .cleanup_required(Some(true))
                .quarantined(Some(false))
                .effective_capabilities(payload.capability_categories.clone())
                .event_kind(Some("sandbox.budget_exceeded"))
                .build()
            } else {
                SandboxResponseBuilder::new(
                    &payload,
                    sandbox_transition_allowed(&current_state, "draining"),
                    "execution-completed",
                )
                .current_state(Some(current_state))
                .next_state(Some("draining".to_string()))
                .resolved_state(Some("draining".to_string()))
                .cleanup_required(Some(true))
                .quarantined(Some(false))
                .effective_capabilities(payload.capability_categories.clone())
                .event_kind(Some("sandbox.execution_completed"))
                .build()
            }
        }
        other => return Err(format!("unsupported sandbox control operation: {other}")),
    };
    maybe_record_sandbox_event(&mut response, &payload)?;
    Ok(response)
}
