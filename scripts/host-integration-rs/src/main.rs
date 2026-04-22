use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs as unix_fs;

const CONFIG_SCHEMA_HEADER: &str =
    "#:schema https://developers.openai.com/codex/config-schema.json\n";
const FRAMEWORK_START_MARKER: &str = "<!-- FRAMEWORK_DEFAULT_RUNTIME_START -->";
const FRAMEWORK_END_MARKER: &str = "<!-- FRAMEWORK_DEFAULT_RUNTIME_END -->";
const PLUGIN_NAME: &str = "skill-framework-native";
const DEFAULT_TUI_STATUS_ITEMS: [&str; 4] = [
    "model-with-reasoning",
    "git-branch",
    "context-used",
    "fast-mode",
];

const FULL_SYNC_TEXT_FILES: [&str; 15] = [
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".claude/agents/README.md",
    ".claude/commands/refresh.md",
    ".claude/commands/background_batch.md",
    ".claude/hooks/README.md",
    ".claude/hooks/session_start.sh",
    ".claude/hooks/stop.sh",
    ".claude/hooks/pre_compact.sh",
    ".claude/hooks/subagent_stop.sh",
    ".claude/hooks/session_end.sh",
    ".claude/hooks/config_change.sh",
    ".claude/hooks/stop_failure.sh",
];

const FULL_SYNC_JSON_FILES: [&str; 2] = [".claude/settings.json", ".gemini/settings.json"];
const FULL_SYNC_MANAGED_DIRS: [&str; 6] = [
    ".claude",
    ".claude/agents",
    ".claude/commands",
    ".claude/hooks",
    ".gemini",
    ".codex",
];
const PARTIAL_SYNC_TEXT_FILES: [&str; 2] = [
    ".claude/commands/refresh.md",
    ".claude/commands/background_batch.md",
];
const PARTIAL_SYNC_MANAGED_DIRS: [&str; 2] = [".claude", ".claude/commands"];
const RETIRED_PATHS: [&str; 6] = [
    ".claude/CLAUDE.md",
    ".codex/model_instructions.md",
    ".mcp.json",
    "configs/codex/AGENTS.md",
    "configs/claude/CLAUDE.md",
    "configs/gemini/GEMINI.md",
];

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    SyncHostEntrypoints {
        #[arg(long)]
        template_root: PathBuf,
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long, conflicts_with = "check")]
        apply: bool,
        #[arg(long, conflicts_with = "apply")]
        check: bool,
    },
    InstallNativeIntegration {
        #[arg(long)]
        template_root: PathBuf,
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        home_config_path: PathBuf,
        #[arg(long)]
        home_plugin_root: PathBuf,
        #[arg(long)]
        home_marketplace_path: PathBuf,
        #[arg(long)]
        home_codex_skills_path: PathBuf,
        #[arg(long)]
        home_claude_refresh_path: PathBuf,
        #[arg(long)]
        project_instructions_path: PathBuf,
        #[arg(long)]
        skip_browser_mcp: bool,
        #[arg(long)]
        skip_framework_mcp: bool,
        #[arg(long)]
        skip_framework_overlay_retirement: bool,
        #[arg(long)]
        skip_personal_plugin: bool,
        #[arg(long)]
        skip_personal_marketplace: bool,
        #[arg(long)]
        skip_home_codex_skills_link: bool,
        #[arg(long)]
        skip_home_claude_refresh: bool,
    },
}

#[derive(Default, Serialize)]
struct SyncReport {
    written: Vec<String>,
    unchanged: Vec<String>,
    created_dirs: Vec<String>,
    synced_worktrees: Vec<String>,
    skipped_worktrees: Vec<String>,
}

#[derive(Default)]
struct SingleSyncReport {
    written: Vec<String>,
    unchanged: Vec<String>,
    created_dirs: Vec<String>,
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let payload = match cli.command {
        Commands::SyncHostEntrypoints {
            template_root,
            repo_root,
            apply,
            check: _,
        } => serde_json::to_value(sync_host_entrypoints(&template_root, &repo_root, apply)?)
            .map_err(|err| err.to_string())?,
        Commands::InstallNativeIntegration {
            template_root,
            repo_root,
            home_config_path,
            home_plugin_root,
            home_marketplace_path,
            home_codex_skills_path,
            home_claude_refresh_path,
            project_instructions_path,
            skip_browser_mcp,
            skip_framework_mcp,
            skip_framework_overlay_retirement,
            skip_personal_plugin,
            skip_personal_marketplace,
            skip_home_codex_skills_link,
            skip_home_claude_refresh,
        } => install_native_integration(
            &template_root,
            &repo_root,
            &home_config_path,
            &home_plugin_root,
            &home_marketplace_path,
            &home_codex_skills_path,
            &home_claude_refresh_path,
            &project_instructions_path,
            !skip_browser_mcp,
            !skip_framework_mcp,
            !skip_framework_overlay_retirement,
            !skip_personal_plugin,
            !skip_personal_marketplace,
            !skip_home_codex_skills_link,
            !skip_home_claude_refresh,
        )?,
    };
    let stdout = serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())?;
    println!("{stdout}");
    Ok(())
}

fn sync_host_entrypoints(
    template_root: &Path,
    repo_root: &Path,
    apply: bool,
) -> Result<SyncReport, String> {
    let root = normalize_path(repo_root)?;
    let template = normalize_path(template_root)?;
    let (matched_worktrees, skipped_worktrees) = discover_matching_worktrees(&root);
    let mut report = SyncReport {
        skipped_worktrees,
        ..SyncReport::default()
    };
    let mut targets = vec![root.clone()];
    targets.extend(matched_worktrees);

    for target_root in targets {
        let single = sync_single_root(&template, &target_root, &root, apply, target_root == root)?;
        report.written.extend(single.written);
        report.unchanged.extend(single.unchanged);
        report.created_dirs.extend(single.created_dirs);
        if target_root != root {
            report
                .synced_worktrees
                .push(target_root.to_string_lossy().into_owned());
        }
    }

    report.written.sort();
    report.unchanged.sort();
    report.created_dirs.sort();
    report.synced_worktrees.sort();
    report.skipped_worktrees.sort();
    Ok(report)
}

fn sync_single_root(
    template_root: &Path,
    target_root: &Path,
    report_root: &Path,
    apply: bool,
    full_sync: bool,
) -> Result<SingleSyncReport, String> {
    let mut report = SingleSyncReport::default();
    let managed_dirs = if full_sync {
        FULL_SYNC_MANAGED_DIRS.as_slice()
    } else {
        PARTIAL_SYNC_MANAGED_DIRS.as_slice()
    };
    for relative in managed_dirs {
        let directory = target_root.join(relative);
        if !directory.exists() {
            if apply {
                fs::create_dir_all(&directory).map_err(|err| err.to_string())?;
            }
            report
                .created_dirs
                .push(describe_path(report_root, target_root, &directory));
        }
    }

    let text_files = if full_sync {
        FULL_SYNC_TEXT_FILES.as_slice()
    } else {
        PARTIAL_SYNC_TEXT_FILES.as_slice()
    };
    for relative in text_files {
        sync_template_file(
            &template_root.join(relative),
            &target_root.join(relative),
            report_root,
            target_root,
            apply,
            &mut report,
        )?;
    }

    if full_sync {
        for relative in FULL_SYNC_JSON_FILES {
            sync_template_file(
                &template_root.join(relative),
                &target_root.join(relative),
                report_root,
                target_root,
                apply,
                &mut report,
            )?;
        }
        for relative in RETIRED_PATHS {
            let path = target_root.join(relative);
            let exists = path.exists() || symlink_exists(&path);
            if exists && apply {
                remove_path(&path).map_err(|err| err.to_string())?;
            }
            if exists {
                report
                    .written
                    .push(describe_path(report_root, target_root, &path));
            }
        }
    }

    Ok(report)
}

fn sync_template_file(
    source: &Path,
    destination: &Path,
    report_root: &Path,
    target_root: &Path,
    apply: bool,
    report: &mut SingleSyncReport,
) -> Result<(), String> {
    let desired = fs::read(source).map_err(|err| err.to_string())?;
    let existing = fs::read(destination).ok();
    let changed = existing.as_ref() != Some(&desired);
    if changed && apply {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(destination, desired).map_err(|err| err.to_string())?;
    }
    let bucket = if changed {
        &mut report.written
    } else {
        &mut report.unchanged
    };
    bucket.push(describe_path(report_root, target_root, destination));
    Ok(())
}

fn discover_matching_worktrees(root: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let root_head = read_git_stdout(root, &["rev-parse", "HEAD"]);
    let worktree_listing = read_git_stdout(root, &["worktree", "list", "--porcelain"]);
    if root_head.is_none() || worktree_listing.is_none() {
        return (Vec::new(), Vec::new());
    }

    let normalized_root_head = root_head.unwrap_or_default().trim().to_string();
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    let mut worktrees: Vec<BTreeMap<String, String>> = Vec::new();
    for raw_line in worktree_listing.unwrap_or_default().lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                worktrees.push(current);
                current = BTreeMap::new();
            }
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or_default().to_string();
        let value = parts.next().unwrap_or_default().to_string();
        current.insert(key, value);
    }
    if !current.is_empty() {
        worktrees.push(current);
    }

    let mut matches = Vec::new();
    let mut skipped = Vec::new();
    for entry in worktrees {
        let Some(worktree_path) = entry.get("worktree") else {
            continue;
        };
        let candidate = normalize_path(Path::new(worktree_path))
            .unwrap_or_else(|_| PathBuf::from(worktree_path));
        if candidate == root {
            continue;
        }
        if !candidate.exists() {
            skipped.push(format!("{} (missing)", candidate.to_string_lossy()));
            continue;
        }
        if entry.get("HEAD").map(|value| value.trim()) != Some(normalized_root_head.as_str()) {
            skipped.push(format!("{} (head mismatch)", candidate.to_string_lossy()));
            continue;
        }
        matches.push(candidate);
    }
    (matches, skipped)
}

fn read_git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn normalize_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|err| err.to_string())?
            .join(path))
    }
}

fn describe_path(report_root: &Path, target_root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(report_root) {
        return relative.to_string_lossy().into_owned();
    }
    if let Ok(relative) = path.strip_prefix(target_root) {
        return format!(
            "{}::{}",
            target_root.to_string_lossy(),
            relative.to_string_lossy()
        );
    }
    path.to_string_lossy().into_owned()
}

fn install_native_integration(
    template_root: &Path,
    repo_root: &Path,
    home_config_path: &Path,
    home_plugin_root: &Path,
    home_marketplace_path: &Path,
    home_codex_skills_path: &Path,
    home_claude_refresh_path: &Path,
    project_instructions_path: &Path,
    install_browser_mcp: bool,
    install_framework_mcp: bool,
    retire_framework_overlay_file: bool,
    install_personal_plugin: bool,
    install_personal_marketplace_entry: bool,
    install_home_codex_skills_link: bool,
    install_home_claude_refresh_command: bool,
) -> Result<Value, String> {
    let template_root = normalize_path(template_root)?;
    let repo_root = normalize_path(repo_root)?;
    let home_config_path = normalize_path(home_config_path)?;
    let home_plugin_root = normalize_path(home_plugin_root)?;
    let home_marketplace_path = normalize_path(home_marketplace_path)?;
    let home_codex_skills_path = normalize_path(home_codex_skills_path)?;
    let home_claude_refresh_path = normalize_path(home_claude_refresh_path)?;

    let created_config = ensure_config_file(&home_config_path)?;
    let browser_changed = if install_browser_mcp {
        install_mcp_block(
            &home_config_path,
            "[mcp_servers.browser-mcp]",
            &build_browser_server_block(&repo_root),
        )?
    } else {
        false
    };
    let framework_changed = if install_framework_mcp {
        install_mcp_block(
            &home_config_path,
            "[mcp_servers.framework-mcp]",
            &build_framework_server_block(&repo_root),
        )?
    } else {
        false
    };
    let tui_changed = ensure_tui_status_line(&home_config_path)?;
    let personal_plugin_changed = if install_personal_plugin {
        sync_directory(
            &repo_root.join("plugins").join(PLUGIN_NAME),
            &home_plugin_root,
        )?
    } else {
        false
    };
    let personal_marketplace_changed = if install_personal_marketplace_entry {
        install_personal_marketplace(&home_marketplace_path, &home_plugin_root)?
    } else {
        false
    };
    let home_codex_skills_link_changed = if install_home_codex_skills_link {
        ensure_home_codex_skills_link(&repo_root, &home_codex_skills_path)?
    } else {
        false
    };
    let home_claude_refresh_changed = if install_home_claude_refresh_command {
        ensure_home_claude_refresh_command(
            &template_root.join(".claude/commands/refresh.md"),
            &home_claude_refresh_path,
        )?
    } else {
        false
    };
    let framework_overlay_result = if retire_framework_overlay_file {
        retire_overlay(&repo_root.join(project_instructions_path))?
    } else {
        Value::Null
    };

    Ok(json!({
        "success": true,
        "repo_root": repo_root.to_string_lossy(),
        "home_config_path": home_config_path.to_string_lossy(),
        "home_plugin_root": home_plugin_root.to_string_lossy(),
        "home_marketplace_path": home_marketplace_path.to_string_lossy(),
        "home_codex_skills_path": home_codex_skills_path.to_string_lossy(),
        "home_claude_refresh_path": home_claude_refresh_path.to_string_lossy(),
        "repo_marketplace_path": repo_root.join(".agents/plugins/marketplace.json").to_string_lossy(),
        "created_config": created_config,
        "browser_mcp_changed": browser_changed,
        "framework_mcp_changed": framework_changed,
        "tui_status_line_changed": tui_changed,
        "personal_plugin_changed": personal_plugin_changed,
        "personal_marketplace_changed": personal_marketplace_changed,
        "home_codex_skills_link_changed": home_codex_skills_link_changed,
        "home_claude_refresh_changed": home_claude_refresh_changed,
        "framework_overlay_retirement": framework_overlay_result,
    }))
}

fn ensure_config_file(config_path: &Path) -> Result<bool, String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if config_path.exists() {
        return Ok(false);
    }
    fs::write(config_path, CONFIG_SCHEMA_HEADER).map_err(|err| err.to_string())?;
    Ok(true)
}

fn build_browser_server_block(repo_root: &Path) -> String {
    format!(
        "[mcp_servers.browser-mcp]\ncommand = \"{}\"",
        repo_root
            .join("tools/browser-mcp/scripts/start_browser_mcp.sh")
            .to_string_lossy()
    )
}

fn build_framework_server_block(repo_root: &Path) -> String {
    format!(
        "[mcp_servers.framework-mcp]\ncommand = \"python3\"\nargs = [\"-m\", \"scripts.framework_mcp\"]\ncwd = \"{}\"",
        repo_root.to_string_lossy()
    )
}

fn install_mcp_block(config_path: &Path, marker: &str, block: &str) -> Result<bool, String> {
    let content = read_text_if_exists(config_path)?;
    let existing = content.unwrap_or_default();
    if existing.contains(marker) {
        return Ok(false);
    }
    let updated = if existing.trim().is_empty() {
        format!("{block}\n")
    } else {
        format!("{}\n\n{block}\n", existing.trim_end())
    };
    write_text_if_changed(config_path, &updated)
}

fn ensure_tui_status_line(config_path: &Path) -> Result<bool, String> {
    let content = read_text_if_exists(config_path)?.unwrap_or_default();
    let status_line = format_status_line();
    if let Some((start, end)) = find_tui_block_bounds(&content) {
        let block = content[start..end].trim_end_matches('\n');
        let mut replaced = false;
        let mut updated_lines = Vec::new();
        for line in block.lines() {
            if is_status_line(line) {
                updated_lines.push(status_line.clone());
                replaced = true;
            } else {
                updated_lines.push(line.to_string());
            }
        }
        if !replaced {
            updated_lines.push(status_line);
        }
        let new_block = format!("{}\n", updated_lines.join("\n"));
        let updated = format!("{}{}{}", &content[..start], new_block, &content[end..]);
        return write_text_if_changed(config_path, &updated);
    }

    let mut updated = content.trim_end().to_string();
    if !updated.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str("[tui]\n");
    updated.push_str(&format_status_line());
    updated.push('\n');
    write_text_if_changed(config_path, &updated)
}

fn find_tui_block_bounds(content: &str) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    let mut start: Option<usize> = None;
    for line in content.split_inclusive('\n') {
        let normalized = line.trim_end_matches('\n');
        if start.is_none() {
            if normalized == "[tui]" {
                start = Some(offset);
            }
        } else if normalized.starts_with('[') {
            return Some((start.unwrap_or(0), offset));
        }
        offset += line.len();
    }
    start.map(|value| (value, content.len()))
}

fn is_status_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("status_line") && trimmed.contains('=')
}

fn format_status_line() -> String {
    let items = DEFAULT_TUI_STATUS_ITEMS
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("status_line = [{items}]")
}

fn sync_directory(source: &Path, destination: &Path) -> Result<bool, String> {
    if !source.is_dir() {
        return Err(format!(
            "Plugin source directory not found: {}",
            source.to_string_lossy()
        ));
    }
    fs::create_dir_all(destination).map_err(|err| err.to_string())?;
    let mut changed = false;

    let source_children = read_dir_map(source)?;
    let destination_children = read_dir_map(destination)?;

    for (name, stale_path) in destination_children {
        if source_children.contains_key(&name) {
            continue;
        }
        remove_path(&stale_path).map_err(|err| err.to_string())?;
        changed = true;
    }

    for (name, source_path) in source_children {
        let destination_path = destination.join(name);
        if source_path.is_dir() {
            if destination_path.exists() && !destination_path.is_dir() {
                remove_path(&destination_path).map_err(|err| err.to_string())?;
                changed = true;
            }
            changed = sync_directory(&source_path, &destination_path)? || changed;
            continue;
        }
        let source_bytes = fs::read(&source_path).map_err(|err| err.to_string())?;
        let destination_bytes = fs::read(&destination_path).ok();
        if destination_bytes.as_ref() == Some(&source_bytes) {
            continue;
        }
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::copy(&source_path, &destination_path).map_err(|err| err.to_string())?;
        changed = true;
    }

    Ok(changed)
}

fn read_dir_map(root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        entries.insert(
            entry.file_name().to_string_lossy().into_owned(),
            entry.path(),
        );
    }
    Ok(entries)
}

fn ensure_home_codex_skills_link(repo_root: &Path, target_path: &Path) -> Result<bool, String> {
    let source = repo_root.join("skills");
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    if symlink_exists(target_path) {
        let current_target = fs::read_link(target_path).map_err(|err| err.to_string())?;
        let resolved_target = if current_target.is_absolute() {
            current_target
        } else {
            target_path
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join(current_target)
        };
        if resolved_target == source {
            return Ok(false);
        }
        remove_path(target_path).map_err(|err| err.to_string())?;
    } else if target_path.exists() {
        let backup_path = target_path.with_file_name(format!(
            "{}.bak",
            target_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("skills")
        ));
        if backup_path.exists() || symlink_exists(&backup_path) {
            remove_path(&backup_path).map_err(|err| err.to_string())?;
        }
        fs::rename(target_path, &backup_path).map_err(|err| err.to_string())?;
    }

    #[cfg(unix)]
    {
        unix_fs::symlink(&source, target_path).map_err(|err| err.to_string())?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        let _ = source;
        let _ = target_path;
        Err("home codex skills link requires unix symlink support".to_string())
    }
}

fn ensure_home_claude_refresh_command(
    source_path: &Path,
    command_path: &Path,
) -> Result<bool, String> {
    let content = fs::read_to_string(source_path).map_err(|err| err.to_string())?;
    write_text_if_changed(command_path, &content)
}

fn install_personal_marketplace(
    marketplace_path: &Path,
    plugin_root: &Path,
) -> Result<bool, String> {
    let existing = read_json_map_if_exists(marketplace_path)?;
    let relative_base = marketplace_root(marketplace_path)?;
    let payload = build_personal_marketplace_payload(plugin_root, &relative_base, existing)?;
    write_json_if_changed(marketplace_path, &Value::Object(payload))
}

fn marketplace_root(path: &Path) -> Result<PathBuf, String> {
    let absolute = normalize_path(path)?;
    absolute
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "Could not derive marketplace root from {}",
                absolute.to_string_lossy()
            )
        })
}

fn build_personal_marketplace_payload(
    plugin_root: &Path,
    marketplace_root: &Path,
    existing: Option<Map<String, Value>>,
) -> Result<Map<String, Value>, String> {
    let mut payload = existing.unwrap_or_default();
    let plugin_relative = plugin_root
        .strip_prefix(marketplace_root)
        .map_err(|_| {
            format!(
                "Plugin root {} is not under marketplace root {}",
                plugin_root.to_string_lossy(),
                marketplace_root.to_string_lossy()
            )
        })?
        .to_string_lossy()
        .into_owned();
    let plugin_path = format!("./{plugin_relative}");

    payload
        .entry("name".to_string())
        .or_insert_with(|| Value::String("skill-personal-marketplace".to_string()));

    let interface_value = payload
        .remove("interface")
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut interface = match interface_value {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    if !interface.contains_key("displayName") {
        interface.insert(
            "displayName".to_string(),
            Value::String("Skill Personal Marketplace".to_string()),
        );
    }
    payload.insert("interface".to_string(), Value::Object(interface));

    let plugins_value = payload
        .remove("plugins")
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let existing_plugins = match plugins_value {
        Value::Array(items) => items,
        _ => Vec::new(),
    };

    let mut updated_plugins = Vec::new();
    let mut replaced = false;
    for row in existing_plugins {
        let Value::Object(mut row_map) = row else {
            continue;
        };
        if row_map.get("name").and_then(Value::as_str) != Some(PLUGIN_NAME) {
            updated_plugins.push(Value::Object(row_map));
            continue;
        }
        replaced = true;
        let category = row_map
            .remove("category")
            .unwrap_or_else(|| Value::String("Developer Tools".to_string()));
        updated_plugins.push(plugin_marketplace_row(&plugin_path, category));
    }
    if !replaced {
        updated_plugins.push(plugin_marketplace_row(
            &plugin_path,
            Value::String("Developer Tools".to_string()),
        ));
    }
    payload.insert("plugins".to_string(), Value::Array(updated_plugins));
    Ok(payload)
}

fn plugin_marketplace_row(plugin_path: &str, category: Value) -> Value {
    json!({
        "name": PLUGIN_NAME,
        "source": {"source": "local", "path": plugin_path},
        "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
        "category": category,
    })
}

fn retire_overlay(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({
            "success": true,
            "path": path.to_string_lossy(),
            "changed": false,
            "status": "already-retired",
            "retirement_mode": "missing",
        }));
    }
    let original = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let stripped = strip_managed_block(&original).trim().to_string();
    if !stripped.is_empty() {
        let updated = format!("{stripped}\n");
        let changed = updated != original;
        if changed {
            fs::write(path, updated).map_err(|err| err.to_string())?;
        }
        return Ok(json!({
            "success": true,
            "path": path.to_string_lossy(),
            "changed": changed,
            "status": "retired-managed-block",
            "retirement_mode": "preserved-user-content",
        }));
    }
    remove_path(path).map_err(|err| err.to_string())?;
    Ok(json!({
        "success": true,
        "path": path.to_string_lossy(),
        "changed": true,
        "status": "retired-file",
        "retirement_mode": "deleted-empty-overlay",
    }))
}

fn strip_managed_block(text: &str) -> String {
    let start = text.find(FRAMEWORK_START_MARKER);
    let end = text.find(FRAMEWORK_END_MARKER);
    match (start, end) {
        (Some(start_index), Some(end_index)) => {
            let after = &text[end_index + FRAMEWORK_END_MARKER.len()..];
            let merged = format!("{}{}", &text[..start_index], after);
            let trimmed = merged.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("{trimmed}\n")
            }
        }
        _ => text.to_string(),
    }
}

fn read_text_if_exists(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn write_text_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    let existing = read_text_if_exists(path)?;
    if existing.as_deref() == Some(content) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, content).map_err(|err| err.to_string())?;
    Ok(true)
}

fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, String> {
    let formatted = format!(
        "{}\n",
        serde_json::to_string_pretty(payload).map_err(|err| err.to_string())?
    );
    write_text_if_changed(path, &formatted)
}

fn read_json_map_if_exists(path: &Path) -> Result<Option<Map<String, Value>>, String> {
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(None);
    };
    let parsed: Value = serde_json::from_str(&content).map_err(|err| err.to_string())?;
    match parsed {
        Value::Object(map) => Ok(Some(map)),
        _ => Ok(None),
    }
}

fn remove_path(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn symlink_exists(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}
