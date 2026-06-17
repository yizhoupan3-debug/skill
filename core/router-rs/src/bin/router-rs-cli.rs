use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use clap::Parser;

fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off")),
        )
        .with_target(false)
        .init();
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.len() > 1
        && let Some(cmd) = args[1].to_str() {
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
                let action = match args.get(2).and_then(|s| s.to_str()) {
                    Some("agent" | "Agent") => "agent",
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
    router_rs::init_browser_mcp_dispatch();
    router_rs::cli::run(&args)
}
