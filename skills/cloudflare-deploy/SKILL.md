---
name: cloudflare-deploy
description: |
  Deploy, publish, migrate, and operate apps on Cloudflare Workers, Pages,
  Wrangler, routes, bindings, env vars, KV, R2, D1, Durable Objects, and related
  platform services. Use for 上 Cloudflare, 发 Workers/Pages, 配绑定和环境变量,
  edge-runtime migration, or production rollout on Cloudflare.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - Cloudflare Workers
  - Cloudflare Pages
  - Wrangler
  - bindings
  - 边缘部署
  - 发 Workers
  - Pages
  - 配绑定和环境变量
  - 迁移到边缘运行时
  - cloudflare
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags: [cloudflare, deploy, workers, pages, wrangler, edge]
risk: medium
source: local
---

# cloudflare-deploy

This skill owns Cloudflare-specific deployment and operations. It should keep
platform details in references and avoid becoming a general server-ops skill.

## When to Use

- The target platform is Cloudflare Workers, Pages, or a Cloudflare edge service.
- The task involves Wrangler, routes, bindings, secrets, env vars, previews, or rollbacks.
- The app needs migration from a generic server runtime to Cloudflare's runtime.
- The user asks for D1, R2, KV, Durable Objects, Pages, Workers, or Wrangler setup.

## Do Not Use

- Generic Linux/server deployment -> use `$linux-server-ops`.
- Vercel/Netlify/AWS/GCP deployment unless Cloudflare is still the target edge layer.
- General app debugging before Cloudflare-specific failure evidence exists.
- DNS/CDN/security policy work with no app deployment component.

## Safety Rules

- Ask before account-impacting actions such as publishing, deleting resources, or changing production routes.
- Prefer preview deployments before production changes.
- Do not expose tokens, account IDs, or secrets in logs or final replies.
- Keep local and production environment bindings distinct.
- Verify runtime compatibility before promising an edge migration.

## Workflow

1. Identify app type, Cloudflare target, repo commands, and deployment environment.
2. Inspect existing `wrangler.toml`, package scripts, bindings, and env usage.
3. Build locally or run the narrowest available validation.
4. Configure bindings/secrets/routes with least privilege and explicit environment names.
5. Deploy to preview first when possible; promote only when the user intends production.
6. Capture final URL, deployment ID, and verification status.

## Common Checks

- Workers runtime APIs vs Node-only APIs.
- Pages build output directory and functions routing.
- D1 migrations, KV/R2 binding names, Durable Object migrations.
- Secret availability per environment.
- Cache, route, and custom-domain behavior.

## References

- [references/workers/README.md](./references/workers/README.md)
- [references/pages/README.md](./references/pages/README.md)
- [references/wrangler/README.md](./references/wrangler/README.md)
- [references/d1/README.md](./references/d1/README.md)
- [references/r2/README.md](./references/r2/README.md)
- [references/durable-objects/README.md](./references/durable-objects/README.md)
