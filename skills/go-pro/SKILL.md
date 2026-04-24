---
name: go-pro
description: |
  Deliver safe concurrent Go code with managed goroutine lifecycles, composable
  interfaces, and explicit error handling. Produces services and CLIs that pass
  `go vet`, `golangci-lint`, and `-race` tests — avoiding goroutine leaks,
  unchecked errors, and unnecessary `interface{}`. Use when the user asks for Go
  development, microservices, CLI tools, concurrency design, or phrases like
  "Go 项目", "goroutine", "并发", "Go 微服务", "错误处理策略".
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - go
    - goroutines
    - channels
    - microservices
    - concurrency
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - Go 项目
  - goroutine
  - 并发
  - Go 微服务
  - 错误处理策略
  - Go development
  - microservices
  - CLI tools
  - concurrency design
  - go
---

# go-pro

This skill owns Go-first engineering work: concurrency design, interface-based abstraction, error handling, and production HTTP services.

## When to use

- The user wants to build, refactor, or debug a Go codebase
- The task involves goroutines, channels, or concurrency patterns
- Building HTTP services, CLI tools, or infrastructure in Go
- Best for requests like:
  - "用 Go 写一个微服务"
  - "goroutine 泄漏怎么排查"
  - "帮我设计 Go 的错误处理策略"
  - "优化这个 Go 并发代码"

## Do not use

- The task is Rust/Python/JS → use language-specific skills
- The task is pure infrastructure/DevOps without Go code
- The task is algorithm design without language-specific needs

## Task ownership and boundaries

This skill owns:
- Go concurrency patterns (goroutines, channels, sync primitives)
- interface design and composition
- error handling with `error` interface and custom types
- Go module management and dependency strategy
- HTTP service design (stdlib, Chi, Gin, Echo)
- testing with `go test`, table-driven tests, and benchmarks

This skill does not own:
- non-Go language tasks
- pure infrastructure without Go code context

### Overlay interaction rules

- Language-idiomatic error handling (Go `error` interface, sentinel errors, custom types, wrapping) is owned by this skill
- Cross-language error **architecture** (error taxonomies, retry/circuit-breaker, error code systems) → `$error-handling-patterns`
- Go-specific profiling (pprof, race detection, benchmark) is owned by this skill
- Web frontend performance audits → `$performance-expert`

## Required workflow

1. Confirm the task shape:
   - object: Go file, package, module, service, CLI
   - action: build, refactor, debug, optimize, review
   - constraints: Go version, module structure, deployment target
   - deliverable: code change, design, review guidance
2. Check Go version and module configuration.
3. Run `go vet` and `golangci-lint` after modifications.
4. Verify with `go test ./...`.

## Core workflow

### 1. Intake
- Identify Go version and module structure.
- Check existing patterns (error handling, interface usage, concurrency).
- Inspect go.mod for dependencies and replace directives.

### 2. Execution
- Design with small, composable interfaces.
- Handle errors explicitly; don't ignore returned errors.
- Use goroutines with proper lifecycle management (context, WaitGroup).
- Prefer channels for communication, mutexes for shared state.
- Follow stdlib conventions and idiomatic Go patterns.

### 3. Validation / recheck
- Run `go vet ./...` for static analysis.
- Run `golangci-lint run` if available.
- Run `go test -race ./...` for race condition detection.
- Check for goroutine leaks in long-running services.

## Capabilities

### Core Language
- Goroutines, channels, select, and context propagation
- Interfaces and type embedding (composition over inheritance)
- Error handling patterns (sentinel errors, custom types, wrapping)
- Generics (Go 1.18+) and type constraints
- Stdlib patterns and idiomatic Go
- **Critical implementation auditing (Memory, Speed, Platform-native)** → `$execution-audit` [Overlay]

If the task shifts to adjacent skill territory, route to:
- Chi, Gin, Echo router frameworks
- gRPC with protobuf
- Middleware patterns and request lifecycle
- Graceful shutdown and health checks

### Web Services
- `net/http` stdlib server and client
- Chi, Gin, Echo router frameworks
- gRPC with protobuf
- Middleware patterns and request lifecycle
- Graceful shutdown and health checks

### Concurrency
- Worker pools and fan-out/fan-in
- Rate limiting and backpressure
- Context cancellation and timeout
- sync.Pool, sync.Once, sync.Map
- Atomic operations

### Testing
- Table-driven tests
- Subtests and test helpers
- Benchmarks (`go test -bench`)
- Fuzz testing (Go 1.18+)
- Integration test patterns

### Tooling
- Go modules and workspace mode
- golangci-lint configuration
- go generate and code generation
- pprof profiling
- Delve debugger

## Output defaults

Recommended structure:

````markdown
## Go Summary
- Go version: ...
- Module: ...

## Changes / Guidance
- ...

## Validation / Risks
- go vet: ...
- Tests: ...
- Race detection: ...
````

## Hard constraints

- Do not ignore returned errors; handle or explicitly discard with `_ =`.
- Do not launch goroutines without lifecycle management (context or WaitGroup).
- Do not use `panic` for expected error conditions.
- Do not use `init()` for complex initialization; prefer explicit setup.
- Prefer stdlib solutions over third-party when equivalent.
- Do not use `interface{}` / `any` when a specific type or generic works.

## Trigger examples

- "Use $go-pro to build a concurrent HTTP service."
- "帮我用 Go 写一个并发安全的 worker pool。"
- "这个 goroutine 泄漏怎么解？"
- "设计一个 Go 微服务的错误处理策略。"
