//! Route policy payloads and diagnostic reports.
use super::constants::{
    ROUTE_AUTHORITY, ROUTE_POLICY_SCHEMA_VERSION, ROUTE_REPORT_SCHEMA_VERSION,
    ROUTE_RESOLUTION_SCHEMA_VERSION,
};
use super::types::{
    RouteDecision, RouteDecisionSnapshotPayload, RouteDiffReportPayload,
    RouteExecutionPolicyPayload, RouteResolutionPayload,
};
use core_errors::FrameworkError;

pub fn build_route_diff_report(
    mode: &str,
    rust_snapshot: RouteDecisionSnapshotPayload,
    route_decision: Option<&RouteDecision>,
) -> Result<RouteDiffReportPayload, FrameworkError> {
    let normalized_mode = mode.trim().to_ascii_lowercase();
    let strict_verification = match normalized_mode.as_str() {
        "shadow" => false,
        "verify" => true,
        _ => {
            return Err(FrameworkError::Unsupported {
                what: format!("unsupported route report mode: {mode}"),
            });
        }
    };
    let (verified_contract_fields, contract_mismatch_fields) =
        compare_route_contract_to_snapshot(route_decision, &rust_snapshot);

    Ok(RouteDiffReportPayload {
        report_schema_version: ROUTE_REPORT_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        mode: normalized_mode,
        primary_engine: "rust".to_string(),
        evidence_kind: "rust-owned-snapshot".to_string(),
        strict_verification,
        verification_passed: contract_mismatch_fields.is_empty(),
        verified_contract_fields,
        contract_mismatch_fields,
        route_snapshot: rust_snapshot,
    })
}

pub fn build_route_resolution(
    mode: &str,
    route_decision: &RouteDecision,
) -> Result<RouteResolutionPayload, FrameworkError> {
    let policy = build_route_policy(mode)?;
    let report = if policy.diagnostic_report_required {
        Some(build_route_diff_report(
            &policy.mode,
            route_decision.route_snapshot.clone(),
            Some(route_decision),
        )?)
    } else {
        None
    };
    if policy.strict_verification_required
        && report
            .as_ref()
            .map(|value| !value.verification_passed)
            .unwrap_or(false)
    {
        let mismatch_fields = report
            .as_ref()
            .map(|value| value.contract_mismatch_fields.join(", "))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(FrameworkError::Validation {
            message: format!(
                "Rust verification route report detected contract drift: {mismatch_fields}."
            ),
        });
    }
    Ok(RouteResolutionPayload {
        schema_version: ROUTE_RESOLUTION_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        policy,
        route_diagnostic_report: report,
    })
}

fn compare_route_contract_to_snapshot(
    route_decision: Option<&RouteDecision>,
    rust_snapshot: &RouteDecisionSnapshotPayload,
) -> (Vec<String>, Vec<String>) {
    let Some(route_decision) = route_decision else {
        return (Vec::new(), Vec::new());
    };

    let mut verified_fields = Vec::new();
    let mut mismatch_fields = Vec::new();

    let expected_fields = [
        (
            "engine",
            route_decision.route_snapshot.engine.as_str(),
            rust_snapshot.engine.as_str(),
        ),
        (
            "selected_skill",
            route_decision.selected_skill.as_str(),
            rust_snapshot.selected_skill.as_str(),
        ),
        (
            "layer",
            route_decision.layer.as_str(),
            rust_snapshot.layer.as_str(),
        ),
    ];
    for (field, expected, actual) in expected_fields {
        if expected == actual {
            verified_fields.push(field.to_string());
        } else {
            mismatch_fields.push(field.to_string());
        }
    }

    if route_decision.overlay_skill == rust_snapshot.overlay_skill {
        verified_fields.push("overlay_skill".to_string());
    } else {
        mismatch_fields.push("overlay_skill".to_string());
    }

    (verified_fields, mismatch_fields)
}

pub fn build_route_policy(mode: &str) -> Result<RouteExecutionPolicyPayload, FrameworkError> {
    let normalized_mode = mode.trim().to_ascii_lowercase();
    let base = RouteExecutionPolicyPayload {
        policy_schema_version: ROUTE_POLICY_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        mode: normalized_mode.clone(),
        diagnostic_route_mode: "none".to_string(),
        diagnostic_report_required: false,
        strict_verification_required: false,
    };
    let policy = match normalized_mode.as_str() {
        "shadow" => RouteExecutionPolicyPayload {
            diagnostic_route_mode: "shadow".to_string(),
            diagnostic_report_required: true,
            ..base
        },
        "verify" => RouteExecutionPolicyPayload {
            diagnostic_route_mode: "verify".to_string(),
            diagnostic_report_required: true,
            strict_verification_required: true,
            ..base
        },
        "rust" => base,
        _ => {
            return Err(FrameworkError::Unsupported {
                what: format!("unsupported route policy mode: {mode}"),
            });
        }
    };
    if policy.diagnostic_report_required && policy.diagnostic_route_mode == "none" {
        return Err(FrameworkError::Validation {
            message: "route policy declared diagnostics outside the diagnostic route mode".into(),
        });
    }
    if policy.strict_verification_required && !policy.diagnostic_report_required {
        return Err(FrameworkError::Validation {
            message: "route policy declared strict verification without diagnostic reporting"
                .into(),
        });
    }
    Ok(policy)
}
