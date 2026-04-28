use serde_json::{json, Map, Value};

pub const EXECUTION_SCHEMA_VERSION: &str = "router-rs-execute-response-v1";
pub const EXECUTION_METADATA_SCHEMA_VERSION: &str = "router-rs-execution-kernel-metadata-v1";
pub const EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION: &str =
    "router-rs-execution-kernel-metadata-contract-v1";
pub const EXECUTION_CONTRACT_BUNDLE_SCHEMA_VERSION: &str =
    "router-rs-execution-kernel-contract-bundle-v1";
pub const EXECUTION_AUTHORITY: &str = "rust-execution-cli";
pub const EXECUTION_KERNEL_KIND: &str = "rust-execution-kernel-slice";
pub const EXECUTION_KERNEL_AUTHORITY: &str = "rust-execution-kernel-authority";
pub const EXECUTION_KERNEL_CONTRACT_MODE: &str = "rust-live-primary";
pub const EXECUTION_KERNEL_FALLBACK_POLICY: &str = "infrastructure-only-explicit";
pub const EXECUTION_KERNEL_DELEGATE_FAMILY: &str = "rust-cli";
pub const EXECUTION_KERNEL_DELEGATE_IMPL: &str = "router-rs";
pub const EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY: &str = "live_primary";
pub const EXECUTION_RESPONSE_SHAPE_DRY_RUN: &str = "dry_run";
pub const EXECUTION_PROMPT_PREVIEW_OWNER: &str = "rust-execution-cli";
pub const EXECUTION_MODEL_ID_SOURCE: &str = "aggregator-response.model";
pub const EXECUTION_LIVE_RESPONSE_SERIALIZATION_CONTRACT: &str =
    "execution_kernel_live_response_serialization_contract_v1";

const EXECUTION_PUBLIC_RESPONSE_FIELDS: [&str; 10] = [
    "session_id",
    "user_id",
    "skill",
    "overlay",
    "live_run",
    "content",
    "usage",
    "prompt_preview",
    "model_id",
    "metadata",
];
const EXECUTION_USAGE_FIELDS: [&str; 4] = ["input_tokens", "output_tokens", "total_tokens", "mode"];
const EXECUTION_STEADY_STATE_FIELDS: [&str; 14] = [
    "execution_kernel_metadata_schema_version",
    "execution_kernel",
    "execution_kernel_authority",
    "execution_kernel_contract_mode",
    "execution_kernel_fallback_policy",
    "execution_kernel_in_process_replacement_complete",
    "execution_kernel_delegate",
    "execution_kernel_delegate_authority",
    "execution_kernel_delegate_family",
    "execution_kernel_delegate_impl",
    "execution_kernel_live_primary",
    "execution_kernel_live_primary_authority",
    "execution_kernel_response_shape",
    "execution_kernel_prompt_preview_owner",
];
const EXECUTION_SHARED_RUNTIME_FIELDS: [&str; 2] = ["trace_event_count", "trace_output_path"];
const EXECUTION_LIVE_PRIMARY_REQUIRED_RUNTIME_FIELDS: [&str; 6] = [
    "run_id",
    "status",
    "execution_mode",
    "execution_kernel_model_id_source",
    "trace_event_count",
    "trace_output_path",
];
const EXECUTION_LIVE_PRIMARY_PASSTHROUGH_RUNTIME_FIELDS: [&str; 2] =
    ["route_engine", "diagnostic_route_mode"];
const EXECUTION_DRY_RUN_REQUIRED_RUNTIME_FIELDS: [&str; 6] = [
    "reason",
    "execution_mode",
    "execution_kernel_contract_mode",
    "execution_kernel_fallback_policy",
    "trace_event_count",
    "trace_output_path",
];

#[derive(Debug, Clone)]
struct ExecutionKernelExpectations {
    execution_kernel: String,
    execution_kernel_authority: String,
    execution_kernel_delegate: String,
    execution_kernel_delegate_authority: String,
}

fn non_empty_string(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(Value::String(text)) if !text.trim().is_empty() => text.trim().to_string(),
        Some(Value::Null) | None => fallback.to_string(),
        Some(other) => match other {
            Value::Bool(flag) => flag.to_string(),
            Value::Number(number) => number.to_string(),
            _ => other.to_string(),
        },
    }
}

fn required_object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{context} must decode to a JSON object."))
}

fn required_payload_object<'a>(
    value: Option<&'a Value>,
    context: &str,
) -> Result<&'a Map<String, Value>, String> {
    match value {
        Some(Value::Object(map)) => Ok(map),
        Some(_) => Err(format!("{context} must decode to a JSON object.")),
        None => Err(format!("{context} is missing.")),
    }
}

fn required_string_field(
    payload: &Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<String, String> {
    match payload.get(field) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(Value::Null) | None => Err(format!("{context} is missing {field}.")),
        Some(other) => Ok(match other {
            Value::Bool(flag) => flag.to_string(),
            Value::Number(number) => number.to_string(),
            _ => other.to_string(),
        }),
    }
}

fn optional_string_field(payload: &Map<String, Value>, field: &str) -> Option<String> {
    match payload.get(field) {
        Some(Value::Null) | None => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(other) => Some(match other {
            Value::Bool(flag) => flag.to_string(),
            Value::Number(number) => number.to_string(),
            _ => other.to_string(),
        }),
    }
}

fn normalize_runtime_field_group(
    runtime_fields_payload: Option<&Map<String, Value>>,
    name: &str,
    fallback: &[&str],
) -> Result<Vec<String>, String> {
    let payload = runtime_fields_payload
        .and_then(|fields| fields.get(name))
        .unwrap_or(&Value::Null);
    let values = if payload.is_null() {
        fallback
            .iter()
            .map(|field| field.to_string())
            .collect::<Vec<_>>()
    } else {
        let sequence = payload.as_array().ok_or_else(|| {
            format!(
                "execution-kernel metadata contract returned an invalid runtime field group: {name}={payload:?}"
            )
        })?;
        let mut values = Vec::with_capacity(sequence.len());
        for field in sequence {
            match field {
                Value::String(text) if !text.trim().is_empty() => values.push(text.clone()),
                _ => {
                    return Err(format!(
                        "execution-kernel metadata contract returned an invalid runtime field group: {name}={payload:?}"
                    ))
                }
            }
        }
        values
    };
    Ok(values)
}

fn runtime_fields_payload() -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert(
        "shared".to_string(),
        Value::Array(
            EXECUTION_SHARED_RUNTIME_FIELDS
                .iter()
                .map(|field| Value::String((*field).to_string()))
                .collect(),
        ),
    );
    payload.insert(
        "live_primary_required".to_string(),
        Value::Array(
            EXECUTION_LIVE_PRIMARY_REQUIRED_RUNTIME_FIELDS
                .iter()
                .map(|field| Value::String((*field).to_string()))
                .collect(),
        ),
    );
    payload.insert(
        "live_primary_passthrough".to_string(),
        Value::Array(
            EXECUTION_LIVE_PRIMARY_PASSTHROUGH_RUNTIME_FIELDS
                .iter()
                .map(|field| Value::String((*field).to_string()))
                .collect(),
        ),
    );
    payload.insert(
        "dry_run_required".to_string(),
        Value::Array(
            EXECUTION_DRY_RUN_REQUIRED_RUNTIME_FIELDS
                .iter()
                .map(|field| Value::String((*field).to_string()))
                .collect(),
        ),
    );
    payload
}

pub fn build_steady_state_execution_kernel_metadata(response_shape: &str) -> Map<String, Value> {
    let mut metadata = Map::new();
    metadata.insert(
        "execution_kernel_metadata_schema_version".to_string(),
        Value::String(EXECUTION_METADATA_SCHEMA_VERSION.to_string()),
    );
    metadata.insert(
        "execution_kernel".to_string(),
        Value::String(EXECUTION_KERNEL_KIND.to_string()),
    );
    metadata.insert(
        "execution_kernel_authority".to_string(),
        Value::String(EXECUTION_KERNEL_AUTHORITY.to_string()),
    );
    metadata.insert(
        "execution_kernel_contract_mode".to_string(),
        Value::String(EXECUTION_KERNEL_CONTRACT_MODE.to_string()),
    );
    metadata.insert(
        "execution_kernel_fallback_policy".to_string(),
        Value::String(EXECUTION_KERNEL_FALLBACK_POLICY.to_string()),
    );
    metadata.insert(
        "execution_kernel_in_process_replacement_complete".to_string(),
        Value::Bool(true),
    );
    metadata.insert(
        "execution_kernel_delegate".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_IMPL.to_string()),
    );
    metadata.insert(
        "execution_kernel_delegate_authority".to_string(),
        Value::String(EXECUTION_AUTHORITY.to_string()),
    );
    metadata.insert(
        "execution_kernel_delegate_family".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_FAMILY.to_string()),
    );
    metadata.insert(
        "execution_kernel_delegate_impl".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_IMPL.to_string()),
    );
    metadata.insert(
        "execution_kernel_live_primary".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_IMPL.to_string()),
    );
    metadata.insert(
        "execution_kernel_live_primary_authority".to_string(),
        Value::String(EXECUTION_AUTHORITY.to_string()),
    );
    metadata.insert(
        "execution_kernel_response_shape".to_string(),
        Value::String(response_shape.to_string()),
    );
    metadata.insert(
        "execution_kernel_prompt_preview_owner".to_string(),
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string()),
    );
    metadata
}

pub fn build_execution_kernel_contracts_by_mode() -> Map<String, Value> {
    let mut contracts = Map::new();
    contracts.insert(
        EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY.to_string(),
        Value::Object(build_steady_state_execution_kernel_metadata(
            EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
        )),
    );
    contracts.insert(
        EXECUTION_RESPONSE_SHAPE_DRY_RUN.to_string(),
        Value::Object(build_steady_state_execution_kernel_metadata(
            EXECUTION_RESPONSE_SHAPE_DRY_RUN,
        )),
    );
    contracts
}

pub fn build_execution_kernel_metadata_contract() -> Value {
    json!({
        "schema_version": EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION,
        "authority": EXECUTION_KERNEL_AUTHORITY,
        "steady_state_fields": EXECUTION_STEADY_STATE_FIELDS,
        "runtime_fields": runtime_fields_payload(),
        "metadata_keys": {
            "metadata_schema_version": "execution_kernel_metadata_schema_version",
            "contract_mode": "execution_kernel_contract_mode",
            "fallback_policy": "execution_kernel_fallback_policy",
            "response_shape": "execution_kernel_response_shape",
            "prompt_preview_owner": "execution_kernel_prompt_preview_owner",
            "model_id_source": "execution_kernel_model_id_source",
        },
        "defaults": {
            "contract_mode": EXECUTION_KERNEL_CONTRACT_MODE,
            "fallback_policy": EXECUTION_KERNEL_FALLBACK_POLICY,
            "prompt_preview_owner_by_mode": {
                EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY: EXECUTION_PROMPT_PREVIEW_OWNER,
                EXECUTION_RESPONSE_SHAPE_DRY_RUN: EXECUTION_PROMPT_PREVIEW_OWNER,
            },
            "live_primary_model_id_source": EXECUTION_MODEL_ID_SOURCE,
            "supported_response_shapes": [
                EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
                EXECUTION_RESPONSE_SHAPE_DRY_RUN,
            ],
        },
    })
}

pub fn build_execution_kernel_live_response_serialization_contract() -> Map<String, Value> {
    let steady_state_fields = EXECUTION_STEADY_STATE_FIELDS
        .iter()
        .map(|field| Value::String((*field).to_string()))
        .collect::<Vec<_>>();
    let mut live_primary_required_metadata_fields = steady_state_fields.clone();
    live_primary_required_metadata_fields.extend(
        EXECUTION_LIVE_PRIMARY_REQUIRED_RUNTIME_FIELDS
            .iter()
            .map(|field| Value::String((*field).to_string())),
    );
    let mut dry_run_required_metadata_fields = steady_state_fields.clone();
    dry_run_required_metadata_fields.extend(
        EXECUTION_DRY_RUN_REQUIRED_RUNTIME_FIELDS
            .iter()
            .map(|field| Value::String((*field).to_string())),
    );

    let mut runtime_response_metadata_fields = Map::new();
    runtime_response_metadata_fields.insert(
        "shared".to_string(),
        Value::Array(
            EXECUTION_SHARED_RUNTIME_FIELDS
                .iter()
                .map(|field| Value::String((*field).to_string()))
                .collect(),
        ),
    );
    runtime_response_metadata_fields.insert(
        "steady_state_kernel".to_string(),
        Value::Array(steady_state_fields.clone()),
    );
    runtime_response_metadata_fields.insert(
        "live_primary".to_string(),
        Value::Array(vec![
            Value::String("run_id".to_string()),
            Value::String("status".to_string()),
            Value::String("execution_mode".to_string()),
            Value::String("route_engine".to_string()),
            Value::String("diagnostic_route_mode".to_string()),
            Value::String("execution_kernel_model_id_source".to_string()),
        ]),
    );
    runtime_response_metadata_fields.insert(
        "dry_run".to_string(),
        Value::Array(vec![
            Value::String("reason".to_string()),
            Value::String("execution_kernel_contract_mode".to_string()),
            Value::String("execution_kernel_fallback_policy".to_string()),
        ]),
    );

    let mut current_contract_truth = Map::new();
    current_contract_truth.insert(
        "public_response_model".to_string(),
        Value::String("RunTaskResponse".to_string()),
    );
    current_contract_truth.insert(
        "execution_request_schema_version".to_string(),
        Value::String("router-rs-execute-request-v1".to_string()),
    );
    current_contract_truth.insert(
        "live_primary_schema_version".to_string(),
        Value::String(EXECUTION_SCHEMA_VERSION.to_string()),
    );
    current_contract_truth.insert(
        "steady_state_metadata_schema_version".to_string(),
        Value::String(EXECUTION_METADATA_SCHEMA_VERSION.to_string()),
    );
    current_contract_truth.insert(
        "live_primary_prompt_preview_owner".to_string(),
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string()),
    );
    current_contract_truth.insert(
        "steady_state_response_shapes".to_string(),
        json!([
            EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
            EXECUTION_RESPONSE_SHAPE_DRY_RUN,
        ]),
    );
    current_contract_truth.insert(
        "dry_run_prompt_preview_owner".to_string(),
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string()),
    );
    current_contract_truth.insert(
        "live_primary_model_id_source".to_string(),
        Value::String(EXECUTION_MODEL_ID_SOURCE.to_string()),
    );

    let mut live_primary = Map::new();
    live_primary.insert("live_run".to_string(), Value::Bool(true));
    live_primary.insert("usage_mode".to_string(), Value::String("live".to_string()));
    live_primary.insert(
        "content_type".to_string(),
        Value::String("string".to_string()),
    );
    live_primary.insert(
        "prompt_preview_source".to_string(),
        Value::String("rust-owned-live-prompt".to_string()),
    );
    live_primary.insert("model_id_present".to_string(), Value::Bool(true));
    live_primary.insert(
        "required_metadata_fields".to_string(),
        Value::Array(live_primary_required_metadata_fields),
    );
    live_primary.insert(
        "steady_state_metadata_fields".to_string(),
        Value::Array(steady_state_fields.clone()),
    );
    live_primary.insert(
        "pass_through_metadata_fields".to_string(),
        Value::Array(
            EXECUTION_LIVE_PRIMARY_PASSTHROUGH_RUNTIME_FIELDS
                .iter()
                .map(|field| Value::String((*field).to_string()))
                .collect(),
        ),
    );

    let mut dry_run = Map::new();
    dry_run.insert("live_run".to_string(), Value::Bool(false));
    dry_run.insert(
        "usage_mode".to_string(),
        Value::String("estimated".to_string()),
    );
    dry_run.insert(
        "content_type".to_string(),
        Value::String("string".to_string()),
    );
    dry_run.insert(
        "prompt_preview_source".to_string(),
        Value::String("rust-owned-dry-run-prompt".to_string()),
    );
    dry_run.insert("model_id_present".to_string(), Value::Bool(false));
    dry_run.insert(
        "required_metadata_fields".to_string(),
        Value::Array(dry_run_required_metadata_fields),
    );
    dry_run.insert(
        "steady_state_metadata_fields".to_string(),
        Value::Array(steady_state_fields),
    );
    dry_run.insert("fallback_reason_present".to_string(), Value::Bool(false));

    let mut steady_state_gates = Map::new();
    steady_state_gates.insert(
        "response_shape_contract_externalized".to_string(),
        Value::Bool(true),
    );
    steady_state_gates.insert(
        "live_primary_response_contract_externalized".to_string(),
        Value::Bool(true),
    );
    steady_state_gates.insert(
        "runtime_control_flow_change_required".to_string(),
        Value::Bool(false),
    );

    let mut guardrails = Map::new();
    guardrails.insert(
        "thin_projection_boundary_preserved".to_string(),
        Value::Bool(true),
    );
    guardrails.insert(
        "cli_hosts_may_not_become_framework_truth".to_string(),
        Value::Bool(true),
    );
    guardrails.insert(
        "codex_runtime_semantics_remain_host_owned".to_string(),
        Value::Bool(true),
    );

    let mut payload = Map::new();
    payload.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    payload.insert(
        "status_contract".to_string(),
        Value::String(EXECUTION_LIVE_RESPONSE_SERIALIZATION_CONTRACT.to_string()),
    );
    payload.insert(
        "scope".to_string(),
        Value::String("live_response_serialization".to_string()),
    );
    payload.insert(
        "artifact_role".to_string(),
        Value::String("shared-contract-evidence".to_string()),
    );
    payload.insert(
        "public_response_fields".to_string(),
        Value::Array(
            EXECUTION_PUBLIC_RESPONSE_FIELDS
                .iter()
                .map(|field| Value::String((*field).to_string()))
                .collect(),
        ),
    );
    payload.insert(
        "usage_contract".to_string(),
        json!({
            "fields": EXECUTION_USAGE_FIELDS,
            "live_mode": "live",
            "dry_run_mode": "estimated",
        }),
    );
    payload.insert(
        "runtime_response_metadata_fields".to_string(),
        Value::Object(runtime_response_metadata_fields),
    );
    payload.insert(
        "current_contract_truth".to_string(),
        Value::Object(current_contract_truth),
    );
    payload.insert(
        "current_response_shape_truth".to_string(),
        json!({
            "live_primary": live_primary,
            "dry_run": dry_run,
        }),
    );
    payload.insert(
        "steady_state_gates".to_string(),
        Value::Object(steady_state_gates),
    );
    payload.insert("guardrails".to_string(), Value::Object(guardrails));
    payload
}

pub fn build_execution_contract_bundle() -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert(
        "schema_version".to_string(),
        Value::String(EXECUTION_CONTRACT_BUNDLE_SCHEMA_VERSION.to_string()),
    );
    payload.insert(
        "authority".to_string(),
        Value::String(EXECUTION_KERNEL_AUTHORITY.to_string()),
    );
    payload.insert(
        "metadata_contract".to_string(),
        build_execution_kernel_metadata_contract(),
    );
    payload.insert(
        "contract_by_mode".to_string(),
        Value::Object(build_execution_kernel_contracts_by_mode()),
    );
    payload.insert(
        "live_response_serialization_contract".to_string(),
        Value::Object(build_execution_kernel_live_response_serialization_contract()),
    );
    payload
}

fn resolve_execution_kernel_expectations(
    kernel_contract: Option<&Map<String, Value>>,
) -> ExecutionKernelExpectations {
    let contract = kernel_contract.cloned().unwrap_or_default();
    ExecutionKernelExpectations {
        execution_kernel: non_empty_string(contract.get("execution_kernel"), EXECUTION_KERNEL_KIND),
        execution_kernel_authority: non_empty_string(
            contract.get("execution_kernel_authority"),
            EXECUTION_KERNEL_AUTHORITY,
        ),
        execution_kernel_delegate: non_empty_string(
            contract.get("execution_kernel_delegate"),
            EXECUTION_KERNEL_DELEGATE_IMPL,
        ),
        execution_kernel_delegate_authority: non_empty_string(
            contract.get("execution_kernel_delegate_authority"),
            EXECUTION_AUTHORITY,
        ),
    }
}

fn normalize_execution_kernel_metadata_contract_impl(
    kernel_metadata_contract: Option<&Value>,
) -> Result<Map<String, Value>, String> {
    let expected = build_execution_kernel_metadata_contract();
    let expected_object = required_object(&expected, "execution kernel metadata contract")?;
    if kernel_metadata_contract.is_none() || kernel_metadata_contract == Some(&Value::Null) {
        return Ok(expected_object.clone());
    }
    let payload = required_object(
        kernel_metadata_contract.expect("checked above"),
        "runtime control plane execution descriptor returned an invalid kernel_metadata_contract",
    )
    .map_err(|_| {
        "runtime control plane execution descriptor returned an invalid kernel_metadata_contract."
            .to_string()
    })?;

    for (field, expected_value) in [
        ("schema_version", expected_object["schema_version"].clone()),
        ("authority", expected_object["authority"].clone()),
    ] {
        if payload.get(field) != Some(&expected_value) {
            return Err(format!(
                "runtime control plane execution descriptor returned an unexpected kernel_metadata_contract.{field}: {:?}",
                payload.get(field)
            ));
        }
    }

    let expected_fields = expected_object
        .get("steady_state_fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let payload_fields = payload
        .get("steady_state_fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if payload_fields != expected_fields {
        return Err(
            "runtime control plane execution descriptor returned an unexpected kernel_metadata_contract.steady_state_fields.".to_string(),
        );
    }

    let runtime_fields_value = payload.get("runtime_fields").ok_or_else(|| {
        "runtime control plane execution descriptor returned an invalid kernel_metadata_contract.runtime_fields."
            .to_string()
    })?;
    let runtime_fields_map = runtime_fields_value.as_object().ok_or_else(|| {
        "runtime control plane execution descriptor returned an invalid kernel_metadata_contract.runtime_fields."
            .to_string()
    })?;
    let normalized_runtime_fields = json!({
        "shared": normalize_runtime_field_group(Some(runtime_fields_map), "shared", &EXECUTION_SHARED_RUNTIME_FIELDS)?,
        "live_primary_required": normalize_runtime_field_group(
            Some(runtime_fields_map),
            "live_primary_required",
            &EXECUTION_LIVE_PRIMARY_REQUIRED_RUNTIME_FIELDS,
        )?,
        "live_primary_passthrough": normalize_runtime_field_group(
            Some(runtime_fields_map),
            "live_primary_passthrough",
            &EXECUTION_LIVE_PRIMARY_PASSTHROUGH_RUNTIME_FIELDS,
        )?,
        "dry_run_required": normalize_runtime_field_group(
            Some(runtime_fields_map),
            "dry_run_required",
            &EXECUTION_DRY_RUN_REQUIRED_RUNTIME_FIELDS,
        )?,
    });
    if payload.get("runtime_fields") != Some(&normalized_runtime_fields) {
        return Err(
            "runtime control plane execution descriptor returned an unexpected kernel_metadata_contract.runtime_fields.".to_string(),
        );
    }

    for section in ["metadata_keys", "defaults"] {
        let section_payload = payload.get(section).ok_or_else(|| {
            format!(
                "runtime control plane execution descriptor returned an invalid kernel_metadata_contract.{section}."
            )
        })?;
        if !section_payload.is_object() {
            return Err(format!(
                "runtime control plane execution descriptor returned an invalid kernel_metadata_contract.{section}."
            ));
        }
        if section_payload != &expected_object[section] {
            return Err(format!(
                "runtime control plane execution descriptor returned an unexpected kernel_metadata_contract.{section}."
            ));
        }
    }

    Ok(expected_object.clone())
}

fn validate_execution_kernel_contract_impl(
    kernel_contract: &Value,
    response_shape: Option<&str>,
) -> Result<Map<String, Value>, String> {
    let contract = required_object(
        kernel_contract,
        "execution-kernel steady-state metadata returned an invalid contract",
    )
    .map_err(|_| {
        "execution-kernel steady-state metadata returned an invalid contract.".to_string()
    })?;
    let expectations = resolve_execution_kernel_expectations(Some(contract));
    validate_execution_kernel_steady_state_metadata_impl(
        kernel_contract,
        Some(contract),
        &expectations,
        response_shape,
    )
}

fn validate_execution_kernel_steady_state_metadata_impl(
    metadata: &Value,
    kernel_contract: Option<&Map<String, Value>>,
    expectations: &ExecutionKernelExpectations,
    response_shape: Option<&str>,
) -> Result<Map<String, Value>, String> {
    let metadata =
        required_object(metadata, "execution-kernel steady-state metadata").map_err(|_| {
            "execution-kernel steady-state metadata must decode to a JSON object.".to_string()
        })?;
    let metadata_contract = build_execution_kernel_metadata_contract();
    let metadata_contract_object =
        required_object(&metadata_contract, "execution kernel metadata contract")?;
    let steady_state_fields = metadata_contract_object["steady_state_fields"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let metadata_keys = metadata_contract_object["metadata_keys"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let defaults = metadata_contract_object["defaults"]
        .as_object()
        .cloned()
        .unwrap_or_default();

    let missing = steady_state_fields
        .iter()
        .filter_map(|field| field.as_str())
        .filter(|field| !metadata.contains_key(*field))
        .map(|field| field.to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "execution-kernel steady-state metadata is incomplete: {}",
            missing.join(", ")
        ));
    }

    let response_shape_key = metadata_keys
        .get("response_shape")
        .and_then(Value::as_str)
        .unwrap_or("execution_kernel_response_shape");
    let actual_shape = metadata
        .get(response_shape_key)
        .and_then(Value::as_str)
        .unwrap_or_default();
    let resolved_shape = response_shape.unwrap_or(actual_shape);
    let supported_shapes = defaults["supported_response_shapes"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    if !supported_shapes.iter().any(|shape| shape == resolved_shape) {
        return Err(format!(
            "execution-kernel steady-state metadata returned an unsupported response_shape: {:?}",
            metadata.get(response_shape_key)
        ));
    }

    let expected_prompt_preview_owner = defaults["prompt_preview_owner_by_mode"]
        .get(resolved_shape)
        .and_then(Value::as_str)
        .unwrap_or(EXECUTION_PROMPT_PREVIEW_OWNER);
    let expected_pairs = [
        (
            metadata_keys["metadata_schema_version"]
                .as_str()
                .unwrap_or("execution_kernel_metadata_schema_version"),
            Value::String(EXECUTION_METADATA_SCHEMA_VERSION.to_string()),
        ),
        (
            "execution_kernel",
            Value::String(expectations.execution_kernel.clone()),
        ),
        (
            "execution_kernel_authority",
            Value::String(expectations.execution_kernel_authority.clone()),
        ),
        (
            metadata_keys["contract_mode"]
                .as_str()
                .unwrap_or("execution_kernel_contract_mode"),
            Value::String(
                defaults["contract_mode"]
                    .as_str()
                    .unwrap_or(EXECUTION_KERNEL_CONTRACT_MODE)
                    .to_string(),
            ),
        ),
        (
            metadata_keys["fallback_policy"]
                .as_str()
                .unwrap_or("execution_kernel_fallback_policy"),
            Value::String(
                defaults["fallback_policy"]
                    .as_str()
                    .unwrap_or(EXECUTION_KERNEL_FALLBACK_POLICY)
                    .to_string(),
            ),
        ),
        (
            "execution_kernel_in_process_replacement_complete",
            Value::Bool(true),
        ),
        (
            "execution_kernel_delegate",
            Value::String(expectations.execution_kernel_delegate.clone()),
        ),
        (
            "execution_kernel_delegate_authority",
            Value::String(expectations.execution_kernel_delegate_authority.clone()),
        ),
        (
            response_shape_key,
            Value::String(resolved_shape.to_string()),
        ),
        (
            metadata_keys["prompt_preview_owner"]
                .as_str()
                .unwrap_or("execution_kernel_prompt_preview_owner"),
            Value::String(expected_prompt_preview_owner.to_string()),
        ),
    ];
    for (field, expected) in expected_pairs {
        if metadata.get(field) != Some(&expected) {
            return Err(format!(
                "execution-kernel steady-state metadata returned an unexpected value: {field}={:?}",
                metadata.get(field)
            ));
        }
    }
    for field in [
        "execution_kernel_live_primary",
        "execution_kernel_live_primary_authority",
    ] {
        match metadata.get(field) {
            Some(Value::String(value)) if !value.trim().is_empty() => {}
            other => {
                return Err(format!(
                    "execution-kernel steady-state metadata returned an invalid value: {field}={other:?}"
                ))
            }
        }
    }
    for field in [
        "execution_kernel_fallback_reason",
        "execution_kernel_compatibility_agent_contract",
        "execution_kernel_compatibility_agent_kind",
        "execution_kernel_compatibility_agent_authority",
    ] {
        if metadata.contains_key(field) {
            return Err(format!(
                "execution-kernel steady-state metadata returned an unsupported compatibility field: {field}"
            ));
        }
    }

    let mut normalized = Map::new();
    for (key, value) in metadata {
        normalized.insert(key.clone(), value.clone());
    }
    if kernel_contract.is_some() {
        return Ok(normalized);
    }
    Ok(normalized)
}

fn validate_router_rs_execution_metadata_impl(
    metadata: &Value,
    live_run: bool,
    usage_mode: &str,
    kernel_contract: Option<&Map<String, Value>>,
) -> Result<Map<String, Value>, String> {
    let expectations = resolve_execution_kernel_expectations(kernel_contract);
    let normalized = validate_execution_kernel_steady_state_metadata_impl(
        metadata,
        kernel_contract,
        &expectations,
        Some(if live_run {
            EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY
        } else {
            EXECUTION_RESPONSE_SHAPE_DRY_RUN
        }),
    )?;
    let required_fields = if live_run {
        EXECUTION_LIVE_PRIMARY_REQUIRED_RUNTIME_FIELDS.as_slice()
    } else {
        EXECUTION_DRY_RUN_REQUIRED_RUNTIME_FIELDS.as_slice()
    };
    let missing = required_fields
        .iter()
        .filter(|field| !normalized.contains_key(**field))
        .map(|field| (*field).to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "router-rs execute returned incomplete metadata: {}",
            missing.join(", ")
        ));
    }

    let expected_usage_mode = if live_run { "live" } else { "estimated" };
    if usage_mode != expected_usage_mode {
        return Err(format!(
            "router-rs execute returned an unexpected usage mode: {usage_mode:?} != {expected_usage_mode:?}"
        ));
    }

    let expected_execution_mode = if live_run { "live" } else { "dry_run" };
    if normalized.get("execution_mode") != Some(&Value::String(expected_execution_mode.to_string()))
    {
        return Err(format!(
            "router-rs execute returned an unexpected metadata value: execution_mode={:?}",
            normalized.get("execution_mode")
        ));
    }
    if live_run
        && normalized.get("execution_kernel_model_id_source")
            != Some(&Value::String(EXECUTION_MODEL_ID_SOURCE.to_string()))
    {
        return Err(format!(
            "router-rs execute returned an unexpected metadata value: execution_kernel_model_id_source={:?}",
            normalized.get("execution_kernel_model_id_source")
        ));
    }
    Ok(normalized)
}

pub fn normalize_execution_kernel_metadata_contract_value(
    payload: Option<&Value>,
) -> Result<Value, String> {
    Ok(Value::Object(
        normalize_execution_kernel_metadata_contract_impl(payload)?,
    ))
}

pub fn normalize_execution_kernel_contract_value(
    kernel_contract: &Value,
    response_shape: Option<&str>,
) -> Result<Value, String> {
    Ok(Value::Object(validate_execution_kernel_contract_impl(
        kernel_contract,
        response_shape,
    )?))
}

pub fn validate_execution_kernel_steady_state_metadata_value(
    metadata: &Value,
    kernel_contract: Option<&Value>,
    response_shape: Option<&str>,
) -> Result<Value, String> {
    let contract_object = match kernel_contract {
        Some(value) => Some(required_object(value, "execution-kernel contract")?),
        None => None,
    };
    let expectations = resolve_execution_kernel_expectations(contract_object);
    Ok(Value::Object(
        validate_execution_kernel_steady_state_metadata_impl(
            metadata,
            contract_object,
            &expectations,
            response_shape,
        )?,
    ))
}

pub fn decode_execution_response_value(
    payload: &Value,
    kernel_contract: Option<&Value>,
    dry_run: Option<bool>,
) -> Result<Value, String> {
    let payload = required_object(payload, "router-rs execute response")?;
    let usage_payload = required_payload_object(payload.get("usage"), "router-rs execute usage")?;
    let live_run = match payload.get("live_run") {
        Some(Value::Bool(value)) => *value,
        Some(Value::Null) | None => false,
        Some(other) => other.as_bool().unwrap_or(false),
    };
    let live_run = dry_run.map(|flag| !flag).unwrap_or(live_run);
    let kernel_contract_object = match kernel_contract {
        Some(value) => Some(required_object(value, "execution-kernel contract")?),
        None => None,
    };
    let metadata = validate_router_rs_execution_metadata_impl(
        payload
            .get("metadata")
            .unwrap_or(&Value::Object(Map::new())),
        live_run,
        usage_payload
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("live"),
        kernel_contract_object,
    )?;

    let mut usage = Map::new();
    usage.insert(
        "input_tokens".to_string(),
        Value::Number(serde_json::Number::from(
            usage_payload
                .get("input_tokens")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        )),
    );
    usage.insert(
        "output_tokens".to_string(),
        Value::Number(serde_json::Number::from(
            usage_payload
                .get("output_tokens")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        )),
    );
    usage.insert(
        "total_tokens".to_string(),
        Value::Number(serde_json::Number::from(
            usage_payload
                .get("total_tokens")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        )),
    );
    usage.insert(
        "mode".to_string(),
        Value::String(
            usage_payload
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("live")
                .to_string(),
        ),
    );

    let mut response = Map::new();
    response.insert(
        "session_id".to_string(),
        Value::String(required_string_field(
            payload,
            "session_id",
            "router-rs execute response",
        )?),
    );
    response.insert(
        "user_id".to_string(),
        Value::String(required_string_field(
            payload,
            "user_id",
            "router-rs execute response",
        )?),
    );
    response.insert(
        "skill".to_string(),
        Value::String(required_string_field(
            payload,
            "skill",
            "router-rs execute response",
        )?),
    );
    response.insert(
        "overlay".to_string(),
        optional_string_field(payload, "overlay")
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    response.insert("live_run".to_string(), Value::Bool(live_run));
    response.insert(
        "content".to_string(),
        Value::String(optional_string_field(payload, "content").unwrap_or_default()),
    );
    response.insert("usage".to_string(), Value::Object(usage));
    response.insert(
        "prompt_preview".to_string(),
        optional_string_field(payload, "prompt_preview")
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    response.insert(
        "model_id".to_string(),
        optional_string_field(payload, "model_id")
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    response.insert("metadata".to_string(), Value::Object(metadata));
    Ok(Value::Object(response))
}
