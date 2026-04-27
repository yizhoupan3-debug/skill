use super::*;

struct RuntimeOutputMode {
    stdio_op: Option<&'static str>,
    run_stdio: fn(Value) -> Result<Value, String>,
}

impl RuntimeOutputMode {
    fn run_stdio(&self, payload: Value) -> Result<Value, String> {
        (self.run_stdio)(payload)
    }
}

const RUNTIME_OUTPUT_MODES: &[RuntimeOutputMode] = &[
    RuntimeOutputMode {
        stdio_op: Some("runtime_integrator"),
        run_stdio: |_| Ok(build_runtime_integrator_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_control_plane"),
        run_stdio: |_| Ok(build_runtime_control_plane_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_exporter_descriptor"),
        run_stdio: |_| Ok(build_runtime_observability_exporter_descriptor()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_metric_catalog"),
        run_stdio: |_| Ok(build_runtime_observability_metric_catalog_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_dashboard_schema"),
        run_stdio: |_| Ok(runtime_observability_dashboard_schema()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_metric_record"),
        run_stdio: |payload| build_runtime_metric_record(payload),
    },
];

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
