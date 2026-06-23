//! CLI top-level subcommand backward compatibility (parse-only smoke).

use clap::Parser;

use crate::cli::Cli;
use crate::cli::args::{
    CloseoutCommand, FrameworkCommand, HookPolicyCommand, HostCommand, RouterCommand,
};
use crate::router_self::RouterSelfCommands;

/// Canonical top-level `router-rs` subcommands must keep parsing (binary name + shape stable).
#[test]
fn cli_backward_compat_top_level_commands_smoke() {
    let argv_sets: &[&[&str]] = &[
        &["router-rs", "route", "plan mode"],
        &["router-rs", "search", "workflow", "--limit", "3"],
        &["router-rs", "framework", "doctor", "--repo-root", "."],
        &["router-rs", "host", "hook", "cursor", "--event", "stop"],
        &[
            "router-rs",
            "host",
            "hook",
            "claude",
            "--event",
            "PreToolUse",
        ],
        &["router-rs", "host", "agent", "opencode"],
        &["router-rs", "storage", "backend-catalog"],
        &[
            "router-rs",
            "diagnose",
            "profile",
            "artifacts",
            "--framework-profile",
            "/tmp/smoke.json",
        ],
        &["router-rs", "hook-policy", "contract"],
        &["router-rs", "closeout", "contract"],
        &["router-rs", "eval", "route-contract"],
        &["router-rs", "schema-drift", "contract"],
        &["router-rs", "self", "clean"],
        &[
            "router-rs",
            "self",
            "install",
            "--bin-dir",
            "/tmp/smoke-router-bin",
        ],
        &["router-rs", "--stdio-json"],
    ];

    for argv in argv_sets {
        let cli = Cli::try_parse_from(*argv).unwrap_or_else(|err| {
            panic!("backward-compat parse failed for {argv:?}: {err}");
        });
        if argv.contains(&"--stdio-json") {
            assert!(cli.stdio_json, "stdio-json flag must remain available");
            continue;
        }
        let Some(command) = cli.command else {
            panic!("expected subcommand for {argv:?}");
        };
        match command {
            RouterCommand::Route(_) | RouterCommand::Search(_) => {}
            RouterCommand::Framework {
                command: FrameworkCommand::Doctor(_),
            } => {}
            RouterCommand::Host { command } => match command {
                HostCommand::Hook { .. }
                | HostCommand::Agent { .. } => {}
            },
            RouterCommand::Storage { .. } | RouterCommand::Diagnose { .. } => {}
            RouterCommand::HookPolicy {
                command: HookPolicyCommand::Contract,
            } => {}
            RouterCommand::Closeout {
                command: CloseoutCommand::Contract,
            } => {}
            RouterCommand::Eval { .. } | RouterCommand::SchemaDrift { .. } => {}
            RouterCommand::RouterSelf {
                command: RouterSelfCommands::Clean(_),
            } => {}
            RouterCommand::RouterSelf {
                command: RouterSelfCommands::Install(_),
            } => {}
            other => panic!("unexpected top-level command in smoke: {other:?}"),
        }
    }
}
