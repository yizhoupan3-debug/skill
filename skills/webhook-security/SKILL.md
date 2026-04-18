---
name: webhook-security
description: |
  Secure webhook receivers and callback endpoints for Stripe, GitHub, Slack,
  Clerk, payment, and SaaS integrations. Use when implementing or reviewing
  签名校验、raw body 处理、重放防护、timestamp tolerance、幂等、重试去重、队列解耦、
  or trust boundaries around webhook callbacks. Best for Stripe/GitHub/Slack/
  Clerk webhook security work rather than generic API authentication or broad
  application security reviews.
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - webhook
    - security
    - signature-verification
    - replay-protection
    - idempotency
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - webhook
  - security
  - signature verification
  - replay protection
  - idempotency
---

# webhook-security

This skill owns implementation and review of webhook receiver security and reliability boundaries.

## When to use

- The user wants to implement a webhook handler securely
- The user wants to review callback endpoint security
- The user asks about signature verification, replay defense, or idempotency
- The user is integrating Stripe, GitHub, Slack, Clerk, or similar providers
- Best for requests like:
  - "帮我实现 Stripe webhook 验签"
  - "检查这个 webhook handler 安不安全"
  - "怎么防 webhook replay attack"

## Do not use

- The task is generic REST API auth without webhook semantics
- The task is high-level threat modeling without implementation detail → use `$security-threat-model`
- The task is general code security review not centered on webhook/callback boundaries → use `$security-audit`

## Task ownership and boundaries

This skill owns:
- signature verification
- raw-body handling
- replay/timestamp checks
- idempotency and deduplication
- fast ack + async processing boundaries
- provider-specific webhook verification pitfalls

This skill does not own:
- generic API auth design
- whole-system threat modeling as the primary task
- unrelated application security review

If the task shifts to adjacent skill territory, route to:
- `$security-audit` for broader implementation security defects
- `$security-threat-model` for higher-level abuse-path analysis

## Required workflow

1. Identify provider, framework, and current handler shape.
2. Verify the request-body and signature scheme first.
3. Check replay/idempotency controls next.
4. Check processing and retry boundaries.
5. Deliver concrete fixes or implementation guidance.

## Core workflow

### 1. Intake

- Determine:
  - provider: Stripe / GitHub / Slack / Clerk / other
  - framework: Next.js / Express / FastAPI / etc.
  - whether the task is implement vs review
  - whether the handler already exists

### 2. Core security checks

#### Verify before process
- signature check before business logic
- reject invalid requests early

#### Preserve raw body
- many providers sign the raw payload
- parse JSON only after verification if the provider requires raw-body checking

#### Use timing-safe compare
- never compare signatures with plain string equality

#### Defend against replay
- verify timestamp/tolerance where applicable
- reject stale requests

#### Make processing idempotent
- store event id or delivery id
- make side effects safe under retries

#### Return fast
- ack quickly
- offload heavy processing to queues/background work when appropriate

### 3. Provider-specific notes

#### Stripe
- prefer official SDK verification such as `constructEvent`

#### GitHub
- verify `x-hub-signature-256`

#### Slack
- verify timestamp + signature base string

#### Generic SaaS
- confirm exact signature header, raw-body requirement, and replay window

### 4. Framework-specific pitfalls

- Next.js: raw-body access differs by runtime/handler style
- Express: body parsers can break signature verification if misconfigured
- Python frameworks: confirm exact raw request body retrieval before parsing

## Output defaults

Default output should contain:
- current risks
- exploit or failure path
- concrete fix guidance

Recommended structure:

````markdown
## Webhook Security Summary
- Provider: ...
- Framework: ...

## Findings
1. Risk: ...
   - Why it matters: ...
   - Fix: ...

## Implementation Guidance
- ...

## Remaining Risks / Assumptions
- ...
````

## Hard constraints

- Do not process the event before verification.
- Do not lose the raw body if the provider depends on it for signature validation.
- Do not ignore duplicate delivery behavior.
- Do not print webhook secrets or sensitive headers.
- Clearly distinguish confirmed defects from missing-information risk.

## Trigger examples

- "Use $webhook-security to review this GitHub webhook endpoint."
- "Implement secure Stripe webhook verification with replay and idempotency protections."
- "检查这个 callback handler 有没有原始 body 和验签问题。"
