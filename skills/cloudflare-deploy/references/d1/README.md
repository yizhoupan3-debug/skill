# Cloudflare D1 Database

Expert guidance for Cloudflare D1, a serverless SQLite database designed for horizontal scale-out across multiple databases.

## Overview

D1 is Cloudflare's managed, serverless database with:
- SQLite SQL semantics and compatibility
- Built-in disaster recovery via Time Travel (30-day point-in-time recovery)
- Horizontal scale-out architecture (10 GB per database)
- Worker and HTTP API access
- Pricing based on query and storage costs only

**Architecture Philosophy**: D1 is optimized for per-user, per-tenant, or per-entity database patterns rather than single large databases.

## Quick Start

```bash
# Create database
wrangler d1 create <database-name>

# Execute migration
wrangler d1 migrations apply <db-name> --remote

# Local development
wrangler dev
```

## Core Query Methods

```typescript
// .all() - Returns all rows; .first() - First row or null; .first(col) - Single column value
// .run() - INSERT/UPDATE/DELETE; .raw() - Array of arrays (efficient)
const { results, success, meta } = await env.DB.prepare('SELECT * FROM users WHERE active = ?').bind(true).all();
const user = await env.DB.prepare('SELECT * FROM users WHERE id = ?').bind(userId).first();
```

## Batch Operations

```typescript
// Multiple queries in single round trip (atomic transaction)
const results = await env.DB.batch([
  env.DB.prepare('SELECT * FROM users WHERE id = ?').bind(1),
  env.DB.prepare('SELECT * FROM posts WHERE author_id = ?').bind(1),
  env.DB.prepare('UPDATE users SET last_access = ? WHERE id = ?').bind(Date.now(), 1)
]);
```

## Sessions API (Paid Plans)

```typescript
// Create long-running session for analytics/migrations (up to 15 minutes)
const session = env.DB.withSession();
try {
  await session.prepare('CREATE INDEX idx_heavy ON large_table(column)').run();
  await session.prepare('ANALYZE').run();
} finally {
  session.close(); // Always close to release resources
}
```

## Read Replication (Paid Plans)

```typescript
// Read from nearest replica for lower latency (automatic failover)
const user = await env.DB_REPLICA.prepare('SELECT * FROM users WHERE id = ?').bind(userId).first();

// Writes always go to primary
await env.DB.prepare('UPDATE users SET last_login = ? WHERE id = ?').bind(Date.now(), userId).run();
```

## Platform Limits

| Limit | Free Tier | Paid Plans |
|-------|-----------|------------|
| Database size | 500 MB | 10 GB per database |
| Row size | 1 MB max | 1 MB max |
| Query timeout | 30 seconds | 30 seconds |
| Batch size | 1,000 statements | 10,000 statements |
| Time Travel retention | 7 days | 30 days |
| Read replicas | Not available | Yes (paid add-on) |

**Pricing**: $5/month per database beyond free tier + $0.001 per 1K reads + $1 per 1M writes + $0.75/GB storage/month

## CLI Commands

```bash
# Database management
wrangler d1 create <db-name>
wrangler d1 list
wrangler d1 delete <db-name>

# Migrations
wrangler d1 migrations create <db-name> <migration-name>    # Create new migration file
wrangler d1 migrations apply <db-name> --remote             # Apply pending migrations
wrangler d1 migrations apply <db-name> --local              # Apply locally
wrangler d1 migrations list <db-name> --remote              # Show applied migrations

# Direct SQL execution
wrangler d1 execute <db-name> --remote --command="SELECT * FROM users"
wrangler d1 execute <db-name> --local --file=./schema.sql

# Backups & Import/Export
wrangler d1 export <db-name> --remote --output=./backup.sql  # Full export with schema
wrangler d1 export <db-name> --remote --no-schema --output=./data.sql  # Data only
wrangler d1 time-travel restore <db-name> --timestamp="2024-01-15T14:30:00Z"  # Point-in-time recovery

# Development
wrangler dev --persist-to=./.wrangler/state
```

## Use Official Docs For Details

This is a quick card only. Use official Cloudflare docs for current setup, API details, limits, pricing, and troubleshooting.
