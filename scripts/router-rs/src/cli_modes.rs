use super::*;

struct RuntimeOutputMode {
    stdio_op: Option<&'static str>,
    enabled: fn(&Cli) -> bool,
    run_cli: fn(&Cli) -> Result<Value, String>,
    run_stdio: fn(Value) -> Result<Value, String>,
}

impl RuntimeOutputMode {
    fn enabled(&self, args: &Cli) -> bool {
        (self.enabled)(args)
    }

    fn run_stdio(&self, payload: Value) -> Result<Value, String> {
        (self.run_stdio)(payload)
    }
}

const RUNTIME_OUTPUT_MODES: &[RuntimeOutputMode] = &[
    RuntimeOutputMode {
        stdio_op: Some("runtime_integrator"),
        enabled: |args| args.runtime_integrator_json,
        run_cli: cli_runtime_integrator_output,
        run_stdio: |_| Ok(build_runtime_integrator_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_control_plane"),
        enabled: |args| args.runtime_control_plane_json,
        run_cli: cli_runtime_control_plane_output,
        run_stdio: |_| Ok(build_runtime_control_plane_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_exporter_descriptor"),
        enabled: |args| args.runtime_observability_exporter_json,
        run_cli: cli_runtime_observability_exporter_output,
        run_stdio: |_| Ok(build_runtime_observability_exporter_descriptor()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_metric_catalog"),
        enabled: |args| args.runtime_observability_metric_catalog_json,
        run_cli: cli_runtime_observability_metric_catalog_output,
        run_stdio: |_| Ok(build_runtime_observability_metric_catalog_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_dashboard_schema"),
        enabled: |args| args.runtime_observability_dashboard_json,
        run_cli: cli_runtime_observability_dashboard_output,
        run_stdio: |_| Ok(runtime_observability_dashboard_schema()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_metric_record"),
        enabled: |args| args.runtime_metric_record_json,
        run_cli: cli_runtime_metric_record_output,
        run_stdio: |payload| build_runtime_metric_record(payload),
    },
];

pub(crate) fn enabled_runtime_output_mode_count(args: &Cli) -> usize {
    RUNTIME_OUTPUT_MODES
        .iter()
        .filter(|mode| mode.enabled(args))
        .count()
}

pub(crate) fn run_runtime_output_mode_cli(args: &Cli) -> Result<bool, String> {
    if let Some(mode) = RUNTIME_OUTPUT_MODES.iter().find(|mode| mode.enabled(args)) {
        let payload = (mode.run_cli)(args)?;
        print_json_value(&payload)?;
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn dispatch_runtime_output_mode_stdio(
    op: &str,
    payload: Value,
) -> Option<Result<Value, String>> {
    RUNTIME_OUTPUT_MODES
        .iter()
        .find(|mode| mode.stdio_op == Some(op))
        .map(|mode| mode.run_stdio(payload))
}

pub(crate) fn handles_runtime_output_stdio_op(op: &str) -> bool {
    RUNTIME_OUTPUT_MODES
        .iter()
        .any(|mode| mode.stdio_op == Some(op))
}

fn cli_runtime_integrator_output(_args: &Cli) -> Result<Value, String> {
    Ok(build_runtime_integrator_payload())
}

fn cli_runtime_control_plane_output(_args: &Cli) -> Result<Value, String> {
    Ok(build_runtime_control_plane_payload())
}

fn cli_runtime_observability_exporter_output(_args: &Cli) -> Result<Value, String> {
    Ok(build_runtime_observability_exporter_descriptor())
}

fn cli_runtime_observability_metric_catalog_output(_args: &Cli) -> Result<Value, String> {
    Ok(build_runtime_observability_metric_catalog_payload())
}

fn cli_runtime_observability_dashboard_output(_args: &Cli) -> Result<Value, String> {
    Ok(runtime_observability_dashboard_schema())
}

fn cli_runtime_metric_record_output(args: &Cli) -> Result<Value, String> {
    let payload = serde_json::from_str::<Value>(
        args.runtime_metric_record_input_json
            .as_deref()
            .ok_or_else(|| {
                "--runtime-metric-record-input-json is required with --runtime-metric-record-json"
                    .to_string()
            })?,
    )
    .map_err(|err| format!("parse runtime metric record input failed: {err}"))?;
    build_runtime_metric_record(payload)
}
