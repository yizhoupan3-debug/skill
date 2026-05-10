//! Neutral entry for Cursor hook JSON stdin → review/subagent gate (implementation in `cursor_hooks`).

use std::io::Write;
use std::path::Path;

pub fn run_review_gate(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    let payload = crate::cursor_hooks::read_cursor_hook_stdin_json()?;
    let repo_root = crate::cursor_hooks::resolve_cursor_hook_repo_root(cli_repo_root, &payload)?;
    let mut output = crate::cursor_hooks::dispatch_cursor_hook_event(&repo_root, event, &payload);
    crate::autopilot_goal::scrub_followup_fields_in_hook_output(&mut output);
    crate::cursor_hooks::apply_cursor_hook_output_policy(&mut output);
    let mut stdout = std::io::stdout();
    let serialized = serde_json::to_string(&output).map_err(|e| e.to_string())?;
    stdout
        .write_all(format!("{serialized}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}
