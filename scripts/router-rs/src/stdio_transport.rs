use crate::{
    dispatch_stdio_json_request_payload, env_usize, DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS,
    DEFAULT_COMPUTE_THREADS, DEFAULT_MAX_BACKGROUND_JOBS, DEFAULT_MAX_CONCURRENT_SUBAGENTS,
    DEFAULT_SUBAGENT_TIMEOUT_SECONDS, MAX_BACKGROUND_JOBS_LIMIT, MAX_COMPUTE_THREADS,
    MAX_CONCURRENT_SUBAGENTS_LIMIT,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufWriter, Write};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(crate) const DEFAULT_ROUTER_STDIO_POOL_SIZE: usize = 8;
pub(crate) const MAX_ROUTER_STDIO_POOL_SIZE: usize = 32;
const DEFAULT_STDIO_IN_FLIGHT_TIMEOUT_SECONDS: u64 = 30;
const MAX_STDIO_IN_FLIGHT_TIMEOUT_SECONDS: u64 = 3600;
const STDIO_RESPONSE_FLUSH_BATCH_SIZE: usize = 16;

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

#[derive(Debug, Clone)]
struct InFlightRequest {
    id: Value,
    started_at: Option<Instant>,
}

#[derive(Debug, Clone)]
enum StdioWorkerMessage {
    Started { line_index: u64 },
    Finished(StdioJsonResponseEnvelope),
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
    let mut response_writer = StdioResponseWriter::new(stdout.lock());
    for line_result in stdin.lock().lines() {
        let line = line_result.map_err(|err| format!("read stdio request failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_stdio_json_line(&line);
        let encoded = serde_json::to_string(&response)
            .map_err(|err| format!("serialize stdio response failed: {err}"))?;
        response_writer.write_encoded_response(&encoded)?;
    }
    response_writer.flush()?;
    Ok(())
}

fn run_concurrent_stdio_json_loop(max_concurrency: usize) -> Result<(), String> {
    let in_flight_timeout = resolve_stdio_in_flight_timeout();
    let (task_tx, task_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
    let (result_tx, result_rx) = mpsc::channel::<StdioWorkerMessage>();
    let (worker_txs, worker_joins) = spawn_stdio_workers(max_concurrency, result_tx.clone());
    let dispatcher_handle = spawn_stdio_dispatcher(task_rx, worker_txs, result_tx.clone());

    drop(result_tx);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut response_writer = StdioResponseWriter::new(stdout.lock());
    let mut in_flight_requests = BTreeMap::<u64, InFlightRequest>::new();
    let mut next_line_index = 0_u64;
    for line_result in stdin.lock().lines() {
        let line = line_result.map_err(|err| format!("read stdio request failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }

        while in_flight_requests.len() >= max_concurrency {
            if let Some(envelope) = recv_stdio_response_or_timeout(
                &result_rx,
                &mut in_flight_requests,
                in_flight_timeout,
            )? {
                write_stdio_response(&envelope.response, &mut response_writer)?;
            }
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
                    let request_id = request.id.clone();
                    task_tx
                        .send(StdioJsonRequestEnvelope {
                            line_index: next_line_index,
                            request,
                        })
                        .map_err(|err| format!("queue stdio request failed: {err}"))?;
                    in_flight_requests.insert(
                        next_line_index,
                        InFlightRequest {
                            id: request_id,
                            started_at: None,
                        },
                    );
                    next_line_index += 1;
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
        write_stdio_response(&envelope.response, &mut response_writer)?;
        next_line_index += 1;
    }

    drop(task_tx);
    dispatcher_handle
        .join()
        .map_err(|_| "stdio dispatcher thread panicked".to_string())?;
    for join in worker_joins {
        join.join()
            .map_err(|_| "stdio worker thread panicked".to_string())?;
    }
    while !in_flight_requests.is_empty() {
        if let Some(envelope) =
            recv_stdio_response_or_timeout(&result_rx, &mut in_flight_requests, in_flight_timeout)?
        {
            write_stdio_response(&envelope.response, &mut response_writer)?;
        }
    }
    response_writer.flush()?;
    Ok(())
}

fn recv_stdio_response_or_timeout(
    result_rx: &mpsc::Receiver<StdioWorkerMessage>,
    in_flight_requests: &mut BTreeMap<u64, InFlightRequest>,
    in_flight_timeout: Duration,
) -> Result<Option<StdioJsonResponseEnvelope>, String> {
    loop {
        let wait_duration = next_in_flight_wait_duration(in_flight_requests, in_flight_timeout);
        match result_rx.recv_timeout(wait_duration) {
            Ok(StdioWorkerMessage::Started { line_index }) => {
                if let Some(request) = in_flight_requests.get_mut(&line_index) {
                    request.started_at = Some(Instant::now());
                }
            }
            Ok(StdioWorkerMessage::Finished(envelope)) => {
                if in_flight_requests.remove(&envelope.line_index).is_some() {
                    return Ok(Some(envelope));
                }
                // The request already timed out and emitted a synthetic response; ignore late completion.
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Ok(pop_timed_out_stdio_request(in_flight_requests, in_flight_timeout).map(
                    |(line_index, timed_out)| StdioJsonResponseEnvelope {
                        line_index,
                        response: StdioJsonResponsePayload {
                            id: timed_out.id,
                            ok: false,
                            payload: None,
                            error: Some(format!(
                                "stdio request timed out after {}s while waiting for worker response",
                                in_flight_timeout.as_secs()
                            )),
                        },
                    },
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("receive stdio response failed: channel disconnected".to_string());
            }
        }
    }
}

fn next_in_flight_wait_duration(
    in_flight_requests: &BTreeMap<u64, InFlightRequest>,
    in_flight_timeout: Duration,
) -> Duration {
    let now = Instant::now();
    in_flight_requests
        .values()
        .filter_map(|request| {
            let started_at = request.started_at?;
            in_flight_timeout
                .checked_sub(now.duration_since(started_at))
                .or(Some(Duration::ZERO))
        })
        .min()
        .unwrap_or(Duration::from_millis(50))
}

fn pop_timed_out_stdio_request(
    in_flight_requests: &mut BTreeMap<u64, InFlightRequest>,
    in_flight_timeout: Duration,
) -> Option<(u64, InFlightRequest)> {
    let now = Instant::now();
    let timed_out_index = in_flight_requests
        .iter()
        .find_map(|(line_index, request)| {
            let started_at = request.started_at?;
            let elapsed = now.duration_since(started_at);
            (elapsed >= in_flight_timeout).then_some(*line_index)
        })?;
    in_flight_requests.remove_entry(&timed_out_index)
}

fn spawn_stdio_workers(
    worker_count: usize,
    result_tx: mpsc::Sender<StdioWorkerMessage>,
) -> (
    Vec<mpsc::Sender<StdioJsonRequestEnvelope>>,
    Vec<JoinHandle<()>>,
) {
    let mut worker_txs = Vec::with_capacity(worker_count);
    let mut worker_joins = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let (worker_tx, worker_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
        let worker_result_tx = result_tx.clone();
        let join = std::thread::spawn(move || {
            while let Ok(envelope) = worker_rx.recv() {
                if worker_result_tx
                    .send(StdioWorkerMessage::Started {
                        line_index: envelope.line_index,
                    })
                    .is_err()
                {
                    return;
                }
                let response = dispatch_stdio_json_envelope(envelope);
                if worker_result_tx
                    .send(StdioWorkerMessage::Finished(response))
                    .is_err()
                {
                    return;
                }
            }
        });
        worker_txs.push(worker_tx);
        worker_joins.push(join);
    }
    (worker_txs, worker_joins)
}

fn spawn_stdio_dispatcher(
    task_rx: mpsc::Receiver<StdioJsonRequestEnvelope>,
    worker_txs: Vec<mpsc::Sender<StdioJsonRequestEnvelope>>,
    result_tx: mpsc::Sender<StdioWorkerMessage>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || dispatch_stdio_work_items(task_rx, worker_txs, result_tx))
}

fn dispatch_stdio_work_items(
    task_rx: mpsc::Receiver<StdioJsonRequestEnvelope>,
    mut worker_txs: Vec<mpsc::Sender<StdioJsonRequestEnvelope>>,
    result_tx: mpsc::Sender<StdioWorkerMessage>,
) {
    let mut next_worker = 0_usize;
    while let Ok(envelope) = task_rx.recv() {
        let mut maybe_envelope = Some(envelope);
        while let Some(pending_envelope) = maybe_envelope.take() {
            if worker_txs.is_empty() {
                if report_stdio_worker_pool_unavailable(pending_envelope, &result_tx).is_err() {
                    return;
                }
                for queued_envelope in task_rx {
                    if report_stdio_worker_pool_unavailable(queued_envelope, &result_tx).is_err() {
                        return;
                    }
                }
                return;
            }
            let worker_index = next_worker % worker_txs.len();
            match worker_txs[worker_index].send(pending_envelope) {
                Ok(()) => {
                    next_worker = worker_index.wrapping_add(1);
                }
                Err(err) => {
                    maybe_envelope = Some(err.0);
                    worker_txs.swap_remove(worker_index);
                    if !worker_txs.is_empty() {
                        next_worker = worker_index % worker_txs.len();
                    }
                }
            }
        }
    }
}

fn report_stdio_worker_pool_unavailable(
    envelope: StdioJsonRequestEnvelope,
    result_tx: &mpsc::Sender<StdioWorkerMessage>,
) -> Result<(), mpsc::SendError<StdioWorkerMessage>> {
    result_tx.send(StdioWorkerMessage::Finished(StdioJsonResponseEnvelope {
        line_index: envelope.line_index,
        response: StdioJsonResponsePayload {
            id: envelope.request.id,
            ok: false,
            payload: None,
            error: Some("stdio worker pool unavailable: all workers have stopped".to_string()),
        },
    }))
}

struct StdioResponseWriter<W: Write> {
    output: BufWriter<W>,
    pending_responses: usize,
}

impl<W: Write> StdioResponseWriter<W> {
    fn new(output: W) -> Self {
        Self {
            output: BufWriter::new(output),
            pending_responses: 0,
        }
    }

    fn write_encoded_response(&mut self, encoded: &str) -> Result<(), String> {
        writeln!(self.output, "{encoded}")
            .map_err(|err| format!("write stdio response failed: {err}"))?;
        self.pending_responses = self.pending_responses.saturating_add(1);
        if self.pending_responses >= STDIO_RESPONSE_FLUSH_BATCH_SIZE {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        self.output
            .flush()
            .map_err(|err| format!("flush stdio response failed: {err}"))?;
        self.pending_responses = 0;
        Ok(())
    }
}

fn write_stdio_response<W: Write>(
    response: &StdioJsonResponsePayload,
    output: &mut StdioResponseWriter<W>,
) -> Result<(), String> {
    let encoded = serde_json::to_string(response)
        .map_err(|err| format!("serialize stdio response failed: {err}"))?;
    output.write_encoded_response(&encoded)
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
            scheduling: "bounded FIFO with completion-order response emission",
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
        .unwrap_or(DEFAULT_ROUTER_STDIO_POOL_SIZE)
        .clamp(1, MAX_ROUTER_STDIO_POOL_SIZE)
}

fn resolve_stdio_in_flight_timeout() -> Duration {
    let seconds = env_usize("ROUTER_RS_STDIO_IN_FLIGHT_TIMEOUT_SECONDS")
        .map(|value| value as u64)
        .unwrap_or(DEFAULT_STDIO_IN_FLIGHT_TIMEOUT_SECONDS)
        .clamp(1, MAX_STDIO_IN_FLIGHT_TIMEOUT_SECONDS);
    Duration::from_secs(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    // Serializes tests that mutate process-wide env vars so they do not race with each other
    // or with parallel resolver tests.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn descriptor_default_pool_size_matches_resolver_fallback() {
        let _env_lock = ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _stdio_pool = EnvVarGuard::unset("ROUTER_RS_STDIO_POOL_SIZE");
        let _stdio_max = EnvVarGuard::unset("ROUTER_RS_STDIO_MAX_CONCURRENCY");

        let defaults = runtime_concurrency_defaults_payload();
        let resolved_default = resolve_stdio_max_concurrency(None);

        assert_eq!(
            defaults.router_stdio.default_pool_size, resolved_default,
            "descriptor default_pool_size must match resolver fallback when no env / override is set"
        );
        assert_eq!(
            defaults.router_stdio.default_pool_size, DEFAULT_ROUTER_STDIO_POOL_SIZE,
            "descriptor default_pool_size must reflect DEFAULT_ROUTER_STDIO_POOL_SIZE"
        );
        assert!(
            defaults.router_stdio.default_pool_size <= defaults.router_stdio.max_pool_size,
            "default_pool_size must not exceed max_pool_size"
        );
        assert_eq!(
            resolve_stdio_max_concurrency(Some(defaults.router_stdio.max_pool_size + 100)),
            defaults.router_stdio.max_pool_size,
            "resolver must clamp at descriptor-advertised max_pool_size"
        );
    }

    #[test]
    fn dispatch_work_items_spreads_requests_round_robin() {
        let (task_tx, task_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
        let (worker_a_tx, worker_a_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
        let (worker_b_tx, worker_b_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();

        let (result_tx, _result_rx) = mpsc::channel::<StdioWorkerMessage>();
        let dispatcher = std::thread::spawn(move || {
            dispatch_stdio_work_items(task_rx, vec![worker_a_tx, worker_b_tx], result_tx)
        });

        task_tx
            .send(StdioJsonRequestEnvelope {
                line_index: 0,
                request: StdioJsonRequestPayload {
                    id: json!(0),
                    op: "concurrency_defaults".to_string(),
                    payload: Value::Null,
                    concurrency: None,
                },
            })
            .expect("send first task");
        task_tx
            .send(StdioJsonRequestEnvelope {
                line_index: 1,
                request: StdioJsonRequestPayload {
                    id: json!(1),
                    op: "concurrency_defaults".to_string(),
                    payload: Value::Null,
                    concurrency: None,
                },
            })
            .expect("send second task");
        drop(task_tx);

        let first = worker_a_rx.recv().expect("worker A receives first item");
        let second = worker_b_rx.recv().expect("worker B receives second item");
        assert_eq!(first.line_index, 0);
        assert_eq!(second.line_index, 1);
        dispatcher.join().expect("dispatcher exits cleanly");
    }

    #[test]
    fn dispatch_work_items_skips_closed_worker_and_continues() {
        let (task_tx, task_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
        let (closed_worker_tx, closed_worker_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
        let (worker_b_tx, worker_b_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();

        drop(closed_worker_rx);

        let (result_tx, _result_rx) = mpsc::channel::<StdioWorkerMessage>();
        let dispatcher = std::thread::spawn(move || {
            dispatch_stdio_work_items(task_rx, vec![closed_worker_tx, worker_b_tx], result_tx)
        });

        task_tx
            .send(StdioJsonRequestEnvelope {
                line_index: 0,
                request: StdioJsonRequestPayload {
                    id: json!(0),
                    op: "concurrency_defaults".to_string(),
                    payload: Value::Null,
                    concurrency: None,
                },
            })
            .expect("send first task");
        task_tx
            .send(StdioJsonRequestEnvelope {
                line_index: 1,
                request: StdioJsonRequestPayload {
                    id: json!(1),
                    op: "concurrency_defaults".to_string(),
                    payload: Value::Null,
                    concurrency: None,
                },
            })
            .expect("send second task");
        drop(task_tx);

        let first = worker_b_rx.recv().expect("worker B receives first item");
        let second = worker_b_rx.recv().expect("worker B receives second item");
        assert_eq!(first.line_index, 0);
        assert_eq!(second.line_index, 1);
        dispatcher.join().expect("dispatcher exits cleanly");
    }

    #[test]
    fn write_stdio_response_keeps_ordered_lines() {
        let mut output = Vec::<u8>::new();
        let mut response_writer = StdioResponseWriter::new(&mut output);

        write_stdio_response(
            &StdioJsonResponsePayload {
                id: json!(1),
                ok: true,
                payload: Some(json!({"n": 1})),
                error: None,
            },
            &mut response_writer,
        )
        .expect("writes first response");

        write_stdio_response(
            &StdioJsonResponsePayload {
                id: json!(0),
                ok: true,
                payload: Some(json!({"n": 0})),
                error: None,
            },
            &mut response_writer,
        )
        .expect("writes second response");

        write_stdio_response(
            &StdioJsonResponsePayload {
                id: json!(2),
                ok: true,
                payload: Some(json!({"n": 2})),
                error: None,
            },
            &mut response_writer,
        )
        .expect("writes third response");
        response_writer.flush().expect("flush buffered responses");
        drop(response_writer);

        let output_text = String::from_utf8(output).expect("valid utf8 output");
        let lines: Vec<&str> = output_text.lines().collect();
        assert_eq!(lines.len(), 3);

        let first: Value = serde_json::from_str(lines[0]).expect("parse first line");
        let second: Value = serde_json::from_str(lines[1]).expect("parse second line");
        let third: Value = serde_json::from_str(lines[2]).expect("parse third line");
        assert_eq!(first.get("id"), Some(&json!(1)));
        assert_eq!(second.get("id"), Some(&json!(0)));
        assert_eq!(third.get("id"), Some(&json!(2)));
    }

    #[test]
    fn dispatch_work_items_returns_errors_when_last_worker_is_gone() {
        let (task_tx, task_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
        let (worker_tx, worker_rx) = mpsc::channel::<StdioJsonRequestEnvelope>();
        let (result_tx, result_rx) = mpsc::channel::<StdioWorkerMessage>();

        drop(worker_rx);

        let dispatcher = std::thread::spawn(move || {
            dispatch_stdio_work_items(task_rx, vec![worker_tx], result_tx)
        });

        task_tx
            .send(StdioJsonRequestEnvelope {
                line_index: 0,
                request: StdioJsonRequestPayload {
                    id: json!("first"),
                    op: "concurrency_defaults".to_string(),
                    payload: Value::Null,
                    concurrency: None,
                },
            })
            .expect("send first task");
        task_tx
            .send(StdioJsonRequestEnvelope {
                line_index: 1,
                request: StdioJsonRequestPayload {
                    id: json!("second"),
                    op: "concurrency_defaults".to_string(),
                    payload: Value::Null,
                    concurrency: None,
                },
            })
            .expect("send second task");
        drop(task_tx);

        let first = match result_rx.recv().expect("receive first error response") {
            StdioWorkerMessage::Finished(envelope) => envelope,
            StdioWorkerMessage::Started { .. } => panic!("expected finished error envelope"),
        };
        let second = match result_rx.recv().expect("receive second error response") {
            StdioWorkerMessage::Finished(envelope) => envelope,
            StdioWorkerMessage::Started { .. } => panic!("expected finished error envelope"),
        };
        assert_eq!(first.line_index, 0);
        assert_eq!(second.line_index, 1);
        assert_eq!(first.response.id, json!("first"));
        assert_eq!(second.response.id, json!("second"));
        assert!(!first.response.ok);
        assert!(!second.response.ok);
        assert_eq!(first.response.payload, None);
        assert_eq!(second.response.payload, None);
        assert_eq!(
            first.response.error.as_deref(),
            Some("stdio worker pool unavailable: all workers have stopped")
        );
        assert_eq!(
            second.response.error.as_deref(),
            Some("stdio worker pool unavailable: all workers have stopped")
        );
        dispatcher.join().expect("dispatcher exits cleanly");
    }

    #[test]
    fn in_flight_timeout_unblocks_ordered_output_with_error_response() {
        let (_result_tx, result_rx) = mpsc::channel::<StdioWorkerMessage>();
        let timeout = Duration::from_millis(10);
        let mut in_flight_requests = BTreeMap::<u64, InFlightRequest>::new();
        let mut output = Vec::<u8>::new();
        let mut response_writer = StdioResponseWriter::new(&mut output);

        in_flight_requests.insert(
            0,
            InFlightRequest {
                id: json!("stuck"),
                started_at: Some(Instant::now() - Duration::from_millis(30)),
            },
        );

        write_stdio_response(
            &StdioJsonResponsePayload {
                id: json!("next"),
                ok: true,
                payload: Some(json!({"n": 1})),
                error: None,
            },
            &mut response_writer,
        )
        .expect("write completed response immediately");

        let timeout_envelope =
            recv_stdio_response_or_timeout(&result_rx, &mut in_flight_requests, timeout)
                .expect("receiving timeout envelope succeeds")
                .expect("timed out request becomes synthetic response");
        assert_eq!(timeout_envelope.line_index, 0);
        assert_eq!(timeout_envelope.response.id, json!("stuck"));
        assert!(!timeout_envelope.response.ok);
        assert_eq!(timeout_envelope.response.payload, None);
        assert_eq!(
            timeout_envelope.response.error.as_deref(),
            Some("stdio request timed out after 0s while waiting for worker response")
        );

        write_stdio_response(&timeout_envelope.response, &mut response_writer)
            .expect("timeout response also writes immediately");
        response_writer.flush().expect("flush buffered responses");
        drop(response_writer);

        let output_text = String::from_utf8(output).expect("valid utf8 output");
        let lines: Vec<&str> = output_text.lines().collect();
        assert_eq!(lines.len(), 2);

        let first: Value = serde_json::from_str(lines[0]).expect("parse first line");
        let second: Value = serde_json::from_str(lines[1]).expect("parse second line");
        assert_eq!(first.get("id"), Some(&json!("next")));
        assert_eq!(first.get("ok"), Some(&json!(true)));
        assert_eq!(second.get("id"), Some(&json!("stuck")));
        assert_eq!(second.get("ok"), Some(&json!(false)));
        assert_eq!(
            second.get("error"),
            Some(&json!(
                "stdio request timed out after 0s while waiting for worker response"
            ))
        );
    }

    #[test]
    fn late_worker_response_after_timeout_is_ignored() {
        let (result_tx, result_rx) = mpsc::channel::<StdioWorkerMessage>();
        let timeout = Duration::from_millis(10);
        let mut in_flight_requests = BTreeMap::<u64, InFlightRequest>::new();

        in_flight_requests.insert(
            7,
            InFlightRequest {
                id: json!("stuck"),
                started_at: Some(Instant::now() - Duration::from_millis(30)),
            },
        );

        let timeout_envelope =
            recv_stdio_response_or_timeout(&result_rx, &mut in_flight_requests, timeout)
                .expect("receiving timeout envelope succeeds")
                .expect("timed out request becomes synthetic response");
        assert_eq!(timeout_envelope.line_index, 7);
        assert!(in_flight_requests.is_empty());

        result_tx
            .send(StdioWorkerMessage::Finished(StdioJsonResponseEnvelope {
                line_index: 7,
                response: StdioJsonResponsePayload {
                    id: json!("stuck"),
                    ok: true,
                    payload: Some(json!({"late": true})),
                    error: None,
                },
            }))
            .expect("send late worker response");

        let late = recv_stdio_response_or_timeout(&result_rx, &mut in_flight_requests, timeout)
            .expect("receive call succeeds");
        assert!(late.is_none());
    }
}
