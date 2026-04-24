use anyhow::{Context, Result};
use clap::Parser;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use std::env;
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "codex-health-monitor")]
#[command(about = "Rust Copilot token health monitor for codex-aggregator")]
struct Args {
    #[arg(long)]
    once: bool,
    #[arg(long, default_value_t = 3600)]
    check_interval_seconds: u64,
}

#[derive(Debug)]
struct TokenStatus {
    active: bool,
    status: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("codex-aggregator-health-monitor-rs/1.0")
        .build()
        .context("failed to build HTTP client")?;
    let tokens = env::var("COPILOT_TOKENS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let notification_url = env::var("NOTIFICATION_URL")
        .ok()
        .filter(|value| !value.is_empty());

    println!("Starting Copilot Health Monitor...");
    loop {
        for token in &tokens {
            let status = check_token(&client, token);
            if !status.active {
                let message = format!(
                    "Codex Alert: Account [{}...] status: {}",
                    token.chars().take(8).collect::<String>(),
                    status.status
                );
                send_alert(&client, notification_url.as_deref(), &message);
            }
        }

        if args.once {
            break;
        }
        thread::sleep(Duration::from_secs(args.check_interval_seconds));
    }

    Ok(())
}

fn check_token(client: &Client, token: &str) -> TokenStatus {
    let request = client
        .get("https://api.github.com/user/billing/copilot")
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json");

    match request.send() {
        Ok(response) => match response.status() {
            StatusCode::OK => TokenStatus {
                active: true,
                status: "Active".to_string(),
            },
            StatusCode::UNPROCESSABLE_ENTITY => TokenStatus {
                active: false,
                status: "Subscription Expired or Billing Issue".to_string(),
            },
            StatusCode::UNAUTHORIZED => TokenStatus {
                active: false,
                status: "Token Invalid or Revoked".to_string(),
            },
            other => TokenStatus {
                active: false,
                status: format!("Unexpected Status: {}", other.as_u16()),
            },
        },
        Err(error) => TokenStatus {
            active: false,
            status: format!("Error: {error}"),
        },
    }
}

fn send_alert(client: &Client, notification_url: Option<&str>, message: &str) {
    let Some(base_url) = notification_url else {
        return;
    };
    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        urlencoding::encode(message)
    );
    match client.get(url).send() {
        Ok(_) => println!("Alert sent: {message}"),
        Err(error) => eprintln!("Failed to send alert: {error}"),
    }
}
