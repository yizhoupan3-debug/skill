use super::common::*;
use super::*;

use serde_json::{json, Map, Value};
use crate::integration_test_prelude::*;


#[test]
fn cli_parses_codex_hook_with_event_flag() {
    let cli = Cli::try_parse_from(["router-rs", "host", "codex", "hook", "--event", "Stop"])
        .expect("parse host codex hook --event");
    let Some(RouterCommand::Host {
        command: HostCommand::Codex {
            command: CodexSubcommand::Hook(command),
        },
    }) = cli.command
    else {
        panic!("expected host codex hook command");
    };
    assert_eq!(command.event.as_deref(), Some("Stop"));
    assert_eq!(command.name.as_deref(), None);
}


#[test]
fn cli_parses_codex_hook_with_positional() {
    let cli = Cli::try_parse_from(["router-rs", "host", "codex", "hook", "Stop"])
        .expect("parse host codex hook");
    let Some(RouterCommand::Host {
        command: HostCommand::Codex {
            command: CodexSubcommand::Hook(command),
        },
    }) = cli.command
    else {
        panic!("expected host codex hook command");
    };
    assert_eq!(command.name.as_deref(), Some("Stop"));
    assert_eq!(command.event.as_deref(), None);
}


#[test]
fn install_hooks_cli_repo_root_optional() {
    let cli = Cli::try_parse_from(["router-rs", "host", "codex", "install-hooks"])
        .expect("parse host codex install-hooks without repo-root");
    let Some(RouterCommand::Host {
        command:
            HostCommand::Codex {
                command: CodexSubcommand::InstallHooks(command),
            },
    }) = cli.command
    else {
        panic!("expected host codex install-hooks command");
    };
    assert!(command.repo_root.is_none());
}


#[test]
fn hook_status_constants_are_stable() {
    assert_eq!(
        hook_status::REVIEW_GATE_CHECKING,
        "Loading Codex turn context"
    );
    assert_eq!(
        hook_status::REVIEW_GATE_UPDATING,
        "Recording Codex tool evidence"
    );
    assert_eq!(
        hook_status::REVIEW_GATE_ENFORCING,
        "Enforcing Codex review gate"
    );
}


