---
name: error-handling-patterns
description: |
  Design cross-language error-handling architectures such as custom errors,
  retry/backoff, circuit breakers, error boundaries, and graceful degradation.
  Use for “设计错误处理体系”, “统一错误码”, “error propagation”, or “circuit
  breaker” across JS/TS, Python, Rust, Go, and frontend/backend boundaries.
  Best for error-handling design, not debugging a specific bug.
routing_layer: L1
routing_owner: overlay
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 设计错误处理体系
  - 统一错误码
  - error propagation
  - circuit breaker
  - error handling
  - retry
  - error boundary
  - custom error
  - graceful degradation
allowed_tools:
  - shell
  - python
approval_required_tools: []
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - error-handling
    - retry
    - circuit-breaker
    - error-boundary
    - custom-error
    - graceful-degradation
    - result-type
risk: low
source: local

---

# error-handling-patterns

This skill provides cross-language error handling architecture guidance as an
overlay: custom error design, propagation strategy, retry/backoff, circuit
breaking, and graceful degradation patterns.

## When to use

- The user wants to design or refactor a project's error handling architecture
- The task involves creating custom error hierarchies or error class taxonomies
- The user needs retry/backoff, circuit breaker, or timeout patterns
- The task involves error boundary design (React, backend middleware, or cross-service)
- The user wants to unify error codes, error serialization, or error reporting
- The task involves Result/Either/Option patterns (Rust-style error handling in TS/Python)
- The user needs graceful degradation or fallback strategies
- Best for requests like:
  - "帮我设计这个项目的错误处理体系"
  - "写一套自定义 Error class"
  - "怎么做 retry with exponential backoff"
  - "设计一下 circuit breaker 模式"
  - "前后端错误码怎么统一"
  - "怎么实现 graceful degradation"

## Do not use

- The task is debugging a specific bug → use `$systematic-debugging`
- The task is code style / naming / formatting → use `$coding-standards`
- The task is logging / metrics / tracing infrastructure → use `$observability`
- The task is API error response design (HTTP status codes, error schema) → use `$api-design`
- The task is **language-idiomatic error syntax** only (Go `error` interface, Rust `Result<T,E>`, Python `try/except`, JS `try/catch`) without architectural design → use the relevant language pro skill

> **Decision guide**: If the user asks "how do I handle errors in Go", that's `$go-pro`. If the user asks "design an error taxonomy with retry strategy across my microservices", that's `$error-handling-patterns`.

## Task ownership and boundaries

This skill owns:
- Error hierarchy and taxonomy design (custom error classes, error codes, error categories)
- Error propagation strategy (throw vs return, error wrapping, stack trace preservation)
- Retry patterns (exponential backoff, jitter, max retries, idempotency guards)
- Circuit breaker pattern (states, thresholds, half-open probing, fallbacks)
- Graceful degradation (feature flags, fallback data, partial success handling)
- Error boundary design (React error boundaries, middleware catch-all, top-level handlers)
- Result/Either/Option patterns in non-native languages
- Error serialization and cross-boundary error mapping (frontend/backend, service/service)

This skill does not own:
- Bug diagnosis and root cause analysis
- Code formatting and naming conventions
- Logging/metrics/tracing infrastructure
- API design and HTTP status code selection
- Language-specific idiomatic patterns in isolation

## Core workflow

### 1. Assess

- Identify current error handling patterns in the codebase
- Map error propagation paths: where errors originate, how they flow, where they're caught
- Identify gaps: unhandled cases, swallowed errors, inconsistent patterns
- Determine error consumers: logs, monitoring, user-facing messages, upstream callers

### 2. Design

- Define error taxonomy: categories, severity levels, recoverability
- Choose error representation: classes, codes, Result types, or union types
- Design propagation strategy: when to throw, when to return, when to wrap
- Plan retry/backoff policy for transient failures
- Design circuit breaker if calling unreliable external services
- Plan graceful degradation for non-critical failures

### 3. Implement

- Create base error classes and per-domain error subtypes
- Implement retry utilities with configurable backoff and max attempts
- Add circuit breaker if applicable
- Set up error boundaries at appropriate levels (route, component, middleware)
- Implement error serialization for cross-boundary communication
- Add error reporting hooks (to logging, monitoring, user notification)

### 4. Verify

- Test happy path, expected error cases, and edge cases
- Verify retry behavior with simulated transient failures
- Test circuit breaker state transitions
- Confirm error messages are useful to each consumer (dev, ops, end user)
- Check that no errors are silently swallowed

## Output defaults

```markdown
## Error Handling Design
- Scope: [project/module/service]
- Pattern: [hierarchy / Result type / error codes / mixed]

## Error Taxonomy
| Category | Code Range | Retryable | User-facing |
|---|---|---|---|

## Patterns Applied
- Retry: [strategy]
- Circuit breaker: [thresholds]
- Degradation: [fallback strategy]

## Verification
- Test coverage: ...
- Edge cases checked: ...
```

## Hard constraints

- Never silently swallow errors; always log, propagate, or explicitly handle
- Preserve stack traces and error context through wrapping
- Make retry policies configurable, not hard-coded
- Separate internal error details from user-facing messages
- Document the error taxonomy where consumers can reference it
- Prefer fail-fast for programming errors, retry for transient infrastructure errors

## Trigger examples

- "Use $error-handling-patterns to design a retry strategy with exponential backoff."
- "帮我设计这个项目的错误处理体系。"
- "写一套自定义 Error class 和错误码。"
- "怎么实现 circuit breaker？"
- "前后端错误码怎么统一？"
