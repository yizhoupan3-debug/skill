use anyhow::{Context, Result};
use clap::Parser;
use mysql::prelude::Queryable;
use mysql::{OptsBuilder, Pool};
use serde::Serialize;
use std::env;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const INDEX_HTML: &str = include_str!("../../dashboard/static/index.html");

#[derive(Parser)]
#[command(name = "codex-dashboard")]
#[command(about = "Rust dashboard for codex-aggregator quota visibility")]
struct Args {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    #[arg(long, default_value_t = 5000)]
    port: u16,
}

#[derive(Clone)]
struct DbConfig {
    host: String,
    user: String,
    pass: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct AccountQuota {
    id: u64,
    name: String,
    status: String,
    used_5h: u64,
    used_7d: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let db = DbConfig::from_env();
    let addr = format!("{}:{}", args.host, args.port);
    let server = Server::http(&addr).map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("codex-dashboard listening on http://{addr}");
    for request in server.incoming_requests() {
        if let Err(error) = handle_request(request, &db) {
            eprintln!("request failed: {error:#}");
        }
    }

    Ok(())
}

impl DbConfig {
    fn from_env() -> Self {
        Self {
            host: env::var("DB_HOST").unwrap_or_else(|_| "db".to_string()),
            user: env::var("DB_USER").unwrap_or_else(|_| "root".to_string()),
            pass: env::var("DB_PASS").unwrap_or_else(|_| "123456".to_string()),
            name: env::var("DB_NAME").unwrap_or_else(|_| "oneapi".to_string()),
        }
    }

    fn pool(&self) -> Result<Pool> {
        let opts = OptsBuilder::new()
            .ip_or_hostname(Some(self.host.clone()))
            .user(Some(self.user.clone()))
            .pass(Some(self.pass.clone()))
            .db_name(Some(self.name.clone()));
        Pool::new(opts).context("failed to create MySQL pool")
    }
}

fn handle_request(request: Request, db: &DbConfig) -> Result<()> {
    match (request.method(), request.url()) {
        (&Method::Get, "/") => respond_html(request, StatusCode(200), INDEX_HTML),
        (&Method::Get, "/api/quotas") => match load_quotas(db) {
            Ok(quotas) => respond_json(request, StatusCode(200), &quotas),
            Err(error) => {
                let payload = serde_json::json!({ "error": error.to_string() });
                respond_json(request, StatusCode(500), &payload)
            }
        },
        _ => respond_text(request, StatusCode(404), "not found"),
    }
}

fn load_quotas(db: &DbConfig) -> Result<Vec<AccountQuota>> {
    let pool = db.pool()?;
    let mut conn = pool.get_conn().context("failed to connect to MySQL")?;
    conn.query_map(
        "SELECT c.id, c.name, c.status, \
                COUNT(CASE WHEN l.created_at > UNIX_TIMESTAMP(NOW() - INTERVAL 5 HOUR) THEN 1 END) AS used_5h, \
                COUNT(CASE WHEN l.created_at > UNIX_TIMESTAMP(NOW() - INTERVAL 7 DAY) THEN 1 END) AS used_7d \
         FROM channels c \
         LEFT JOIN logs l \
           ON l.channel_id = c.id \
          AND l.created_at > UNIX_TIMESTAMP(NOW() - INTERVAL 7 DAY) \
         WHERE c.type = 15 \
         GROUP BY c.id, c.name, c.status \
         ORDER BY c.id",
        |(id, name, status, used_5h, used_7d)| AccountQuota {
            id,
            name,
            status: channel_status_label(status),
            used_5h,
            used_7d,
        },
    )
    .context("failed to query channel quotas")
}

fn channel_status_label(status: i32) -> String {
    if status == 1 { "active" } else { "red" }.to_string()
}

fn respond_html(request: Request, status: StatusCode, body: &str) -> Result<()> {
    respond_with_content_type(request, status, body, "text/html; charset=utf-8")
}

fn respond_json<T: Serialize>(request: Request, status: StatusCode, payload: &T) -> Result<()> {
    let body = serde_json::to_string(payload).context("failed to serialize JSON response")?;
    respond_with_content_type(request, status, &body, "application/json; charset=utf-8")
}

fn respond_text(request: Request, status: StatusCode, body: &str) -> Result<()> {
    respond_with_content_type(request, status, body, "text/plain; charset=utf-8")
}

fn respond_with_content_type(
    request: Request,
    status: StatusCode,
    body: &str,
    content_type: &str,
) -> Result<()> {
    let header = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
        .map_err(|_| anyhow::anyhow!("failed to build Content-Type header"))?;
    request
        .respond(
            Response::from_string(body.to_string())
                .with_status_code(status)
                .with_header(header),
        )
        .map_err(|err| anyhow::anyhow!("failed to write HTTP response: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_channel_status_labels() {
        assert_eq!(channel_status_label(1), "active");
        assert_eq!(channel_status_label(2), "red");
    }
}
