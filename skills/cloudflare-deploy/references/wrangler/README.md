# Cloudflare Wrangler

Official CLI for Cloudflare Workers - develop, manage, and deploy Workers from the command line.

## What is Wrangler?

Wrangler is the Cloudflare Developer Platform CLI that allows you to:
- Create, develop, and deploy Workers
- Manage bindings (KV, D1, R2, Durable Objects, etc.)
- Configure routing and environments
- Run local development servers
- Execute migrations and manage resources
- Perform integration testing

## Installation

```bash
npm install wrangler --save-dev
# or globally
npm install -g wrangler
```

Run commands: `npx wrangler <command>` (or `pnpm`/`yarn wrangler`)

## Use Official Docs For Details

This is a quick command card only. Use official Wrangler docs for current flags, config schema, APIs, and troubleshooting.

## Essential Commands

### Project & Development
```bash
wrangler init [name]              # Create new project
wrangler dev                      # Local dev server (fast, simulated)
wrangler dev --remote             # Dev with remote resources (production-like)
wrangler deploy                   # Deploy to production
wrangler deploy --env staging     # Deploy to environment
wrangler versions list            # List versions
wrangler rollback [id]            # Rollback deployment
wrangler login                    # OAuth login
wrangler whoami                   # Check auth status
```

## Resource Management

### KV
```bash
wrangler kv namespace create NAME
wrangler kv key put "key" "value" --namespace-id=<id>
wrangler kv key get "key" --namespace-id=<id>
```

### D1
```bash
wrangler d1 create NAME
wrangler d1 execute NAME --command "SQL"
wrangler d1 migrations create NAME "description"
wrangler d1 migrations apply NAME
```

### R2
```bash
wrangler r2 bucket create NAME
wrangler r2 object put BUCKET/key --file path
wrangler r2 object get BUCKET/key
```

### Other Resources
```bash
wrangler queues create NAME
wrangler vectorize create NAME --dimensions N --metric cosine
wrangler hyperdrive create NAME --connection-string "..."
wrangler workflows create NAME
wrangler constellation create NAME
wrangler pages project create NAME
wrangler pages deployment create --project NAME --branch main
```

### Secrets
```bash
wrangler secret put NAME          # Set Worker secret
wrangler secret list              # List Worker secrets
wrangler secret delete NAME       # Delete Worker secret
wrangler secret bulk FILE.json    # Bulk upload from JSON

# Secrets Store (centralized, reusable across Workers)
wrangler secret-store:secret put STORE_NAME SECRET_NAME
wrangler secret-store:secret list STORE_NAME
```

### Monitoring
```bash
wrangler tail                     # Real-time logs
wrangler tail --env production    # Tail specific env
wrangler tail --status error      # Filter by status
```

## Local Scope

This is a quick card only. Use official Cloudflare docs for current setup, API details, limits, pricing, and troubleshooting.
