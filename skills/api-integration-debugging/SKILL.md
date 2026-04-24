---
name: api-integration-debugging
description: |
  Diagnose and fix API integration failures at service boundaries. Produces reproducible request
  examples, pinpoints the failure layer (auth, transport, schema, rate-limit, or CORS), and
  delivers working call patterns with verified responses. Use when: 对接接口、联调 API、排查接口报错、
  debug 401/403/429/5xx、trace request-response mismatches、调 curl/fetch/axios、看接口文档写调用。
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - api
    - rest
    - graphql
    - websocket
    - openapi
    - cors
    - integration
risk: medium
source: local
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
trigger_hints:
  - api
  - rest
  - graphql
  - websocket
  - openapi
  - cors
allowed_tools:
  - shell
  - browser
  - python
  - node
approval_required_tools:
  - gui automation
---

# api-integration-debugging

> [!NOTE]
> 若 API 调用完全无法复现（没有 status code、没有 error body、现象描述模糊），先过 `$systematic-debugging` gate 定位根因，再路由至此。
> 已知 status code 或 error body 时可直接进入本 skill。

This skill owns request/response integration work when the problem is making systems talk correctly over API boundaries rather than building one service's internal business logic.

## When to use

- The user needs to integrate with a third-party or internal API
- The task involves REST, GraphQL, WebSocket, SSE, OpenAPI, fetch, axios, curl, or Postman-style debugging
- The user is hitting auth, header, payload, pagination, retry, rate-limit, signature, or CORS issues
- The task is to compare docs vs observed responses, or reproduce an API failure with concrete requests
- Best for requests like:
  - "帮我联调这个第三方 API"
  - "为什么这个接口一直 401/403/429"
  - "看一下这个 OpenAPI/接口文档，然后写调用代码"
  - "排查 WebSocket / SSE / GraphQL 请求为什么不通"

## Do not use

- The main task is implementing backend route/controller/service internals → use `$node-backend` or `$python-pro`
- The task is webhook receiver security and verification boundaries → use `$webhook-security`
- The task is API load/stress benchmarking rather than correctness/debugging → use `$api-load-tester`
- The task is high-level threat modeling instead of concrete API integration behavior → use `$security-threat-model`

## Task ownership and boundaries

This skill owns:
- API request construction and response interpretation
- auth headers, cookies, tokens, signing, and transport-level debugging
- REST/GraphQL/WebSocket/SSE integration mismatches
- pagination, retries, idempotency, and rate-limit handling
- reproducing and isolating API failures with concrete evidence

This skill does not own:
- deep business logic inside one service
- full webhook receiver design
- performance benchmarking at load scale
- generic frontend state management around API data

If the task shifts to adjacent skill territory, route to:
- `$node-backend`
- `$python-pro`
- `$webhook-security`
- `$api-load-tester`
- `$security-audit`

## Required workflow

1. **Gate check**: if the failure has no status code or observable error surface yet, confirm root cause is known before proceeding. If unknown, apply `$systematic-debugging` first.
2. Confirm the task shape:
   - object: endpoint, schema, subscription, request flow, client integration
   - action: integrate, debug, review, reproduce, harden
   - constraints: auth method, protocol, docs source, environment, rate limits
   - deliverable: working call pattern, diagnosis, patch, or request examples
3. Reconstruct the exact request and expected response before guessing.
4. Compare docs, code, and observed wire behavior.
5. Isolate whether the failure is transport, auth, schema, state, timing, or environment.
6. Deliver reproducible requests and concrete fixes.

## Core workflow

### 1. Intake
- Gather endpoint URL, method/protocol, headers, auth mode, body/query shape, and environment.
- Inspect official docs/OpenAPI/examples first when available.
- Capture the actual error surface: status code, response body, timing, handshake failure, CORS detail, or disconnect pattern.

### 2. Execution
- Reproduce with the smallest concrete request possible.
- Validate auth, header names, content type, body encoding, query params, and path variables.
- Check docs-vs-runtime mismatches, versioning, required fields, and protocol expectations.
- For WebSocket/SSE, verify handshake, upgrade/stream semantics, heartbeat/reconnect, and message schema.
- For GraphQL, inspect operation shape, variables, auth, schema errors, and partial-data-plus-errors behavior.
- For rate limits/retries/idempotency, separate client mistakes from provider-enforced constraints.

### 3. Validation / recheck
- Re-run the minimal reproduction after the fix.
- Verify success and common failure cases.
- Leave behind a concrete curl/fetch/axios/WebSocket example when helpful.
- If docs are ambiguous or behavior seems provider-buggy, mark that explicitly.

## Output defaults

Default output should contain:
- the failing boundary and likely cause
- a reproducible request pattern or integration example
- validation notes and remaining uncertainty

Recommended structure:

````markdown
## API Integration Summary
- Protocol / endpoint: ...
- Main failure: ...

## Findings / Fix
- ...

## Reproduction / Validation
- Request example: ...
- Verified: ...
- Open questions: ...
````

## Protocol Tool Selection Matrix

Always gather concrete evidence before proposing a fix:

| Protocol | Primary Evidence Tool | Key Action |
|---|---|---|
| REST (HTTP) | `run_command` | `curl -v -X METHOD URL -H "..." -d '{...}' 2>&1` |
| REST (browser) | `browser_get_network` | `resourceTypes=["fetch","xhr"], sinceSeconds=30` |
| GraphQL | `run_command` | `curl -X POST -H "Content-Type: application/json" -d '{"query":"..."}' URL` |
| WebSocket | `run_command` + `browser_get_network` | `websocat ws://... --protocol ...` or DevTools WS frames |
| SSE | `run_command` | `curl -N -H "Accept: text/event-stream" URL` |
| OAuth / Token | `run_command` | `curl -X POST token_url -d "grant_type=...&client_id=..."` |
| CORS | `browser_get_network` | Capture preflight OPTIONS + status; check `Access-Control-Allow-*` |

**Minimum reproduction rule**: always produce a `curl` or equivalent one-liner before proposing any code fix.

## Hard constraints

- Do not debug APIs from vague symptoms alone; reconstruct the real request.
- Do not assume docs are correct when runtime evidence disagrees.
- Do not collapse auth, transport, schema, and rate-limit failures into one bucket.
- Always preserve exact status codes, protocol errors, or provider messages when available.
- If a conclusion is inferred rather than observed, label it clearly.
- **No passive finish**: never claim "the API call should work now" without showing a successful `curl` or network trace.
- **No context-begging**: capture `status code + response body` with a tool before asking the user for more info.

## Trigger examples

- "Use $api-integration-debugging to troubleshoot this third-party REST API integration."
- "帮我排查 GraphQL / WebSocket 联调为什么一直失败。"
- "看 OpenAPI 文档并给我最小可用 curl/fetch 调用示例。"

## References

- [references/status-code-quick-ref.md](references/status-code-quick-ref.md) — HTTP status codes, CORS patterns, WebSocket failures, OAuth checklist, GraphQL error patterns
