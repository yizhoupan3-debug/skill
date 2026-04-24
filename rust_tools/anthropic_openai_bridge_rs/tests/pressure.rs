use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn free_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local addr")
}

fn start_mock_upstream(addr: SocketAddr, expected: usize, stream: bool) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = TcpListener::bind(addr).expect("bind mock upstream");
        let handled = Arc::new(AtomicUsize::new(0));
        listener
            .set_nonblocking(true)
            .expect("set nonblocking listener");
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((stream_socket, _)) => {
                    stream_socket
                        .set_nonblocking(false)
                        .expect("set blocking stream");
                    let handled = handled.clone();
                    thread::spawn(move || {
                        handle_mock_connection(stream_socket, stream);
                        handled.fetch_add(1, Ordering::SeqCst);
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                }
                Err(err) => panic!("mock upstream accept failed: {err}"),
            }
            if handled.load(Ordering::SeqCst) >= expected {
                break;
            }
        }
        while handled.load(Ordering::SeqCst) < expected && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(handled.load(Ordering::SeqCst), expected);
    })
}

fn start_stalled_upstream(addr: SocketAddr) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = TcpListener::bind(addr).expect("bind stalled upstream");
        let (mut stream, _) = listener.accept().expect("accept stalled upstream");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut content_length = 0_usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("read request header");
            if line == "\r\n" || line == "\n" || line.is_empty() {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().expect("content length");
            }
        }
        let mut body = vec![0_u8; content_length];
        reader.read_exact(&mut body).expect("read request body");
        thread::sleep(Duration::from_secs(5));
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n{}",
        );
    })
}

fn handle_mock_connection(mut stream: std::net::TcpStream, streaming: bool) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut header = String::new();
    let mut content_length = 0_usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request header");
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().expect("content length");
        }
        header.push_str(&line);
    }
    let mut body = vec![0_u8; content_length];
    reader.read_exact(&mut body).expect("read request body");
    assert!(header.starts_with("POST /v1/chat/completions "));
    assert!(String::from_utf8_lossy(&body).contains("\"model\":\"gpt-5.5\""));

    let payload = if streaming {
        concat!(
            "data: {\"id\":\"chatcmpl_pressure\",\"model\":\"gpt-5.5\",\"choices\":[{\"delta\":{\"content\":\"pong\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":1}}\n\n",
            "data: [DONE]\n\n"
        )
        .to_string()
    } else {
        r#"{"id":"chatcmpl_pressure","model":"gpt-5.5","choices":[{"message":{"role":"assistant","content":"pong"},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":1}}"#.to_string()
    };
    let content_type = if streaming {
        "text/event-stream"
    } else {
        "application/json"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("write mock response");
}

fn start_bridge(upstream: SocketAddr, bridge: SocketAddr) -> Child {
    start_bridge_with_args(upstream, bridge, &[])
}

fn start_bridge_with_args(upstream: SocketAddr, bridge: SocketAddr, extra_args: &[&str]) -> Child {
    let bin = env!("CARGO_BIN_EXE_anthropic_openai_bridge_rs");
    let mut child = Command::new(bin)
        .arg("--listen")
        .arg(bridge.to_string())
        .arg("--upstream-base")
        .arg(format!("http://{upstream}/v1"))
        .arg("--upstream-key")
        .arg("sk-pressure")
        .arg("--model")
        .arg("gpt-5.5")
        .arg("--stream-channel-depth")
        .arg("8")
        .args(extra_args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bridge");
    wait_for_bridge(bridge, &mut child);
    child
}

fn wait_for_bridge(addr: SocketAddr, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("poll child") {
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                let _ = pipe.read_to_string(&mut stderr);
            }
            panic!("bridge exited early with {status}: {stderr}");
        }
        if std::net::TcpStream::connect(addr).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("bridge did not start");
}

fn post_message(addr: SocketAddr, stream: bool) -> String {
    let body = format!(
        r#"{{"model":"claude-sonnet-4-5","max_tokens":32,"stream":{stream},"messages":[{{"role":"user","content":"ping"}}]}}"#
    );
    let request = format!(
        "POST /v1/messages HTTP/1.1\r\nhost: {addr}\r\nauthorization: Bearer sk-test\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut socket = std::net::TcpStream::connect(addr).expect("connect bridge");
    socket
        .write_all(request.as_bytes())
        .expect("write bridge request");
    let mut response = String::new();
    socket
        .read_to_string(&mut response)
        .expect("read bridge response");
    response
}

#[test]
fn pressure_non_stream_bridge_handles_parallel_requests() {
    let request_count = 48;
    let upstream = free_addr();
    let bridge = free_addr();
    let upstream_thread = start_mock_upstream(upstream, request_count, false);
    let mut child = start_bridge(upstream, bridge);

    let started = Instant::now();
    let handles = (0..request_count)
        .map(|_| {
            thread::spawn(move || {
                let response = post_message(bridge, false);
                assert!(response.starts_with("HTTP/1.1 200 OK"));
                assert!(response.contains("\"type\":\"message\""));
                assert!(response.contains("\"text\":\"pong\""));
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().expect("request thread");
    }
    assert!(started.elapsed() < Duration::from_secs(10));
    child.kill().expect("kill bridge");
    let _ = child.wait();
    upstream_thread.join().expect("upstream thread");
}

#[test]
fn pressure_stream_bridge_handles_parallel_requests() {
    let request_count = 32;
    let upstream = free_addr();
    let bridge = free_addr();
    let upstream_thread = start_mock_upstream(upstream, request_count, true);
    let mut child = start_bridge(upstream, bridge);

    let started = Instant::now();
    let handles = (0..request_count)
        .map(|_| {
            thread::spawn(move || {
                let response = post_message(bridge, true);
                assert!(response.starts_with("HTTP/1.1 200 OK"));
                assert!(response.contains("event: message_start"));
                assert!(response.contains("event: content_block_delta"));
                assert!(response.contains("event: message_stop"));
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().expect("request thread");
    }
    assert!(started.elapsed() < Duration::from_secs(10));
    child.kill().expect("kill bridge");
    let _ = child.wait();
    upstream_thread.join().expect("upstream thread");
}

#[test]
fn non_stream_bridge_times_out_stalled_upstream() {
    let upstream = free_addr();
    let bridge = free_addr();
    let upstream_thread = start_stalled_upstream(upstream);
    let mut child =
        start_bridge_with_args(upstream, bridge, &["--upstream-request-timeout-secs", "1"]);

    let started = Instant::now();
    let response = post_message(bridge, false);
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(response.starts_with("HTTP/1.1 502 Bad Gateway"));

    child.kill().expect("kill bridge");
    let _ = child.wait();
    upstream_thread.join().expect("stalled upstream thread");
}
