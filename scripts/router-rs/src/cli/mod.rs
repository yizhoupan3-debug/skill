//! CLI 子命令、stdio 与 live execute 控制面（从 `main.rs` 拆分，阶段 1）。

pub(crate) mod args;
pub(crate) mod common;
mod dispatch;
pub(crate) mod runtime_ops;

pub(crate) use common::{
    configure_compute_parallelism, env_usize, route_task_with_manifest_fallback,
};
pub(crate) use runtime_ops::dispatch_stdio_json_request_payload;

#[cfg(test)]
pub(crate) use runtime_ops::{
    classify_stdio_op, dispatch_stdio_json_request, is_framework_stdio_op, is_routing_stdio_op,
    is_runtime_stdio_op, is_trace_stdio_op, StdioOpDomain,
};

pub(crate) fn run(args: &args::Cli) -> Result<(), String> {
    configure_compute_parallelism(args.compute_threads)?;
    if let Some(command) = args.command.clone() {
        return dispatch::dispatch_router_command(command);
    }
    if args.stdio_json {
        return crate::stdio_transport::run_stdio_json_loop(args.stdio_max_concurrency);
    }
    Err("missing router-rs command; use `router-rs --help` for canonical subcommands".to_string())
}

pub(crate) use args::Cli;
