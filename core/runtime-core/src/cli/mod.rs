//! CLI 子命令、stdio 与 live execute 控制面（从 `main.rs` 拆分，阶段 1）。

pub mod args;
pub mod common;
mod dispatch;
pub mod router_command_dispatch;
pub mod runtime_ops;

pub use common::{
    configure_compute_parallelism, env_usize,
};
pub use crate::framework_runtime::route_manifest_fallback::{
    manifest_fallback_path, resolve_runtime_declared_manifest_fallback,
    route_task_with_manifest_fallback,
};
pub use runtime_ops::dispatch_stdio_json_request_payload;

#[cfg(not(test))]
pub use crate::framework_runtime::stdio_op_registry::StdioOpDomain;
pub use crate::framework_runtime::stdio_op_registry::classify_stdio_op;
#[cfg(test)]
pub use crate::framework_runtime::stdio_op_registry::{
    StdioOpDomain, is_framework_stdio_op, is_routing_stdio_op, is_runtime_stdio_op,
    is_trace_stdio_op,
};

#[tracing::instrument(name = "router-rs", skip_all, ret)]
pub fn run(args: &args::Cli) -> Result<(), String> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    configure_compute_parallelism(args.compute_threads)?;
    if let Some(command) = args.command.clone() {
        return dispatch::dispatch_router_command(command);
    }
    if args.stdio_json {
        return crate::stdio_transport::run_stdio_json_loop(args.stdio_max_concurrency);
    }
    Err("missing router-rs command; use `router-rs --help` for canonical subcommands".to_string())
}

pub use args::Cli;

/// 薄 CLI 壳行数预算（不含 `runtime_ops` stdio 控制面）。
pub const CLI_THIN_SHELL_LINE_BUDGET: usize = 1500;

/// `runtime_ops.inc` 行数上限（P7 增量切片：全量迁入 B3 前只减不增）。
pub const RUNTIME_OPS_INC_LINE_CEILING: usize = 3400;

pub fn cli_thin_shell_line_count() -> usize {
    const MAIN: &str = ""; // main.rs lives in router-rs, not runtime-core
    const MOD_RS: &str = include_str!("mod.rs");
    const DISPATCH: &str = include_str!("dispatch.rs");
    const ARGS: &str = include_str!("args.rs");
    const COMMON_RS: &str = include_str!("common.rs");
    const COMMON_INC: &str = include_str!("common.inc");
    [MAIN, MOD_RS, DISPATCH, ARGS, COMMON_RS, COMMON_INC]
        .iter()
        .map(|src| src.lines().count())
        .sum()
}

#[cfg(test)]
mod cli_thin_shell_budget_tests {
    use super::{
        CLI_THIN_SHELL_LINE_BUDGET, cli_thin_shell_line_count,
    };

    #[test]
    fn b7_cli_thin_shell_under_line_budget() {
        let lines = cli_thin_shell_line_count();
        assert!(
            lines < CLI_THIN_SHELL_LINE_BUDGET,
            "B7 CLI thin shell is {lines} lines (budget < {CLI_THIN_SHELL_LINE_BUDGET})"
        );
    }

    #[test]
    fn p7_runtime_ops_inc_under_line_ceiling() {
        let ceiling = super::RUNTIME_OPS_INC_LINE_CEILING;
        let lines = include_str!("runtime_ops.inc").lines().count();
        assert!(
            lines <= ceiling,
            "runtime_ops.inc is {lines} lines (ceiling {ceiling}); \
             migrate chunks to framework_runtime/ before growing"
        );
    }
}
