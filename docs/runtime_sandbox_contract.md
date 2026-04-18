# Runtime Sandbox Contract

This document freezes the sandbox control-plane contract for the next runtime phase. It is intentionally implementation-agnostic and defines the minimum behavior that the runtime, tests, and future workers must preserve.

## Scope

The sandbox contract covers:

- lifecycle state transitions for pooled and per-task sandbox instances
- tool capability policy and high-risk isolation boundaries
- resource budget enforcement and terminalization rules
- asynchronous cleanup semantics
- failure isolation and recoverability boundaries

The contract is derived from `aionrs_fusion_docs/codex_dual_entry_next_phase_checklist.md` and is the source of truth for sandbox lifecycle and policy drift checks.

## Lifecycle States

Sandbox instances must use the following lifecycle states:

- `created`: instance allocated, not yet warmed or assigned
- `warm`: instance is initialized and ready for assignment
- `busy`: instance is executing an active tool or job
- `draining`: instance is no longer eligible for new work and is awaiting cleanup
- `recycled`: instance passed cleanup and returned to the reusable pool
- `failed`: instance is terminally unhealthy and must not be reused

Allowed transitions:

- `created -> warm`
- `warm -> busy`
- `busy -> draining`
- `draining -> recycled`
- `draining -> failed`
- `warm -> failed`
- `busy -> failed`
- `recycled -> warm`

Invalid transitions must be rejected by policy validation rather than inferred at runtime.

## Tool Capability Policy

Capability policy must be explicit and sandbox-scoped. Tool execution is permitted only when the sandbox profile authorizes the requested tool class.

Required policy rules:

- capabilities are declared per sandbox profile, not guessed from runtime behavior
- high-risk tools must use a dedicated sandbox profile
- sandbox reuse must preserve capability boundaries; a recycled sandbox may not expand privileges
- deny-by-default is the fallback for missing or unknown capability declarations

Capability categories:

- `read_only`: inspection and retrieval tools
- `workspace_mutating`: tools that can write or transform workspace files
- `networked`: tools that can reach external services or perform non-local IO
- `high_risk`: tools that can execute arbitrary code, spawn child processes, or trigger destructive side effects

The policy must surface the effective capability set in traces or durable events so that a tool decision can be audited after recovery.

## Resource Budgets

Sandbox execution must enforce budgets before and during execution. Budgets are part of the contract, not best-effort hints.

Required budget dimensions:

- CPU budget
- memory budget
- memory budget is tracked in bytes after host-specific runtime accounting normalization
- wall-clock budget
- output-size budget

Budget enforcement rules:

- budgets must be attached to the sandbox execution request
- budget checks must occur at admission time and at runtime
- any exceeded budget must transition the sandbox into `draining`
- budget enforcement must produce a durable failure reason
- output-size pressure must not be hidden behind generic timeout errors

Recommended terminal behavior:

- `cpu_exceeded` and `memory_exceeded` should fail the current execution and drain the sandbox
- `wall_clock_exceeded` should request termination and cleanup
- `output_size_exceeded` should truncate or refuse output according to policy, then drain or fail the sandbox

## Async Cleanup

Sandbox cleanup is asynchronous and must be observable.

Cleanup requirements:

- cleanup starts when a sandbox enters `draining`
- cleanup must release temp files, child processes, sockets, and any sandbox-local handles
- cleanup completion must be recorded as a durable event
- cleanup may be retried, but retries must be idempotent
- a sandbox may only enter `recycled` after cleanup success
- cleanup failures must transition the sandbox to `failed`

Cleanup is not optional follow-up work. It is part of the state machine and must be visible to control-plane monitoring.

## Failure Isolation

Failures inside one sandbox must not contaminate other sandboxes or the host runtime.

Isolation requirements:

- a failed sandbox must be quarantined from the reusable pool
- tool crashes, timeouts, and policy violations must be contained to the owning sandbox
- high-risk profiles must not share execution state with low-risk profiles
- partial cleanup failure must not re-enable a sandbox for unrelated work
- failure telemetry must include the sandbox profile, state transition, and durable failure reason

Pool reuse is allowed only when cleanup succeeded and the sandbox remains within its authorized profile.

## Recoverability Boundary

The runtime must distinguish between recoverable and non-recoverable sandbox failures.

Recoverable boundary:

- transient timeout
- transient kill request
- cleanup retry after a failed async cleanup attempt
- takeover after control-plane interruption, if the sandbox is still policy-compliant

Non-recoverable boundary:

- repeated cleanup failure
- policy violation that invalidates the sandbox profile
- contamination of sandbox-local state that cannot be deterministically cleared
- any state where reuse would require privilege expansion or hidden host repair

Recovery may restore control-plane observability and may recycle a healthy sandbox, but it must not silently resurrect a sandbox that has crossed the non-recoverable boundary.

## Machine-Readable Contract

The following schema is the canonical contract snapshot used by tests to detect drift.

```json sandbox-contract-v1
{
  "schema_version": "runtime-sandbox-contract-v1",
  "lifecycle_states": [
    "created",
    "warm",
    "busy",
    "draining",
    "recycled",
    "failed"
  ],
  "allowed_transitions": [
    ["created", "warm"],
    ["warm", "busy"],
    ["busy", "draining"],
    ["draining", "recycled"],
    ["draining", "failed"],
    ["warm", "failed"],
    ["busy", "failed"],
    ["recycled", "warm"]
  ],
  "tool_capability_categories": [
    "read_only",
    "workspace_mutating",
    "networked",
    "high_risk"
  ],
  "tool_policy_rules": [
    "capabilities are declared per sandbox profile, not guessed from runtime behavior",
    "high-risk tools must use a dedicated sandbox profile",
    "sandbox reuse must preserve capability boundaries",
    "deny-by-default is the fallback for missing or unknown capability declarations",
    "effective capabilities must be recorded in traces or durable events"
  ],
  "resource_budgets": [
    "cpu",
    "memory",
    "wall_clock",
    "output_size"
  ],
  "budget_enforcement_rules": [
    "budgets must be attached to the sandbox execution request",
    "budget checks must occur at admission time and at runtime",
    "any exceeded budget must transition the sandbox into draining",
    "budget enforcement must produce a durable failure reason",
    "output-size pressure must not be hidden behind generic timeout errors"
  ],
  "async_cleanup_rules": [
    "cleanup starts when a sandbox enters draining",
    "cleanup must release temp files, child processes, sockets, and sandbox-local handles",
    "cleanup completion must be recorded as a durable event",
    "cleanup retries must be idempotent",
    "a sandbox may only enter recycled after cleanup success",
    "cleanup failures must transition the sandbox to failed"
  ],
  "failure_isolation_rules": [
    "a failed sandbox must be quarantined from the reusable pool",
    "tool crashes, timeouts, and policy violations must be contained to the owning sandbox",
    "high-risk profiles must not share execution state with low-risk profiles",
    "partial cleanup failure must not re-enable a sandbox for unrelated work",
    "failure telemetry must include the sandbox profile, state transition, and durable failure reason"
  ],
  "recoverability_boundary": {
    "recoverable": [
      "transient timeout",
      "transient kill request",
      "cleanup retry after a failed async cleanup attempt",
      "takeover after control-plane interruption when policy-compliant"
    ],
    "non_recoverable": [
      "repeated cleanup failure",
      "policy violation that invalidates the sandbox profile",
      "contamination of sandbox-local state that cannot be deterministically cleared",
      "any state where reuse would require privilege expansion or hidden host repair"
    ]
  }
}
```

## Drift Rule

Any future implementation, schema, or runtime change that alters one of the machine-readable fields above must update this contract first, then update the corresponding implementation and tests in the same change.

## Current Minimal Implementation Status

R8 now lands a contract-backed minimal implementation in the Python host without re-promoting Python to default authority:

- `ExecutionEnvironmentService` routes every kernel request through an explicit sandbox lifecycle manager instead of ad-hoc inline handling
- lifecycle transitions are validated against the frozen state graph before the kernel delegate runs
- capability policy is request-scoped, deny-by-default, and rejects high-risk execution unless the sandbox profile is dedicated
- budgets are attached to every execution request; admission validates all four dimensions, while runtime enforcement checks wall-clock, output size, and host-visible CPU or memory probes
- cleanup is asynchronous, observable, and recorded in `runtime_sandbox_events.jsonl`
- cleanup success is the only path back to `recycled`; cleanup failure quarantines the sandbox as `failed`
- failed sandboxes stay out of the reusable pool, while healthy sandboxes may only be reused under the same profile and capability set

The current minimal implementation is intentionally scoped:

- it provides deterministic lifecycle, policy, budget, and cleanup behavior for the execution seam
- it does not yet claim a remote sandbox backend or a full out-of-process resource governor
- future work may move this host into Rust, but that migration must preserve the frozen contract above instead of weakening it
