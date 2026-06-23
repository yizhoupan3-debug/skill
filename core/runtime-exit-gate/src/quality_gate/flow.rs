//! 主入口与操作分发：`framework_quality_gate` 入口点 + status/start/append_round 处理。

use super::*;

/// stdio：`framework_quality_gate`
pub fn framework_quality_gate(payload: Value) -> Result<Value, String> {
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    if operation == "status" {
        framework_quality_gate_impl(payload)
    } else {
        let resolved = resolve_framework_quality_gate_repo(&payload)?;
        core_state::utils::task_write_lock::apply_task_ledger_mutation(&resolved, || {
            framework_quality_gate_impl(payload)
        })
    }
}

fn handle_status(repo_root: &Path, task_id_override: Option<&str>) -> Result<Value, String> {
    let state = read_quality_gate_state(repo_root, task_id_override)?;
    let tid = task_id_override
        .map(|s| s.to_string())
        .or_else(|| core_state::state_manager::read_primary_task_id(repo_root))
        .unwrap_or_default();
    let path = if tid.is_empty() {
        PathBuf::new()
    } else {
        quality_gate_state_path(repo_root, &tid).unwrap_or_else(|_| PathBuf::new())
    };
    let mut resp = json!({
        "ok": true,
        "operation": "status",
        "task_id": tid,
        "quality_gate_state_path": path.display().to_string(),
    });
    if let Some(ref st) = state {
        merge_operator_nudge_refs(&mut resp, repo_root, Some(st));
    }
    if let Some(st) = state {
        resp["quality_gate_state"] = st;
    }
    Ok(resp)
}

fn handle_start_upsert(payload: &Value, repo_root: &Path, task_id_override: Option<&str>) -> Result<Value, String> {
    let task_id = task_id_override
        .map(|s| s.to_string())
        .or_else(|| core_state::state_manager::read_primary_task_id(repo_root))
        .ok_or_else(|| {
        "framework_quality_gate start requires task_id in payload or TASK_POINTERS.json"
            .to_string()
    })?;
    core_state::utils::path_guard::validate_task_id_component(&task_id)?;
    let goal = payload
        .get("goal")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "framework_quality_gate start requires non-empty goal".to_string())?;
    let requested_max = payload
        .get("max_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(3);
    let (max_rounds, capped) = clamp_max_rounds(requested_max);
    let allow_external = payload
        .get("allow_external_research")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let parallel_external = payload
        .get("parallel_external_with_review")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    // When external research is allowed, default structured blob preference to true so
    // strict-mode validators and struct-hint nudges align; explicit `false` still wins.
    // When `allow_external_research` is false, keep legacy default `false` unless the
    // caller explicitly sets a bool (tests / forward-compat).
    let prefer_structured_external = if allow_external {
        match payload.get("prefer_structured_external_research") {
            None => true,
            Some(v) if v.is_null() => true,
            Some(v) => v.as_bool().unwrap_or(false),
        }
    } else {
        payload
            .get("prefer_structured_external_research")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    // NOTE: `external_research_strict` defaults to true (enforce structured ER quality),
    // while `prefer_structured_external_research` defaults to false (loose recommendation).
    // These have different design intents despite similar naming.
    let external_research_strict = payload
        .get("external_research_strict")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let mut obj = Map::new();
    obj.insert("schema_version".to_string(), json!(QUALITY_GATE_LOOP_SCHEMA_VERSION));
    obj.insert("goal".to_string(), json!(goal));
    obj.insert("max_rounds".to_string(), json!(max_rounds));
    obj.insert("max_rounds_requested".to_string(), json!(requested_max));
    obj.insert("max_rounds_capped".to_string(), json!(capped));
    obj.insert("allow_external_research".to_string(), json!(allow_external));
    obj.insert(
        "parallel_external_with_review".to_string(),
        json!(parallel_external),
    );
    obj.insert(
        "prefer_structured_external_research".to_string(),
        json!(prefer_structured_external),
    );
    obj.insert(
        "external_research_strict".to_string(),
        json!(external_research_strict),
    );
    obj.insert(
        "review_scope".to_string(),
        json!(
            payload
                .get("review_scope")
                .and_then(Value::as_str)
                .unwrap_or("")
        ),
    );
    obj.insert(
        "fix_scope".to_string(),
        json!(
            payload
                .get("fix_scope")
                .and_then(Value::as_str)
                .unwrap_or("")
        ),
    );
    obj.insert(
        "verify_commands".to_string(),
        Value::Array(value_string_list(payload, "verify_commands")),
    );
    obj.insert(
        "stop_when".to_string(),
        Value::Array(value_string_list(payload, "stop_when")),
    );
    // Convergence floor: min_rounds prevents supervisor from closing too early;
    // consecutive_stable_required tracks how many clean rounds are needed.
    let min_rounds = payload
        .get("min_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let consecutive_stable_required = payload
        .get("consecutive_stable_required")
        .and_then(Value::as_u64)
        .unwrap_or(2);
    obj.insert("min_rounds".to_string(), json!(min_rounds));
    obj.insert("consecutive_stable_required".to_string(), json!(consecutive_stable_required));
    obj.insert("consecutive_stable_count".to_string(), json!(0u64));
    obj.insert("loop_status".to_string(), json!("active"));
    obj.insert("current_round".to_string(), json!(0));
    obj.insert("rounds".to_string(), json!([]));
    obj.insert("updated_at".to_string(), json!(now_iso()));
    if let Some(extra) = payload.get("metadata").cloned() {
        obj.insert("metadata".to_string(), extra);
    }
    if let Some(cg) = payload.get("close_gates")
        && !cg.is_null() {
            obj.insert("close_gates".to_string(), cg.clone());
        }

    let path = quality_gate_state_path(repo_root, &task_id)?;
    let value = Value::Object(obj);
    write_atomic_json(&path, &value)?;
    let tx = core_state::task_ledger::LedgerTransaction {
        ts: framework_kernel::time::now_iso(),
        tx_type: "quality_gate_state".to_string(),
        payload: value.clone(),
        idempotency_key: None,
        seq: None,
        schema_version: Some(1),
    };
    if let Err(e) =
        core_state::task_ledger::append_transaction_assuming_l1_held(repo_root, &task_id, tx)
    {
        tracing::error!(task_id = %task_id, error = %e, "failed to append quality_gate transaction to TASK_LEDGER");
    }
    let goal_state_cleared =
        core_state::state_manager::deactivate_goal_for_conflict_with_quality_gate(repo_root, &task_id)?;
    core_state::task_state_aggregate::sync_task_state_aggregate_best_effort(
        repo_root, &task_id,
    );
    let mut resp = json!({
        "ok": true,
        "operation": "start",
        "task_id": task_id,
        "quality_gate_state_path": path.display().to_string(),
        "goal_state_cleared": goal_state_cleared,
        "warning": if capped {
            Some(format!(
                "max_rounds requested {requested_max} exceeds hard cap {}; stored max_rounds={max_rounds}",
                framework_runtime::router_env_flags::router_rs_qg_max_rounds_cap()
            ))
        } else {
            None
        },
    });
    merge_operator_nudge_refs(&mut resp, repo_root, Some(&value));
    resp["quality_gate_state"] = value;
    Ok(resp)
}

fn handle_append_round(payload: &Value, repo_root: &Path) -> Result<Value, String> {
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| "framework_quality_gate append_round requires task_id".to_string())?;
    core_state::utils::path_guard::validate_task_id_component(&task_id)?;
    let path = quality_gate_state_path(repo_root, &task_id)?;
    let mut state = read_quality_gate_state(repo_root, Some(&task_id))?
        .ok_or_else(|| format!("QUALITY_GATE_STATE missing at {}", path.display()))?;

    let round_n = payload
        .get("round")
        .and_then(Value::as_u64)
        .ok_or_else(|| "append_round requires round (u64)".to_string())?;

    let obj = state
        .as_object_mut()
        .ok_or_else(|| "QUALITY_GATE_STATE root must be object".to_string())?;
    let max_rounds = obj
        .get("max_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(framework_runtime::router_env_flags::router_rs_qg_max_rounds_cap());
    if round_n > max_rounds {
        return Err(format!("round {round_n} exceeds max_rounds {max_rounds}"));
    }

    let close_gates_cfg = parse_close_gates(obj);

    // Convergence floor parameters (set during start, read here for enforcement)
    let min_rounds = obj.get("min_rounds").and_then(Value::as_u64).unwrap_or(0);
    let consecutive_stable_required = obj
        .get("consecutive_stable_required")
        .and_then(Value::as_u64)
        .unwrap_or(2);

    let review_summary = payload
        .get("review_summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let external_research_summary = payload
        .get("external_research_summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let fix_summary = payload
        .get("fix_summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let raw_verify = payload
        .get("verify_result")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN");
    let verify_result = normalize_verify_result(raw_verify)?;
    let supervisor_decision = payload
        .get("supervisor_decision")
        .and_then(Value::as_str)
        .unwrap_or("continue")
        .to_ascii_lowercase();
    let reason = payload
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Optional "adversarial depth" fields: stored as-is (array) for audit; no new state machine.
    let adversarial_findings = value_array_or_empty(payload, "adversarial_findings")?;
    let falsification_tests = value_array_or_empty(payload, "falsification_tests")?;

    // Convergence floor: track consecutive stable rounds (no new A/B/P0 findings).
    let round_has_ab = has_ab_level_findings(&adversarial_findings);
    let prev_stable = obj
        .get("consecutive_stable_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let new_stable = if round_has_ab { 0 } else { prev_stable + 1 };
    obj.insert("consecutive_stable_count".to_string(), json!(new_stable));

    // Effective close: supervisor close is blocked until min_rounds reached
    // AND consecutive_stable_required met. max_rounds is unconditional.
    let supervisor_closes = matches!(supervisor_decision.as_str(), "close" | "closed");
    let effective_close = if round_n >= max_rounds {
        true // hard ceiling: always close
    } else if supervisor_closes {
        round_n >= min_rounds && new_stable >= consecutive_stable_required
    } else {
        false
    };

    let external_research_strict = external_research_strict_from_loaded_state(obj);
    if let Some(er) = payload.get("external_research")
        && !er.is_null() {
            validate_external_research_structured(er)?;
            if external_research_strict {
                validate_external_research_strict(er)?;
            }
        }

    // Cross-link this round's verify claim against EVIDENCE_INDEX successful rows
    // recorded since the previous round (audit trail; not a hard block).
    let (evidence_refs, cross_check_label) =
        cross_link_evidence(repo_root, &task_id, obj, &verify_result);

    let mut entry_map = serde_json::Map::new();
    entry_map.insert("round".to_string(), json!(round_n));
    entry_map.insert("review_summary".to_string(), json!(review_summary));
    entry_map.insert(
        "external_research_summary".to_string(),
        json!(external_research_summary),
    );
    entry_map.insert("fix_summary".to_string(), json!(fix_summary));
    entry_map.insert("verify_result".to_string(), json!(verify_result));
    entry_map.insert(
        "supervisor_decision".to_string(),
        json!(supervisor_decision),
    );
    entry_map.insert("reason".to_string(), json!(reason));
    entry_map.insert("at".to_string(), json!(now_iso()));
    entry_map.insert("evidence_refs".to_string(), Value::Array(evidence_refs));
    if !adversarial_findings.is_empty() {
        entry_map.insert(
            "adversarial_findings".to_string(),
            Value::Array(adversarial_findings),
        );
    }
    if !falsification_tests.is_empty() {
        entry_map.insert(
            "falsification_tests".to_string(),
            Value::Array(falsification_tests),
        );
    }
    if let Some(label) = cross_check_label {
        entry_map.insert("cross_check".to_string(), json!(label));
    }
    if let Some(er) = payload.get("external_research")
        && !er.is_null() {
            entry_map.insert("external_research".to_string(), er.clone());
        }
    let entry = Value::Object(entry_map);

    // Push entry now so preview and final state share the same entry.
    // If gates reject, we undo the push before returning Err.
    {
        let rounds = obj
            .get_mut("rounds")
            .and_then(|r| r.as_array_mut())
            .ok_or_else(|| "QUALITY_GATE_STATE.rounds missing".to_string())?;
        rounds.push(entry);
    }

    // Enforce close_gates only when effective_close is true (supervisor close
    // approved by min_rounds + convergence floor, or max_rounds hard ceiling).
    if effective_close
        && let Some(ref g) = close_gates_cfg {
            let closing = obj
                .get("rounds")
                .and_then(|r| r.as_array())
                .and_then(|a| a.last())
                .and_then(|v| v.as_object())
                .ok_or_else(|| {
                    "RFV close_gates: internal error resolving closing round".to_string()
                })?;
            if let Err(e) = enforce_rfv_close_gates(repo_root, &task_id, obj, closing, g) {
                obj.get_mut("rounds")
                    .and_then(|r| r.as_array_mut())
                    .map(|a| a.pop());
                return Err(e);
            }
        }

    let closes_due_to_round_cap = !effective_close
        && !matches!(supervisor_decision.as_str(), "block" | "blocked")
        && round_n >= max_rounds;
    if closes_due_to_round_cap
        && let Some(ref g) = close_gates_cfg {
            let closing = obj
                .get("rounds")
                .and_then(|r| r.as_array())
                .and_then(|a| a.last())
                .and_then(|v| v.as_object())
                .ok_or_else(|| {
                    "RFV close_gates: internal error resolving closing round (max_rounds)"
                        .to_string()
                })?;
            if let Err(e) = enforce_rfv_close_gates(repo_root, &task_id, obj, closing, g) {
                obj.get_mut("rounds")
                    .and_then(|r| r.as_array_mut())
                    .map(|a| a.pop());
                return Err(e);
            }
        }

    obj.insert("current_round".to_string(), json!(round_n));
    obj.insert("updated_at".to_string(), json!(now_iso()));

    // Convergence-aware loop status: supervisor close is gated by min_rounds
    // AND consecutive_stable_required. max_rounds is an unconditional hard ceiling.
    let loop_status = if effective_close {
        "closed"
    } else if matches!(supervisor_decision.as_str(), "block" | "blocked") {
        "blocked"
    } else if round_n >= max_rounds {
        "closed"
    } else {
        "active"
    };
    obj.insert("loop_status".to_string(), json!(loop_status));

    let round_cap_warning = if round_n >= max_rounds
        && close_gates_cfg.is_none()
        && !effective_close
    {
        Some(
            "Quality Gate loop reached max_rounds without close_gates configuration; closing without gate verification. Consider configuring close_gates for future loops.",
        )
    } else {
        None
    };

    // Emit convergence info in response for observability
    let convergence_info = json!({
        "min_rounds": min_rounds,
        "consecutive_stable_required": consecutive_stable_required,
        "consecutive_stable_count": new_stable,
        "round_has_ab_findings": round_has_ab,
        "effective_close": effective_close,
    });

    write_atomic_json(&path, &state)?;
    let tx = core_state::task_ledger::LedgerTransaction {
        ts: framework_kernel::time::now_iso(),
        tx_type: "quality_gate_state".to_string(),
        payload: state.clone(),
        idempotency_key: None,
        seq: None,
        schema_version: Some(1),
    };
    if let Err(e) =
        core_state::task_ledger::append_transaction_assuming_l1_held(repo_root, &task_id, tx)
    {
        tracing::error!(task_id = %task_id, error = %e, "failed to append quality_gate transaction to TASK_LEDGER");
    }
    core_state::task_state_aggregate::sync_task_state_aggregate_best_effort(
        repo_root, &task_id,
    );
    let mut resp = json!({
        "ok": true,
        "operation": "append_round",
        "task_id": task_id,
        "quality_gate_state_path": path.display().to_string(),
    });
    if let Some(w) = round_cap_warning {
        resp["warning"] = json!(w);
    }
    resp["convergence"] = convergence_info;
    merge_operator_nudge_refs(&mut resp, repo_root, Some(&state));
    resp["quality_gate_state"] = state;
    framework_kernel::emit_telemetry(&framework_kernel::TelemetryEvent::RfvRound {
        round: round_n as u32,
        verdict: verify_result.to_string(),
    });
    Ok(resp)
}

fn framework_quality_gate_impl(payload: Value) -> Result<Value, String> {
    let repo_root = payload
        .get("repo_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "framework_quality_gate requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(format!(
            "framework_quality_gate: repo_root is not a directory: {}",
            repo_root.display()
        ));
    }
    let repo_root = resolve_repo_root_arg(Some(repo_root.as_path()))?;
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();

    let task_id_override = payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match operation.as_str() {
        "status" => handle_status(&repo_root, task_id_override),
        "start" | "upsert" => handle_start_upsert(&payload, &repo_root, task_id_override),
        "append_round" => handle_append_round(&payload, &repo_root),
        _ => Err(format!(
            "framework_quality_gate: unknown operation '{operation}'"
        )),
    }
}
