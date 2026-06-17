//! 子命令薄分发壳：解析后的 `RouterCommand` → `cli::router_command_dispatch`。

use super::args::*;
use crate::framework_runtime::print_json_value;
use crate::framework_runtime::route_manifest_fallback::{
    manifest_fallback_path, route_task_with_manifest_fallback,
};
#[cfg(feature = "codegraph")]
use super::router_command_dispatch::dispatch_codegraph_command;
use super::router_command_dispatch::{
    dispatch_browser_command, dispatch_closeout_command, dispatch_diagnose_command,
    dispatch_eval_command, dispatch_framework_command, dispatch_hook_policy_command,
    dispatch_host_command, dispatch_loop_command, dispatch_migrate_command,
    dispatch_schema_drift_command, dispatch_storage_command, dispatch_trace_command,
};
use crate::route::{
    MatchRow, SearchResultsPayload, build_search_results_payload, filter_record_indices_for_host,
    filter_records_for_host, load_records, load_records_cached_for_stdio,
    load_records_from_manifest, search_skills_subset,
};
use crate::router_self;

#[tracing::instrument(name = "dispatch", skip_all, ret)]
pub fn dispatch_router_command(command: RouterCommand) -> Result<(), String> {
    match command {
        RouterCommand::Route(command) => {
            let records = load_records(command.runtime.as_deref(), command.manifest.as_deref())?;
            let records = filter_records_for_host(records, command.host_id.as_deref())?;
            let decision = route_task_with_manifest_fallback(
                &records,
                command.runtime.as_deref(),
                command.manifest.as_deref(),
                command.host_id.as_deref(),
                &command.query,
                &command.session_id,
                command.allow_overlay,
                command.first_turn,
            )?;
            print_json_value(&decision)
        }
        RouterCommand::Search(command) => {
            let manifest_path =
                manifest_fallback_path(command.runtime.as_deref(), command.manifest.as_deref())?;
            let records = if let Some(path) = manifest_path.as_deref() {
                load_records_from_manifest(path)?
            } else {
                load_records_cached_for_stdio(
                    command.runtime.as_deref(),
                    command.manifest.as_deref(),
                )?
                .as_ref()
                .clone()
            };
            let host_indices =
                filter_record_indices_for_host(&records, command.host_id.as_deref())?;
            let rows =
                search_skills_subset(&records, Some(&host_indices), &command.query, command.limit);
            let payload = build_search_results_payload(&command.query, rows.clone());
            if command.json {
                return print_json_value(&payload);
            }
            print_search_results(&command.query, &payload, rows);
            Ok(())
        }
        RouterCommand::Framework { command } => dispatch_framework_command(command),
        RouterCommand::Host { command } => dispatch_host_command(command),
        RouterCommand::Trace { command } => dispatch_trace_command(command),
        RouterCommand::Storage { command } => dispatch_storage_command(command),
        RouterCommand::Browser { command } => dispatch_browser_command(command),
        #[cfg(feature = "codegraph")]
        RouterCommand::Codegraph { command } => dispatch_codegraph_command(command),
        RouterCommand::Diagnose { command } => dispatch_diagnose_command(command),
        RouterCommand::Migrate { command } => dispatch_migrate_command(command),
        RouterCommand::HookPolicy { command } => dispatch_hook_policy_command(command),
        RouterCommand::Closeout { command } => dispatch_closeout_command(command),
        RouterCommand::Loop { command } => dispatch_loop_command(command),
        RouterCommand::Eval { command } => dispatch_eval_command(command),
        RouterCommand::SchemaDrift { command } => dispatch_schema_drift_command(command),
        RouterCommand::RouterSelf { command } => router_self::dispatch(command),
    }
}

fn print_search_results(query: &str, payload: &SearchResultsPayload, rows: Vec<MatchRow>) {
    if payload.matches.is_empty() {
        println!("No skills found matching: {}", query);
        return;
    }

    println!("Found {} matches for '{}':", payload.matches.len(), query);
    println!();
    println!(
        "{:<30} | {:<5} | {:<10} | {:<6} | Description",
        "Skill", "Layer", "Gate", "Score"
    );
    println!("{}", "-".repeat(120));
    for row in rows {
        let mut description = row.description.clone();
        if description.chars().count() > 60 {
            description = description.chars().take(57).collect::<String>() + "...";
        }
        println!(
            "{:<30} | {:<5} | {:<10} | {:<6.2} | {}",
            row.slug, row.layer, row.gate, row.score, description
        );
    }
}
