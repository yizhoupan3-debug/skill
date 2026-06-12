use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use clap::Parser;

fn main() -> Result<(), String> {
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.len() > 1 {
        if let Some(cmd) = args[1].to_str() {
            let cmd_lower = cmd.trim().to_ascii_lowercase();
            let is_host_alias = runtime_core::hosts::host_provider_registry().iter().any(|p| {
                p.host_id() == cmd_lower
                    || p.install_tool() == cmd_lower
                    || p.aliases().iter().any(|a| *a == cmd_lower)
            });
            if is_host_alias {
                args.insert(1, std::ffi::OsString::from("host"));
            }
        }
    }
    let args = router_rs::cli::Cli::parse_from(args);
    router_rs::init_browser_mcp_dispatch();
    router_rs::cli::run(&args)
}
