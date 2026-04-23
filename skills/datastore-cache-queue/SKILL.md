---
name: datastore-cache-queue
description: |
  Diagnose and fix correctness issues across stores, caches, queues, and ORM-backed workers.
  Use for source-of-truth bugs, cache invalidation, idempotency, retries,
  migrations, and worker consistency failures.
metadata:
  version: "1.0.1"
  platforms: [codex]
  tags:
    - datastore
    - redis
    - cache
    - queue
    - worker
    - migration
    - transaction
risk: medium
source: local
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - datastore
  - redis
  - cache
  - queue
  - worker
  - migration
---

# datastore-cache-queue

This skill owns the application's data-runtime layer when the challenge is how services interact with databases, caches, queues, workers, and consistency boundaries in production.

## When to use

- The user wants to design or debug Redis, caching, queues, workers, migrations, transactions, or connection pooling
- The task involves ORM behavior, job retries, idempotency, cache invalidation, or read/write consistency
- The user wants data-layer architecture between application code and storage/runtime systems, not just one SQL statement
- The task is about balancing correctness, throughput, retry safety, and operational simplicity
- Best for requests like:
  - "帮我设计 Redis 缓存和失效策略"
  - "为什么这个 queue/worker 会重复消费或丢任务"
  - "看一下 migration、transaction、连接池、ORM 行为有没有坑"
  - "这个数据层要怎么做一致性和重试"

## Do not use

- The task is a single complex SQL query or index-tuning exercise → use `$sql-pro`
- The task is framework/backend route implementation without storage-runtime design depth → use `$node-backend` or `$python-pro`
- The task is generic high-level architecture review without data-runtime specifics → use `$architect-review`
- The task is pure API integration rather than storage/cache/queue behavior → use `$api-integration-debugging`

## Task ownership and boundaries

This skill owns:
- cache, queue, worker, migration, and pooling strategy
- data consistency, retry safety, idempotency, and transaction boundaries
- ORM/runtime behavior around persistence and concurrency
- Redis and application-side caching patterns
- background-job reliability and storage-interaction design

This skill does not own:
- isolated SQL query authoring as the main task
- general backend endpoint implementation by itself
- whole-system architecture review without data-runtime focus
- pure API protocol debugging
- **Dual-Dimension Audit (Pre: Logic/Patterns, Post: Consistency/State Results)** → `$execution-audit` [Overlay]

If the task shifts to adjacent skill territory, route to:
- `$sql-pro`
- `$node-backend`
- `$python-pro`
- `$architect-review`
- `$api-integration-debugging`

## Required workflow

1. Confirm the task shape:
   - object: datastore, cache, queue, worker, ORM layer, migration path
   - action: design, debug, review, stabilize, optimize
   - constraints: consistency, latency, throughput, failure mode, ops simplicity
   - deliverable: architecture guidance, code/config changes, findings, or migration plan
2. Identify the true correctness boundary before optimizing throughput.
3. Separate persistence, caching, async delivery, and retry semantics.
4. Make failure and recovery paths explicit.
5. Validate both steady-state and failure-mode behavior.

## Core workflow

### 1. Intake
- Identify the storage systems, cache layer, queue/worker stack, ORM, and deployment context.
- Inspect current consistency expectations, retry behavior, invalidation rules, and schema/migration state.
- Determine whether the issue is stale reads, duplicate work, lost updates, deadlocks, pool exhaustion, migration risk, or hot-key/load shape.

### 2. Execution
- Define source of truth, read/write path, and cache/queue ownership clearly.
- Check transaction scope, idempotency keys, deduplication, retry/backoff policy, and poison-message handling.
- Review connection pooling, timeout, locking, concurrency, and migration ordering.
- For caching, verify invalidation strategy, TTL fit, stampede protection, and stale-data tolerance.
- For ORM/runtime behavior, verify query shape, lazy/eager loading, transaction participation, and hidden consistency assumptions.

### 3. Validation / recheck
- Recheck normal path plus at least one failure/retry path.
- Verify whether duplicate delivery, partial failure, or stale cache cases are handled explicitly.
- Call out remaining operational risks such as migration rollback limits, hot keys, or pool pressure.
- If consistency is only eventual, state that clearly.

## Output defaults

Default output should contain:
- data-runtime boundary summary
- correctness/reliability findings or changes
- failure-mode validation and remaining risks

Recommended structure:

````markdown
## Data Runtime Summary
- Systems involved: ...
- Main risk / bottleneck: ...

## Findings / Design Changes
- ...

## Failure Modes / Validation
- Verified: ...
- Remaining risks: ...
````

## Hard constraints

- Do not optimize cache or queue throughput before stating correctness guarantees.
- Do not assume retries are safe without idempotency analysis.
- Do not recommend cache invalidation patterns without naming the source of truth.
- Always mention failure-mode behavior, not just happy path.
- If transactionality or consistency is inferred rather than verified, label it explicitly.
- **Superior Quality Audit**: For system-critical data flows, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## Trigger examples

- "Use $datastore-cache-queue to review this Redis/cache/queue design."
- "帮我排查 worker 重试、重复消费、缓存失效和事务边界问题。"
- "这个 ORM + migration + 连接池 + idempotency 方案稳不稳？"
- "强制进行数据层深度审计 / 检查缓存一致性与队列状态结果。"
- "Use $execution-audit to audit this datastore design for consistency-state idealism."
