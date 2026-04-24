use anyhow::{Context, Result};
use clap::Parser;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "codex-extract-tokens")]
#[command(about = "Rust token discovery helper for codex-aggregator")]
struct Args {
    #[arg(long)]
    vscode_hosts: Option<PathBuf>,
    #[arg(long)]
    skip_gh: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("=== GitHub Copilot Token Extractor ===");

    let vscode_config = args.vscode_hosts.unwrap_or_else(default_vscode_hosts_path);
    extract_vscode_token(&vscode_config)?;
    if !args.skip_gh {
        extract_gh_token();
    }

    println!();
    println!("Instructions:");
    println!(
        "1. If you extracted a 'gho_' or 'ghu_' token, use it in the 'new-api' channel setup."
    );
    println!("2. For stable login, it is recommended to use the 'Device Flow' directly on the 'copilot-proxy' first.");

    Ok(())
}

fn default_vscode_hosts_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Library/Application Support/Code/User/globalStorage/github.copilot/hosts.json")
}

fn extract_vscode_token(path: &PathBuf) -> Result<()> {
    if !path.is_file() {
        println!("[Skip] VS Code config not found.");
        return Ok(());
    }

    println!("[Found] VS Code Copilot config at {}", path.display());
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read VS Code config {}", path.display()))?;
    let json = serde_json::from_str::<Value>(&content)
        .with_context(|| format!("failed to parse VS Code config {}", path.display()))?;
    if let Some(token) = find_oauth_token(&json) {
        println!("Token: {token}");
    }

    Ok(())
}

fn find_oauth_token(value: &Value) -> Option<&str> {
    match value {
        Value::Object(map) => {
            if let Some(token) = map.get("oauth_token").and_then(Value::as_str) {
                return Some(token);
            }
            map.values().find_map(find_oauth_token)
        }
        Value::Array(items) => items.iter().find_map(find_oauth_token),
        _ => None,
    }
}

fn extract_gh_token() {
    let Ok(output) = Command::new("gh").args(["auth", "token"]).output() else {
        return;
    };
    println!("[Found] GitHub CLI. Attempting to get token...");
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() && !token.is_empty() {
        println!("GH_TOKEN: {token}");
    } else {
        println!("[Info] 'gh auth token' returned empty. Try 'gh auth login' first.");
    }
}
