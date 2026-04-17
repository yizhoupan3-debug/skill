# Backend Traceback & Panic Analysis Guide

> Quick reference for parsing crash output across Python, Node.js, Go, and Rust.

## Python Traceback

### Reading a Python traceback

```
Traceback (most recent call last):     ← always read top-to-bottom
  File "app.py", line 42, in handler   ← outermost call (entry point)
    result = process(data)
  File "core.py", line 17, in process  ← intermediate frame
    return transform(item)
  File "core.py", line 8, in transform ← your code! start here
    return item["key"]
KeyError: 'key'                         ← exception type + message
```

**Rule**: find the first frame in YOUR code (not stdlib/vendor) going bottom-up.

### Common Python exceptions

| Exception | Usual Cause |
|---|---|
| `KeyError` | dict key does not exist; use `.get()` or check membership |
| `AttributeError: 'NoneType' object...` | Function returned `None`; check callee return paths |
| `ImportError / ModuleNotFoundError` | Missing dependency or wrong virtual env active |
| `RecursionError` | Deep or infinite recursion; add base case |
| `MemoryError` | OOM; profile with `tracemalloc` |
| `RuntimeError: Event loop is closed` | asyncio task used after loop teardown |

### Suppress-aware exception detection

```python
# Find swallowed exceptions:
import logging
logging.basicConfig(level=logging.DEBUG)
# Or add to suspect try/except:
except Exception:
    logging.exception("Swallowed exception at X")
    raise  # always re-raise in debug mode
```

## Node.js Crash Output

### Unhandled rejection format

```
UnhandledPromiseRejectionWarning: TypeError: Cannot read property 'x' of undefined
    at Object.<anonymous> (/app/handler.js:23:15)   ← your code
    at processTicksAndMicrotasks (internal/process/...)  ← Node internals (ignore)
```

### OOM crash format

```
FATAL ERROR: CALL_AND_RETRY_LAST Allocation failed - JavaScript heap out of memory
 1: 0xb7c5e0 node::Abort() [node]
```
→ Run `node --max-old-space-size=8192` and capture a heap snapshot.

### Async hang diagnosis

```bash
node --inspect app.js
# Open chrome://inspect → CPU profiler → record 10s → look for idle time
# OR:
node -e "setInterval(() => console.log(process._getActiveHandles().length), 1000)" &
node app.js
```

## Go Panic Output

### Reading a Go panic

```
goroutine 1 [running]:
main.processItem(...)            ← start here (your code)
	/app/main.go:42 +0x3c
main.main()
	/app/main.go:15 +0x25

goroutine 7 [chan receive]:       ← goroutines waiting (may indicate deadlock)
```

### Goroutine deadlock

```
fatal error: all goroutines are asleep - deadlock!
goroutine 1 [chan receive]:        ← blocked goroutines
goroutine 18 [chan send]:
```
→ Use `GOTRACEBACK=all` to see ALL goroutines, not just the panicking one.

### Go profiling commands

```bash
# Heap profile (OOM investigation)
go tool pprof http://localhost:6060/debug/pprof/heap

# Goroutine leak
go tool pprof http://localhost:6060/debug/pprof/goroutine

# CPU profile (30s sample)
go tool pprof http://localhost:6060/debug/pprof/profile?seconds=30

# Data race at test time
go test -race ./...
```

## Rust Panic Output

### Reading a Rust backtrace

```
thread 'main' panicked at 'called `Option::unwrap()` on a `None` value', src/main.rs:42:5
stack backtrace:
   0: rust_begin_unwind
   ...
  12: myapp::process       ← your code! (search for your crate name)
        at src/main.rs:42:5
  13: myapp::main
        at src/main.rs:15:5
```

Set `RUST_BACKTRACE=full` for more frames, `RUST_BACKTRACE=1` for a pruned view.

### Common Rust panics

| Panic Message | Cause | Fix |
|---|---|---|
| `called unwrap() on None` | Missing `?` or unhandled None | Use `ok_or(...)` or `if let Some(x) =` |
| `index out of bounds: len is N, index is N` | Off-by-one | Add bounds check |
| `thread panicked while panicking` | Double panic | Check `Drop` implementations |
| `attempt to subtract with overflow` | Integer underflow in debug mode | Use `checked_sub` or `saturating_sub` |
| `stack overflow` | Deep recursion | Use `stacker::grow` or rewrite iteratively |

## Profiling Tools Comparison

| Tool | Language | Use Case | Command |
|---|---|---|---|
| `tracemalloc` | Python | Memory allocation tracking | `python3 -c "import tracemalloc; tracemalloc.start()"` |
| `py-spy` | Python | CPU profiling (production-safe) | `py-spy top --pid PID` |
| `memory_profiler` | Python | Line-by-line memory usage | `mprof run script.py && mprof plot` |
| `clinic.js` (node-clinic) | Node.js | CPU/memory/async diagnosis | `clinic doctor -- node app.js` |
| `0x` | Node.js | Flame graph | `npx 0x -- node app.js` |
| `pprof` | Go | CPU/heap/goroutine profiling | `go tool pprof http://localhost:6060/...` |
| `cargo-flamegraph` | Rust | CPU flame graph | `cargo flamegraph` |
| `cargo-miri` | Rust | UB/data race detection | `cargo miri test` |
