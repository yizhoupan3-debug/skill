#[cfg(feature = "mimalloc")]
use mimalloc::MiMalloc;
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use clap::Parser;

fn main() -> Result<(), String> {
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.len() > 1 {
        if let Some(cmd) = args[1].to_str() {
            if cmd == "codex"
                || cmd == "claude"
                || cmd == "cursor"
                || cmd == "antigravity-app"
                || cmd == "opencode"
            {
                args.insert(1, std::ffi::OsString::from("host"));
            }
        }
    }
    let args = router_rs::cli::Cli::parse_from(args);
    router_rs::cli::run(&args)
}
