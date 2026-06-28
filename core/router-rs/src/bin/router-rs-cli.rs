use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use clap::Parser;

#[cfg(feature = "browser")]
/// Wire browser-mcp dispatch hooks between core/browser-mcp-dispatch and tools/browser-mcp.
/// Called once at startup before any `router-rs browser` CLI command.
fn init_browser_mcp_dispatch() {
    // Wrapper functions to convert FrameworkError -> String for the BrowserMcpHooks struct.
    fn attach_runtime_event_transport_wrapper(
        v: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, String> {
        Ok(host_projection::hooks::attach_runtime_event_transport(v)?)
    }
    fn inspect_trace_stream_wrapper(
        req: framework_kernel::stdio_payload_types::TraceStreamInspectRequestPayload,
    ) -> std::result::Result<
        framework_kernel::stdio_payload_types::TraceStreamInspectResponsePayload,
        String,
    > {
        Ok(host_projection::hooks::inspect_trace_stream(req)?)
    }
    browser_mcp_dispatch::set_hooks(browser_mcp_dispatch::BrowserMcpHooks {
        evaluate_mcp_pre_guard: |tool, args, repo| {
            let v = host_projection::hooks::evaluate_mcp_pre_guard_safe(tool, args, repo);
            browser_mcp_dispatch::McpPreGuardVerdict {
                blocked: v.blocked,
                reason: v.reason,
            }
        },
        attach_runtime_event_transport: attach_runtime_event_transport_wrapper,
        inspect_trace_stream: inspect_trace_stream_wrapper,
    });
    // Adapter: browser-mcp returns anyhow::Error; dispatch hook expects String.
    host_projection::hooks::modify_runtime_hooks(|hooks| {
        hooks.browser_dispatch =
            |cmd| Ok(browser_mcp::dispatch_browser_command(cmd).map_err(|e| e.to_string())?);
    });
}

#[cfg(not(feature = "browser"))]
fn init_browser_mcp_dispatch() {
    // No-op: browser-mcp is not available (feature disabled).
    // `router-rs browser` CLI commands will return an error via
    // host_projection::hooks::dispatch_browser_command's fallback.
}

fn main() -> Result<(), String> {
    // Wire research-harness gate checkers into the QG Route bridge (before init_hooks).
    #[cfg(feature = "research")]
    runtime_core::qg_route::set_extern_checkers(research_harness::register_qg_checkers);

    // Explicit hook initialization (deterministic ordering, testable).
    // runtime_core::init_hooks() uses OnceLock internally for safety.
    runtime_core::init_hooks();

    // Register research hooks if the `research` feature is enabled.
    // This keeps runtime-core (L4) decoupled from research-harness (L5)
    // — research hooks register themselves directly into the L0 registry.
    #[cfg(feature = "research")]
    research_harness::init_hooks();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off")),
        )
        .with_target(false)
        .init();
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.len() > 1
        && let Some(cmd) = args[1].to_str()
    {
        let cmd_lower = cmd.trim().to_ascii_lowercase();
        let is_host_alias = runtime_core::hosts::host_provider_registry()
            .iter()
            .any(|p| {
                p.host_id() == cmd_lower
                    || p.install_tool() == cmd_lower
                    || p.aliases().iter().any(|a| *a == cmd_lower)
            });
        if is_host_alias {
            // Map old-style `router-rs <host> <subcommand>` to registry-driven
            // `router-rs host <action> <host-id> <subcommand>`.
            // Determine action (hook/agent) from the subcommand or default.
            // Consume the original subcommand word when it was "agent" to
            // avoid duplicating it after the insertions below.
            let action = match args.get(2).and_then(|s| s.to_str()) {
                Some(s) if s.to_ascii_lowercase() == "agent" => {
                    args.remove(2);
                    "agent"
                }
                _ => "hook",
            };
            // Replace the host alias with the expanded form
            let host_id = args[1].clone();
            args[1] = std::ffi::OsString::from("host");
            args.insert(2, std::ffi::OsString::from(action));
            args.insert(3, host_id);
        }
    }
    let args = router_rs::cli::Cli::parse_from(args);
    init_browser_mcp_dispatch();
    router_rs::cli::run(&args).map_err(|e| e.to_string())
}
