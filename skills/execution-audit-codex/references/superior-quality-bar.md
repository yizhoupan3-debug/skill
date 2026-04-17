# Superior Quality Bar (Benchmarks)

Use these benchmarks to audit code for "Superior Quality". These go beyond standard naming/style to ensure production-grade robustness and performance.

## 1. Defensive-by-Default
- **State Validity**: Invalid states are made unrepresentable (e.g., via Branded Types, Discriminated Unions, or strict Enums).
- **Early Returns**: All preconditions are checked at the start (Guard Clauses).
- **Error Boundaries**: Every async boundary has a clear error-handling/retry/fallback strategy.
- **Input Sanitization**: Zero trust for external inputs (API, DB, UI).

## 2. Perfect Execution
- **Full Path Wiring**: A new feature must be reachable from the main UI link, API route, and CLI entrypoint. "Ghost features" are unacceptable.
- **Resource Management**: Explicit cleanup for listeners, timers, file handles, and memory buffers.
- **Dependency Safety**: No hidden side-effects. Pure logic is separated from I/O where possible.

## 3. Idiomatic Excellence
- **Zero-Allocation (Perf)**: Minimize unnecessary object/array spreads in hot paths.
- **Strict Typing**: No `any`, `unknown` (without narrowing), or `as` (without safety check).
- **Concurrency Safety**: Lock-free where possible; atomic or explicitly synchronized where needed.
- **Complexity Cap**: No function > 50 lines; No nesting > 3 levels.

## 4. Observability & Supportability
- **Semantic Logging**: Logs include context (correlation IDs, state snapshots), not just "Error occurred".
- **Documentation Fidelity**: Public APIs have JSDoc/Docstrings; complex logic has "Why" comments.
- **Test Integrity**: Every logic branch has at least one unit/integration test. No mock-heavy shells.

## 5. Visual & UX Polish (if applicable)
- **State Feedback**: Loading, Success, Error, and Empty states are all implemented.
- **Micro-interactions**: Hover, Active, and Focus states are styled.
- **Resilience**: Slow network or backend failure does not crash the UI.

## 6. Platform-Native Excellence (System Integrity)
- **Architecture Native**: Execution runs natively on the target architecture (e.g., ARM64/Apple Silicon, x86_64/AVX-512). Avoid emulation or translation overhead.
- **UMA / Locality**: Awareness of Unified Memory Architecture or NUMA. Minimize data copies and maximize cache hits.
- **QoS & Scheduling**: Correct use of task priorities (Quality of Service) to ensure background tasks don't starve interactive ones.
- **Hardware Acceleration**: Proactively use optimized system frameworks (e.g., Accelerate, Metal, CUDA) for heavy computation.

## 7. Performance & Speed (Execution Efficiency)
- **Time Complexity**: Formal audit of O(n) for critical paths. Prefer O(1) or O(log n) where feasible.
- **Hot Path Purity**: Zero unnecessary allocations or I/O in tight loops or render cycles.
- **Async Efficiency**: Use Non-blocking I/O and batching for external calls.
- **Initialization Speed**: Lazy-load expensive modules; minimize cold start/boot time.

## 8. Memory Stewardship (Resource Discipline)
- **Heap vs Stack**: Prefer stack allocation for short-lived data.
- **ARC / GC Optimization**: minimize reference cycles and avoid "Memory Bloat" from long-lived caches without TTL.
- **Zero-Copy Patterns**: Use buffers or views (e.g., `Memory<T>`, `Span<T>`, `Buffer`) to pass large data without allocation.
- **Leak Prevention**: Explicit lifecycle management for external resources (sockets, file descriptors, GPU buffers).

## 9. Result Idealism (The "Perfect Execution" Standard)
- **100% Functional Match**: The output does exactly what was asked, with zero missing features or "TODOs".
- **Aesthetic/Logical Elegance**: The result is not just "working" but "sophisticated". For code, this means high cohesion; for UI, this means pixel-perfect polish.
- **Empirical Proof**: Audit results must be backed by screenshots, terminal logs, or test reports. No "blind trust".
- **Seamless Resilience**: The implementation handles unexpected environment states (missing dirs, offline state) without user intervention.
