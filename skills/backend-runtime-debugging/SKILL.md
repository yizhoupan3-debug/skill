---
name: backend-runtime-debugging
description: |
  Diagnose backend runtime failures: crashes, tracebacks, OOM, deadlocks, hanging tasks,
  or silent data corruption in Python, Node.js, Go, or Rust services. Use when the backend
  produces unexpected runtime errors that are NOT API boundary integration issues.
  Trigger: 后端报错、traceback、进程崩溃、内存泄漏、任务卡死、死锁、OOM、数据静默损坏、Go panic。
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - backend
    - debugging
    - runtime
    - traceback
    - oom
    - deadlock
    - crash
    - go
risk: low
source: local
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - backend
  - debugging
  - runtime
  - traceback
  - oom
  - deadlock
allowed_tools:
  - shell
  - browser
  - python
  - node
approval_required_tools:
  - gui automation

---

# backend-runtime-debugging

Backend-specialized debugging skill. Provides language-specific runtime failure investigation
for Python, Node.js, Go, and Rust services that generic `$systematic-debugging` methodology
cannot efficiently cover alone.

## When to use

- Python service: unhandled exception, traceback, MemoryError, threading deadlock, asyncio task hang
- Node.js: unhandled promise rejection, `process.exit` crash, OOM ("JavaScript heap out of memory"), libuv hang
- Rust: `panic!`, unwrap on None/Err, stack overflow, backtrace analysis
- Go: `panic: runtime error`, goroutine deadlock, OOM, goroutine leak, data race (`go test -race`)
- Any backend: silent data corruption, wrong output without visible error, ghost process
- Process management: zombie processes, fd leaks, stuck background workers, task queue stall
- Best for requests like:
  - "后端 traceback 了，帮我看看"
  - "Python 进程 OOM 了/内存一直涨"
  - "Node.js 无法退出/一直 hang"
  - "Rust panic 了，backtrace 怎么看"
  - "Go 程序 panic / goroutine 死锁了"
  - "任务队列卡住了/后台 worker 没反应"
  - "数据写进去了但查出来不对"

## Do not use

- The root cause is completely unknown and needs generic reproduction first → use `$systematic-debugging`, then route here
- The failure is at the API boundary (wrong status code, CORS, auth) → use `$api-integration-debugging`
- The failure is frontend runtime (blank screen, component crash) → use `$frontend-debugging`
- The task is backend feature implementation, not debugging → use `$node-backend` or `$python-pro`
- Build/dependency resolution failure → use `$build-tooling`

## Task ownership and boundaries

This skill owns:
- Backend runtime crash classification and triage
- Language-specific traceback and panic analysis (Python / Node.js / Go / Rust)
- Memory leak investigation and profiling guidance
- Deadlock, goroutine leak, and async hang root-cause isolation
- Silent data corruption tracing

This skill does not own:
- Generic reproduction and hypothesis-testing methodology → `$systematic-debugging`
- API integration correctness → `$api-integration-debugging`
- Build chain failures → `$build-tooling`
- Frontend runtime bugs → `$frontend-debugging`
- **Observability infrastructure** (logging pipelines, metrics dashboards, alerting setup) → `$observability`
  - Route **from** `$observability` **to here** when a log/trace reveals a specific runtime crash or memory issue
  - Route **from here to** `$observability` when the problem is "we don't have enough logs to diagnose"

## Language-specific diagnostic playbooks

### Python
1. Parse the full traceback bottom-up: find the first frame in your own code.
2. Check `sys.exc_info()` or `logging.exception()` for suppressed exceptions.
3. For OOM: use `tracemalloc`, `memory_profiler`, or `objgraph`.
4. For deadlock: use `faulthandler.dump_traceback()` or `py-spy`.
5. For async hang: check `asyncio.all_tasks()` and `loop.run_until_complete()` stalls.

### Node.js
1. Enable `--trace-warnings --trace-uncaught-exceptions` to surface hidden errors.
2. For OOM: `--max-old-space-size` vs heap dumps via `v8.writeHeapSnapshot()`.
3. For async hang: `--inspect` + Chrome DevTools → CPU profiler or async stack traces.
4. For unhandled rejection: check `process.on('unhandledRejection')` handler.
5. For libuv hang: `process._getActiveHandles()` and `process._getActiveRequests()`.

### Go
1. For panic: read the goroutine stack dump (`GOTRACEBACK=all <binary> 2>&1 | head -100`).
2. For deadlock: `runtime.Stack()` or `go tool pprof http://localhost:6060/debug/pprof/goroutine`.
3. For OOM: use `go tool pprof http://localhost:6060/debug/pprof/heap`.
4. For goroutine leak: use `goroutine` pprof profile; look for leaked HTTP/gRPC handlers.
5. For data race: `go test -race ./...` or `GORACE=log_path=/tmp/race go run main.go`.

### Rust
1. Run with `RUST_BACKTRACE=1` or `RUST_BACKTRACE=full`.
2. Identify the panic site; trace through `unwrap()` / `expect()` chains.
3. For stack overflow: check recursion depth, increase with `stacker` crate.
4. For data races: run `cargo test` under `cargo-tsan` or `cargo-miri`.
5. For deadlock: use `parking_lot`'s deadlock detection feature.

## Tool Selection Matrix

| Failure Type | Command / Tool | Expected Output |
|---|---|---|
| Python traceback | `python3 -Xfaulthandler script.py 2>&1` | Full stack with fault handler |
| Python OOM | `python3 -c "import tracemalloc; tracemalloc.start()"` | Memory snapshot |
| Node.js OOM | `node --max-old-space-size=4096 --inspect app.js` | Heap snapshot URL |
| Go panic | `GOTRACEBACK=all ./binary 2>&1 \| head -200` | All goroutine stacks |
| Go pprof | `go tool pprof http://localhost:6060/debug/pprof/heap` | Heap profile |
| Rust panic | `RUST_BACKTRACE=full cargo run 2>&1` | Full Rust backtrace |
| Any: log search | `grep_search` + `run_command` | `grep -r 'ERROR\|PANIC\|panic' logs/` |

## Output defaults

```markdown
## Backend Runtime Debugging Summary
- Language / Runtime: ...
- Failure type: [crash / OOM / deadlock / hang / data-corruption]
- Reproduction: confirmed / partial / blocked

## Evidence
- Error surface: [traceback / panic / log line / symptom]
- Key frame: [file:line]

## Root Cause
- ...

## Fix / Next Step
- ...
```

## Hard constraints

- Always read the full traceback before guessing; do not stop at the last frame.
- Do not conflate OOM with logic bugs — profile first.
- If the bug is intermittent, require a reproduction strategy before proposing a fix.
- Label inferred vs observed root causes clearly.

## References

- [references/traceback-analysis.md](references/traceback-analysis.md) — Python/Node.js/Go/Rust traceback reading guide, common exception patterns, profiling tools comparison
