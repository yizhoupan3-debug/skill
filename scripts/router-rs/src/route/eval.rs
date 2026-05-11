#[cfg(test)]
use rayon::prelude::*;
#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
use std::collections::HashSet;
#[cfg(test)]
use std::path::Path;

#[cfg(test)]
use super::constants::PARALLEL_EVAL_CASE_MIN;
#[cfg(test)]
use super::routing::route_task;
#[cfg(test)]
use super::text::{normalize_optional_text, read_json, value_to_string};
#[cfg(test)]
use super::types::{
    EvaluatedRoutingCase, RoutingEvalCasePayload, RoutingEvalCasesPayload,
    RoutingEvalMetricsPayload, RoutingEvalReportPayload, RoutingEvalResultPayload, SkillRecord,
};

#[cfg(test)]
pub(crate) fn load_routing_eval_cases(path: &Path) -> Result<RoutingEvalCasesPayload, String> {
    let payload = read_json(path)?;
    let cases = serde_json::from_value::<RoutingEvalCasesPayload>(payload)
        .map_err(|err| format!("failed parsing {}: {err}", path.display()))?;
    if cases.schema_version != "routing-eval-cases-v1" {
        return Err(format!(
            "routing eval case file returned an unknown schema: {:?}",
            cases.schema_version
        ));
    }
    Ok(cases)
}

#[cfg(test)]
pub(crate) fn evaluate_routing_cases(
    records: &[SkillRecord],
    cases_payload: RoutingEvalCasesPayload,
) -> Result<RoutingEvalReportPayload, String> {
    let mut metrics = RoutingEvalMetricsPayload::default();
    let cases = cases_payload.cases;
    let evaluate_one = |(input_index, case): (usize, RoutingEvalCasePayload)| -> Result<Option<EvaluatedRoutingCase>, String> {
        let task = case.task.trim().to_string();
        if task.is_empty() {
            return Ok(None);
        }

        let session_suffix = case
            .id
            .as_ref()
            .map(value_to_string)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| (input_index + 1).to_string());
        let decision = route_task(
            records,
            &task,
            &format!("routing-eval::{session_suffix}"),
            true,
            case.first_turn,
        )?;
        if let Some(expected) = case.expected_layer.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty())
        {
            if decision.layer != *expected {
                return Err(format!(
                    "routing-eval case id={:?}: expected_layer {expected:?} != actual {:?}",
                    case.id, decision.layer
                ));
            }
        }
        if let Some(ref expected_ctx) = case.route_context {
            if &decision.route_context != expected_ctx {
                return Err(format!(
                    "routing-eval case id={:?}: route_context mismatch: actual={:?} expected={expected_ctx:?}",
                    case.id, decision.route_context
                ));
            }
        }
        let selected_owner = decision.selected_skill.clone();
        let selected_overlay = decision.overlay_skill.clone();

        let category = case.category.trim().to_string();
        let expected_owner = normalize_optional_text(case.expected_owner);
        let expected_overlay = normalize_optional_text(case.expected_overlay);
        let focus_skill = normalize_optional_text(case.focus_skill);
        let forbidden_owners = case
            .forbidden_owners
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();

        let mut trigger_hit = false;
        let mut overtrigger = false;
        let owner_correct = expected_owner
            .as_ref()
            .map(|expected| expected == &selected_owner)
            .unwrap_or(false);
        let overlay_correct = match &expected_overlay {
            Some(expected) => Some(expected) == selected_overlay.as_ref(),
            None => selected_overlay.is_none(),
        };

        match category.as_str() {
            "should-trigger" => {
                trigger_hit = focus_skill
                    .as_ref()
                    .map(|focus| focus == &selected_owner)
                    .unwrap_or(false);
            }
            "should-not-trigger" => {
                overtrigger = forbidden_owners.contains(&selected_owner);
            }
            "wrong-owner-near-miss" | "gate-vs-owner-conflict" => {
                trigger_hit = focus_skill
                    .as_ref()
                    .map(|focus| focus == &selected_owner)
                    .unwrap_or(false);
                if forbidden_owners.contains(&selected_owner) {
                    overtrigger = true;
                }
            }
            _ => {}
        }

        let mut forbidden_owner_list = forbidden_owners.into_iter().collect::<Vec<_>>();
        forbidden_owner_list.sort();
        Ok(Some(EvaluatedRoutingCase {
            input_index,
            result: RoutingEvalResultPayload {
                id: case.id,
                category,
                task,
                focus_skill,
                selected_owner,
                selected_overlay,
                expected_owner,
                expected_overlay,
                forbidden_owners: forbidden_owner_list,
                trigger_hit,
                overtrigger,
                owner_correct,
                overlay_correct,
            },
        }))
    };

    let mut evaluated = if cases.len() < PARALLEL_EVAL_CASE_MIN {
        cases
            .into_iter()
            .enumerate()
            .map(evaluate_one)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        cases
            .into_par_iter()
            .enumerate()
            .map(evaluate_one)
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
    }
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    evaluated.sort_by_key(|row| row.input_index);

    let mut results = Vec::with_capacity(evaluated.len());
    for row in evaluated {
        metrics.case_count += 1;
        match row.result.category.as_str() {
            "should-trigger" | "wrong-owner-near-miss" | "gate-vs-owner-conflict" => {
                if row.result.trigger_hit {
                    metrics.trigger_hit += 1;
                } else {
                    metrics.trigger_miss += 1;
                }
            }
            _ => {}
        }
        if row.result.overtrigger {
            metrics.overtrigger += 1;
        }
        if row.result.owner_correct {
            metrics.owner_correct += 1;
        }
        if row.result.overlay_correct {
            metrics.overlay_correct += 1;
        }
        results.push(row.result);
    }

    Ok(RoutingEvalReportPayload {
        schema_version: "routing-eval-v1".to_string(),
        metrics,
        results,
    })
}
