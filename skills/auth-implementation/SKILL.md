---
name: auth-implementation
description: |
  Produce server-enforced auth flows with clean separation between authentication, session lifecycle, and authorization.
  Covers login/signup/logout, token expiry, refresh, revocation, JWT, sessions, OAuth, RBAC/ABAC, route guards, middleware, cookies, CSRF, and webhook callback implementation. Use when the user asks for auth, 登录, 注册, 鉴权, 权限控制, token refresh, JWT vs session, 做登录, 加权限控制, route guard, or implementing provider callbacks.
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - auth
    - authorization
    - jwt
    - session
    - oauth
risk: high
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - auth
  - 登录
  - 注册
  - 鉴权
  - 权限控制
  - token refresh
  - JWT vs session
  - 做登录
  - 加权限控制
  - route guard
  - webhook callback
---

# auth-implementation

This skill owns application-level authentication and authorization implementation so auth work does not get scattered across generic backend, frontend, or security skills.

## When to use

- The user wants login, signup, logout, session handling, JWT flows, refresh tokens, or OAuth integration
- The task involves permissions, RBAC/ABAC, route guards, middleware auth, or identity propagation across services
- The task involves cookies, session stores, token rotation, protected routes, or auth-aware API behavior
- The user asks how to choose between JWT and session auth for an app
- Best for requests like:
  - "给这个项目做登录和权限控制"
  - "JWT 还是 session 怎么选"
  - "帮我加 route guard / middleware auth"
  - "实现 refresh token 和权限校验"

## Do not use

- The main task is security auditing of existing auth code for exploitable flaws → use `$security-audit`
- The task is exploit-focused review of an existing webhook handler → use `$security-audit`
- The task is a high-level threat model rather than auth implementation → use `$security-threat-model`
- The task is generic backend structure with little auth-specific logic → use `$node-backend`

## Task ownership and boundaries

This skill owns:
- auth flow design and implementation
- authorization checks and permission models
- session/JWT/OAuth integration at the application layer
- route guards, middleware, and identity propagation
- token/cookie/session lifecycle decisions tied to app behavior
- **Dual-Dimension Audit (Pre: Flow/Logic, Post: Token/Session Results)** → `$execution-audit` [Overlay]

This skill does not own:
- exploit-focused security auditing by itself
- exploit-focused provider webhook review
- full threat modeling
- generic backend refactors with no auth focus

If the task shifts to adjacent skill territory, route to:
- `$security-audit`
- `$security-threat-model`
- `$node-backend`
- `$nextjs`

## Required workflow

1. Confirm the task shape:
   - object: app auth flow, protected route, permission model, token/session layer
   - action: design, implement, refactor, harden, review
   - constraints: framework, storage model, cookie policy, provider, trust boundary
   - deliverable: code, auth plan, migration path, or review guidance
2. Identify actors, trust boundaries, and protected resources first.
3. Separate authentication, session/token lifecycle, and authorization clearly.
4. Make auth decisions explicit at request and resource boundaries.
5. Validate happy-path and failure-path behavior.

## Core workflow

### 1. Intake
- Determine whether the app uses sessions, JWT, OAuth, API keys, or a hybrid model.
- Clarify who the actors are and what permissions/resources need protection.
- Inspect existing middleware, cookie policy, and identity sources before adding a new auth path.

### 2. Execution
- Choose auth/session strategy based on app topology rather than hype.
- Keep identity extraction and permission checks centralized.
- Make token/session expiry, refresh, revocation, and logout behavior explicit.
- Treat cookie, CSRF, same-site, and storage decisions as first-class design choices.
- Keep authorization decisions close to resource access, not only UI guards.

### 3. Validation / recheck
- Verify login, logout, refresh, unauthorized, expired, and forbidden paths.
- Re-check that permissions are enforced server-side where needed.
- Call out remaining security-sensitive assumptions explicitly.

## Output defaults

Default output should contain:
- auth model and trust assumptions
- implementation choices and protected boundaries
- validation paths and remaining risks

Recommended structure:

````markdown
## Auth Summary
- Auth model: ...
- Protected resources: ...

## Implementation / Design
- Authentication: ...
- Authorization: ...
- Session / token lifecycle: ...

## Validation / Risks
- Tested paths: ...
- Assumptions: ...
- Remaining risk: ...
````

## Hard constraints

- Do not leave authorization implied by frontend state alone when server enforcement is required.
- Do not add JWT/session flows without specifying expiry, refresh, and logout behavior.
- Do not hand-wave cookie storage, CSRF, or token persistence decisions.
- Do not mix authentication and authorization into one vague middleware concept.
- If a security-sensitive assumption remains unverified, say so explicitly.
- **Superior Quality Audit**: For auth-critical flows, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## References

- [references/webhook-callbacks.md](references/webhook-callbacks.md)

## Trigger examples

- "Use $auth-implementation to add login, sessions, and RBAC."
- "帮我实现登录、权限控制和 route guard。"
- "这个项目应该用 JWT 还是 session，顺便落代码。"
- "强制进行 Auth 深度审计 / 检查登录逻辑与 Token 生命周期结果。"
- "Use $execution-audit to audit this auth implementation for zero-token-leak idealism."
