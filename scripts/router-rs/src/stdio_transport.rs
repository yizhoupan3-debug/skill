use crate::{
    dispatch_stdio_json_request_payload, env_usize, DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS,
    DEFAULT_COMPUTE_THREADS, DEFAULT_MAX_BACKGROUND_JOBS, DEFAULT_MAX_CONCURRENT_SUBAGENTS,
    DEFAULT_SUBAGENT_TIMEOUT_SECONDS, MAX_BACKGROUND_JOBS_LIMIT, MAX_COMPUTE_THREADS,
    MAX_CONCURRENT_SUBAGENTS_LIMIT,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::sync::{mpsc, Arc, Mutex};

pub(crate) const DEFAULT_ROUTER_STDIO_POOL_SIZE: usize = 4;
pub(crate) const MAX_ROUTER_STDIO_POOL_SIZE: usize = 16;
const DEFAULT_STDIO_MAX_CONCURRENCY: usize = 1;
const MAX_STDIO_MAX_CONCURRENCY: usize = 16;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StdioJsonRequestPayload {
    pub(crate) id: Value,
    pub(crate) op: String,
    #[serde(default)]
    pub(crate) payload: Value,
    #[serde(default)]
    concurrency: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StdioJsonResponsePayload {
    pub(crate) id: Value,
    pub(crate) ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone)]
struct StdioJsonRequestEnvelope {
    line_index: u64,
    request: StdioJsonRequestPayload,
}

#[derive(Debug, Clone)]
struct StdioJsonResponseEnvelope {
    line_index: u64,
    response: StdioJsonResponsePayload,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StdioRouterConcurrencyDescriptor {
    pub(crate) default_pool_size: usize,
    pub(crate) max_pool_size: usize,
    pub(crate) env_keys: Vec<&'static str>,
    pub(crate) stdio_max_concurrency_arg: &'static str,
    pub(crate) request_concurrency_field: &'static str,
    pub(crate) scheduling: &'static str,
    pub(crate) backpressure: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ComputeConcurrencyDescriptor {
    pub(crate) default_threads: usize,
    pub(crate) max_threads: usize,
    pub(crate) env_keys: Vec<&'static str>,
    pub(crate) cli_arg: &'static str,
    pub(crate) scheduling: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeConcurrencyDefaultsPayload {
    pub(crate) router_stdio: StdioRouterConcurrencyDescriptor,
    pub(crate) compute: ComputeConcurrencyDescriptor,
    pub(crate) max_background_jobs: usize,
    pub(crate) max_background_jobs_limit: usize,
    pub(crate) background_job_timeout_seconds: u64,
    pub(crate) max_concurrent_subagents: usize,
    pub(crate) max_concurrent_subagents_limit: usize,
    pub(crate) subagent_timeout_seconds: u64,
}

pub(crate) fn run_stdio_json_loop(max_concurrency_override: Option<usize>) -> Result<(), String> {
    let max_concurrency = resolve_stdio_max_concurrency(max_concurrency_override);
    if max_concurrency > 1 {
        return run_concurrent_stdio_json_loop(max_concurrency);
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();
    for line_result in stdin.lock().lines() {
        let line = line_result.map_err(|err| format!("read stdio request failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_stdio_json_line(&line);
        let encoded = serde_json::to_string(&response)
            .map_err(|err| format!("serialize stdio response failed: {err}"))?;
        writeln!(stdout_lock, "{encoded}")
            .map_err(|err| format!("write stdio response failed: {err}"))?;
        stdout_lock
            .flush()
            .map_err(|err| format!("flush stdio response failed: {err}"))?;
    }
    Ok(())
}

fn run_concurrent_stdio_json_loop(max_concurrency: usize) -> Result<(), String> {
    let (task_tx, task_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
    let (result_tx, result_rx) = mpsc::channel::<StdioJsonResponseEnvelope>();
    let shared_rx = Arc::new(Mutex::new(task_rx));

    for _ in 0..max_concurrency {
        let worker_rx = Arc::clone(&shared_rx);
        let worker_tx = result_tx.clone();
        std::thread::spawn(move || loop {
            let Ok(rx) = worker_rx.lock() else {
                return;
            };
            let request = rx.recv();
            drop(rx);
            let envelope = match request {
                Ok(envelope) => envelope,
                Err(_) => return,
            };
            let response = dispatch_stdio_json_envelope(envelope);
            if worker_tx.send(response).is_err() {
                return;
            }
        });
    }
    drop(result_tx);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();
    let mut pending = BTreeMap::<u64, StdioJsonResponsePayload>::new();
    let mut next_line_index = 0_u64;
    let mut next_write_index = 0_u64;
    let mut in_flight = 0_usize;

    for line_result in stdin.lock().lines() {
        let line = line_result.map_err(|err| format!("read stdio request failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }

        while in_flight >= max_concurrency {
            let envelope = result_rx
                .recv()
                .map_err(|err| format!("receive stdio response failed: {err}"))?;
            in_flight = in_flight.saturating_sub(1);
            write_ordered_stdio_response(
                envelope,
                &mut pending,
                &mut next_write_index,
                &mut stdout_lock,
            )?;
        }

        let envelope = match parse_stdio_json_line(&line) {
            Ok(request) => match request.concurrency {
                Some(0) => StdioJsonResponseEnvelope {
                    line_index: next_line_index,
                    response: StdioJsonResponsePayload {
                        id: request.id,
                        ok: false,
                        payload: None,
                        error: Some(
                            "stdio request concurrency must be a positive integer when provided"
                                .to_string(),
                        ),
                    },
                },
                Some(1) => StdioJsonResponseEnvelope {
                    line_index: next_line_index,
                    response: dispatch_stdio_json_request_payload(request),
                },
                _ => {
                    task_tx
                        .send(StdioJsonRequestEnvelope {
                            line_index: next_line_index,
                            request,
                        })
                        .map_err(|err| format!("queue stdio request failed: {err}"))?;
                    next_line_index += 1;
                    in_flight += 1;
                    continue;
                }
            },
            Err(err) => StdioJsonResponseEnvelope {
                line_index: next_line_index,
                response: StdioJsonResponsePayload {
                    id: Value::Null,
                    ok: false,
                    payload: None,
                    error: Some(err),
                },
            },
        };
        write_ordered_stdio_response(
            envelope,
            &mut pending,
            &mut next_write_index,
            &mut stdout_lock,
        )?;
        next_line_index += 1;
    }

    drop(task_tx);
    while in_flight > 0 {
        let envelope = result_rx
            .recv()
            .map_err(|err| format!("receive stdio response failed: {err}"))?;
        in_flight -= 1;
        write_ordered_stdio_response(
            envelope,
            &mut pending,
            &mut next_write_index,
            &mut stdout_lock,
        )?;
    }
    Ok(())
}

fn write_ordered_stdio_response<W: Write>(
    envelope: StdioJsonResponseEnvelope,
    pending: &mut BTreeMap<u64, StdioJsonResponsePayload>,
    next_write_index: &mut u64,
    output: &mut W,
) -> Result<(), String> {
    pending.insert(envelope.line_index, envelope.response);
    while let Some(response) = pending.remove(next_write_index) {
        let encoded = serde_json::to_string(&response)
            .map_err(|err| format!("serialize stdio response failed: {err}"))?;
        writeln!(output, "{encoded}")
            .map_err(|err| format!("write stdio response failed: {err}"))?;
        output
            .flush()
            .map_err(|err| format!("flush stdio response failed: {err}"))?;
        *next_write_index += 1;
    }
    Ok(())
}

pub(crate) fn handle_stdio_json_line(line: &str) -> StdioJsonResponsePayload {
    match parse_stdio_json_line(line) {
        Ok(request) => dispatch_stdio_json_request_payload(request),
        Err(err) => StdioJsonResponsePayload {
            id: Value::Null,
            ok: false,
            payload: None,
            error: Some(err),
        },
    }
}

fn dispatch_stdio_json_envelope(envelope: StdioJsonRequestEnvelope) -> StdioJsonResponseEnvelope {
    StdioJsonResponseEnvelope {
        line_index: envelope.line_index,
        response: dispatch_stdio_json_request_payload(envelope.request),
    }
}

fn parse_stdio_json_line(line: &str) -> Result<StdioJsonRequestPayload, String> {
    serde_json::from_str::<StdioJsonRequestPayload>(line)
        .map_err(|err| format!("parse stdio request failed: {err}"))
}

pub(crate) fn runtime_concurrency_defaults_payload() -> RuntimeConcurrencyDefaultsPayload {
    RuntimeConcurrencyDefaultsPayload {
        router_stdio: StdioRouterConcurrencyDescriptor {
            default_pool_size: DEFAULT_ROUTER_STDIO_POOL_SIZE,
            max_pool_size: MAX_ROUTER_STDIO_POOL_SIZE,
            env_keys: vec![
                "ROUTER_RS_STDIO_POOL_SIZE",
                "BROWSER_MCP_ROUTER_STDIO_POOL_SIZE",
                "CODEX_ROUTER_STDIO_POOL_SIZE",
            ],
            stdio_max_concurrency_arg: "--stdio-max-concurrency",
            request_concurrency_field: "concurrency",
            scheduling: "bounded FIFO with ordered responses",
            backpressure:
                "reader stops admitting new work while in-flight requests reach the limit",
        },
        compute: ComputeConcurrencyDescriptor {
            default_threads: DEFAULT_COMPUTE_THREADS,
            max_threads: MAX_COMPUTE_THREADS,
            env_keys: vec!["ROUTER_RS_COMPUTE_THREADS", "RAYON_NUM_THREADS"],
            cli_arg: "--compute-threads",
            scheduling: "bounded Rayon work-stealing for CPU record scans and batch eval",
        },
        max_background_jobs: DEFAULT_MAX_BACKGROUND_JOBS,
        max_background_jobs_limit: MAX_BACKGROUND_JOBS_LIMIT,
        background_job_timeout_seconds: DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS,
        max_concurrent_subagents: DEFAULT_MAX_CONCURRENT_SUBAGENTS,
        max_concurrent_subagents_limit: MAX_CONCURRENT_SUBAGENTS_LIMIT,
        subagent_timeout_seconds: DEFAULT_SUBAGENT_TIMEOUT_SECONDS,
    }
}

fn resolve_stdio_max_concurrency(override_value: Option<usize>) -> usize {
    override_value
        .or_else(|| env_usize("ROUTER_RS_STDIO_MAX_CONCURRENCY"))
        .or_else(|| env_usize("ROUTER_RS_STDIO_POOL_SIZE"))
        .unwrap_or(DEFAULT_STDIO_MAX_CONCURRENCY)
        .clamp(1, MAX_STDIO_MAX_CONCURRENCY)
}
