---
name: security-audit
description: |
  Audit implementation-level security defects in auth, injection, SSRF, CSRF, secrets, sessions, and hardening gaps.
  Use when reviewing backend or frontend code for exploitable issues, or when the user asks to 代码安全审计, 检查鉴权/授权漏洞, 查注入风险, 检查 SSRF, 检查敏感信息泄露, review auth implementation security, or audit input validation and file upload security. Do not use for system-level threat modeling.
routing_layer: L4
routing_owner: overlay
routing_gate: none
session_start: n/a
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags:
    - security
    - auth
    - injection
    - ssrf
    - input-validation
    - csrf
    - secret-exposure
framework_roles:
  - detector
  - planner
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: medium
source: local
---
- **Dual-Dimension Audit (Pre: Vuln-Surface/Logic, Post: Penetration-Scan/OWASP-Compliance Results)** → `$execution-audit-codex` [Overlay]
# security-audit

This skill owns implementation-level security auditing: finding real exploitable weaknesses in code, not just architectural threat modeling.

## Framework compatibility

This skill's findings should be **mappable to the shared finding-driven
framework** without weakening security triage. Keep exploitability and fix
recommendations explicit, and preserve security-native severity or confidence
labels when the result is handed to another skill.

Distinction from `security-threat-model`:
- `security-threat-model` → system-level attack paths
- `security-audit` → code implementation and defense gaps

## When to use

- The user wants a code security audit
- The task involves checking auth/authz implementation
- The task involves input validation, injection, file handling, redirect, or SSRF risks
- The user wants application hardening
- The user wants dependency or secret exposure review
- Best for requests like:
  - "帮我做代码安全审计"
  - "检查一下这个 API 的鉴权实现"
  - "看看有没有注入风险或 SSRF"
  - "检查 secret 泄露问题"

## Do not use

- The task is purely architectural threat modeling → use `$security-threat-model`
- The task is a normal bug fix code review without security focus → use `$code-review`
- The task is non-security code style review → use `$coding-standards`

## Task ownership and boundaries

This skill owns:
- access control and authorization review
- input handling and injection detection
- secret and config exposure detection
- dependency and supply chain risk
- runtime hardening checks

This skill does not own:
- system-level threat modeling
- generic code style review
- performance or architecture review

If the task shifts to adjacent skill territory, route to:
- `$security-threat-model` for architecture-level attack paths
- `$architect-review` for non-security structural decisions
- `$coding-standards` for style and convention review

## Required workflow

1. Confirm the task shape:
   - object: API, auth flow, input handler, file upload, redirect, dependency set
   - action: audit, review, harden, check
   - constraints: stack, framework, deployment context
   - deliverable: finding list, severity ranking, fix recommendations
2. Start with access control review before other categories.
3. Prioritize exploitable findings over theoretical risks.
4. Tie every finding to a specific code location and fix.

## Core workflow

### 1. Access Control
- Verify server-side auth on every sensitive entry point.
- Check for IDOR / horizontal privilege escalation.
- Verify admin interfaces are isolated from normal endpoints.
- Check that frontend hiding is not mistaken for authorization.

### 2. Input Handling
- Review schema validation on all inputs.
- Check file uploads for type, size, and path restrictions.
- Review URL/redirect handling for open redirect.
- Check for SQL/NoSQL/template/HTML injection via concatenation.
- Review shell command construction for injection.

### 3. Secret and Config Exposure
- Check for hardcoded keys and credentials.
- Verify tokens are not written to logs.
- Check that debug info does not leak internal structure.
- Verify environment variables are not leaked to client bundles.

### 4. Dependency and Supply Chain
- Check for known vulnerable dependencies.
- Verify lockfile exists and is committed.
- Review install scripts and CI for supply chain risks.

### 5. Runtime Hardening
- Check rate limiting on sensitive endpoints.
- Review security headers (CSP, HSTS, X-Frame-Options).
- Check CORS configuration.
- Review error message exposure.
- Check deserialization and external request patterns.

## Output defaults

Default output should contain:
- findings sorted by severity
- code location and impact for each finding
- concrete fix recommendations

For framework-compatible output, include stable `finding_id` values and a
`verification_method` such as targeted retest, manual exploit check, or
regression audit.

Recommended structure:

````markdown
## Security Audit Summary
- Scope: ...
- Critical findings: N
- High findings: N

## Findings

### [Severity] Finding Title
- **Category**: ...
- **Location**: ...
- **Impact**: ...
- **Fix**: ...

## Priority Fix Plan
- ...
````

## Hard constraints

- Do not just list OWASP category names; tie findings to specific code paths.
- Do not mark uncertain issues as confirmed; label as "suspected" or "needs verification".
- If a concrete fix point and validation method are available, always provide them.
- Prioritize findings by actual exploitability, not theoretical severity alone.
- Do not skip access control review in favor of lower-priority categories.
- **Superior Quality Audit**: For critical security boundaries, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples

- "Use $security-audit to review this API for auth and injection issues."
- "帮我做代码安全审计，重点看鉴权和输入校验。"
- "检查这个项目有没有 SSRF 或 secret 泄露。"
- "强制进行安全深度审计 / 检查漏洞面与 OWASP 合规结果。"
- "Use $execution-audit-codex to audit this security implementation for penetration-scan idealism."
- "看下这个文件上传是否安全。"
