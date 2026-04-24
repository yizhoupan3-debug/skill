# Cloudflare Durable Objects

Expert guidance for building stateful applications with Cloudflare Durable Objects.

## Use Official Docs For Details

1. **First time?** Read this overview + Quick Start
2. For setup, patterns, debugging, APIs, storage, migrations, and limits, use official Cloudflare docs.

## Overview

Durable Objects combine compute with storage in globally-unique, strongly-consistent packages:
- **Globally unique instances**: Each DO has unique ID for multi-client coordination
- **Co-located storage**: Fast, strongly-consistent storage with compute
- **Automatic placement**: Objects spawn near first request location
- **Stateful serverless**: In-memory state + persistent storage
- **Single-threaded**: Serial request processing (no race conditions)

## Rules of Durable Objects

Critical rules preventing most production issues:

1. **One alarm per DO** - Schedule multiple events via queue pattern
2. **~1K req/s per DO max** - Shard for higher throughput
3. **Constructor runs every wake** - Keep initialization light; use lazy loading
4. **Hibernation clears memory** - In-memory state lost; persist critical data
5. **Use `ctx.waitUntil()` for cleanup** - Ensures completion after response sent
6. **No setTimeout for persistence** - Use `setAlarm()` for reliable scheduling

## Core Concepts

### Class Structure
All DOs extend `DurableObject` base class with constructor receiving `DurableObjectState` (storage, WebSockets, alarms) and `Env` (bindings).

### Lifecycle States

```
[Not Created] → [Active] ⇄ [Hibernated] → [Evicted]
                   ↓
              [Destroyed]
```

- **Not Created**: DO ID exists but instance never spawned
- **Active**: Processing requests, in-memory state valid, billed per GB-hour
- **Hibernated**: WebSocket connections open but zero compute, zero cost
- **Evicted**: Removed from memory; next request triggers cold start
- **Destroyed**: Data deleted via migration or manual deletion

### Accessing from Workers
Workers use bindings to get stubs, then call RPC methods directly (recommended) or use fetch handler (legacy).

**RPC vs fetch() decision:**
```
├─ New project + compat ≥2024-04-03 → RPC (type-safe, simpler)
├─ Need HTTP semantics (headers, status) → fetch()
├─ Proxying requests to DO → fetch()
└─ Legacy compatibility → fetch()
```

Use official Cloudflare docs for current examples.

### ID Generation
- `idFromName()`: Deterministic, named coordination (rate limiting, locks)
- `newUniqueId()`: Random IDs for sharding high-throughput workloads
- `idFromString()`: Derive from existing IDs
- Jurisdiction option: Data locality compliance

### Storage Options

**Which storage API?**
```
├─ Structured data, relations, transactions → SQLite (recommended)
├─ Simple KV on SQLite DO → ctx.storage.kv (sync KV)
└─ Legacy KV-only DO → ctx.storage (async KV)
```

- **SQLite** (recommended): Structured data, transactions, 10GB/DO
- **Synchronous KV API**: Simple key-value on SQLite objects
- **Asynchronous KV API**: Legacy/advanced use cases

Use official Cloudflare docs for current examples.

### Special Features
- **Alarms**: Schedule future execution per-DO (1 per DO - use queue pattern for multiple)
- **WebSocket Hibernation**: Zero-cost idle connections (memory cleared on hibernation)
- **Point-in-Time Recovery**: Restore to any point in 30 days (SQLite only)

## Quick Start

```typescript
import { DurableObject } from "cloudflare:workers";

export class Counter extends DurableObject<Env> {
  async increment(): Promise<number> {
    const result = this.ctx.storage.sql.exec(
      `INSERT INTO counters (id, value) VALUES (1, 1)
       ON CONFLICT(id) DO UPDATE SET value = value + 1
       RETURNING value`
    ).one();
    return result.value;
  }
}

// Worker access
export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const id = env.COUNTER.idFromName("global");
    const stub = env.COUNTER.get(id);
    const count = await stub.increment();
    return new Response(`Count: ${count}`);
  }
};
```

## Decision Trees

### What do you need?

```
├─ Coordinate requests (rate limit, lock, session)
│   → idFromName(identifier) → official Cloudflare docs
│
├─ High throughput (>1K req/s)
│   → Sharding with newUniqueId() or hash → official Cloudflare docs
│
├─ Real-time updates (WebSocket, chat, collab)
│   → WebSocket hibernation + room pattern → official Cloudflare docs
│
├─ Background work (cleanup, notifications, scheduled tasks)
│   → Alarms + queue pattern (1 alarm/DO) → official Cloudflare docs
│
└─ User sessions with expiration
    → Session pattern + alarm cleanup → official Cloudflare docs
```

### Which access pattern?

```
├─ New project + typed methods → RPC (compat ≥2024-04-03)
├─ Need HTTP semantics → fetch()
├─ Proxying to DO → fetch()
└─ Legacy compat → fetch()
```

Use official Cloudflare docs for current examples.

### Which storage?

```
├─ Structured data, SQL queries, transactions → SQLite (recommended)
├─ Simple KV on SQLite DO → ctx.storage.kv (sync API)
└─ Legacy KV-only DO → ctx.storage (async API)
```

Use official Cloudflare docs for current examples.

## Essential Commands

```bash
npx wrangler dev              # Local dev with DOs
npx wrangler dev --remote     # Test against prod DOs
npx wrangler deploy           # Deploy + auto-apply migrations
```

## Resources

Use official Cloudflare Durable Objects docs for current APIs, storage behavior, limits, migrations, and examples.
