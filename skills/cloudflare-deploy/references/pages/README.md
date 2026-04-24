# Cloudflare Pages

JAMstack platform for full-stack apps on Cloudflare's global network.

## Key Features

- **Git-based deploys**: Auto-deploy from GitHub/GitLab
- **Preview deployments**: Unique URL per branch/PR
- **Pages Functions**: File-based serverless routing (Workers runtime)
- **Static + dynamic**: Smart asset caching + edge compute
- **Smart Placement**: Automatic function optimization based on traffic patterns
- **Framework optimized**: SvelteKit, Astro, Nuxt, Qwik, Solid Start

## Deployment Methods

### 1. Git Integration (Production)
Dashboard → Workers & Pages → Create → Connect to Git → Configure build

### 2. Direct Upload
```bash
npx wrangler pages deploy ./dist --project-name=my-project
npx wrangler pages deploy ./dist --project-name=my-project --branch=staging
```

### 3. C3 CLI
```bash
npm create cloudflare@latest my-app
# Select framework → auto-setup + deploy
```

## vs Workers

- **Pages**: Static sites, JAMstack, frameworks, git workflow, file-based routing
- **Workers**: Pure APIs, complex routing, WebSockets, scheduled tasks, email handlers
- **Combine**: Pages Functions use Workers runtime, can bind to Workers

## Quick Start

```bash
# Create
npm create cloudflare@latest

# Local dev
npx wrangler pages dev ./dist

# Deploy
npx wrangler pages deploy ./dist --project-name=my-project

# Types
npx wrangler types --path='./functions/types.d.ts'

# Secrets
echo "value" | npx wrangler pages secret put KEY --project-name=my-project

# Logs
npx wrangler pages deployment tail --project-name=my-project
```

## Resources

Use official Cloudflare Pages docs for current framework guides, limits, and function APIs.
