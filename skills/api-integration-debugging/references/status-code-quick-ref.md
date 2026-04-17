# API Status Code & Protocol Error Quick Reference

> Use this to quickly identify which failure layer is responsible and what to check.

## HTTP Status Codes

| Code | Name | Likely Cause | First Check |
|---|---|---|---|
| 400 | Bad Request | Wrong body shape, missing field, wrong content-type | Compare request body vs API docs schema |
| 401 | Unauthorized | Missing/expired token, wrong auth header format | Check `Authorization` header value and token TTL |
| 403 | Forbidden | Valid auth but insufficient permissions/scope | Check token scopes, API key permissions, IP allowlist |
| 404 | Not Found | Wrong URL path, resource not created yet, wrong env | Double-check base URL and route prefix per env |
| 405 | Method Not Allowed | Wrong HTTP method (PUT vs PATCH, etc.) | Check docs for correct method |
| 409 | Conflict | Duplicate resource, optimistic lock violation | Check idempotency key, retry logic |
| 422 | Unprocessable Entity | Passes schema but fails business validation | Read error body for field-specific messages |
| 429 | Too Many Requests | Rate limit exceeded | Check `Retry-After` or `X-RateLimit-*` headers |
| 500 | Internal Server Error | Server-side bug (not client fault) | Check server logs; simplify request to minimum payload |
| 502 | Bad Gateway | Upstream service down, proxy misconfiguration | Check if API base URL is reachable (`curl -I`) |
| 503 | Service Unavailable | Overload or maintenance | Check status page; add retry with backoff |
| 504 | Gateway Timeout | Slow upstream, large payload, cold start | Reduce payload; check timeout settings |

## CORS Errors

| Error Pattern | Cause | Fix |
|---|---|---|
| No `Access-Control-Allow-Origin` | Server not sending CORS headers | Add CORS middleware on server |
| `Access-Control-Allow-Origin: *` but cookies needed | Wildcard origin incompatible with `withCredentials` | Use specific origin on server |
| Preflight (OPTIONS) returns 405 | Backend route handler does not handle OPTIONS | Add OPTIONS handler |
| Blocked by browser but `curl` succeeds | CORS is browser-only; server-side calls are unaffected | CORS is client-browser concern only |

## WebSocket Failure Patterns

| Symptom | Layer | Diagnosis Command |
|---|---|---|
| Connection immediately closes (code 1006) | Transport | `websocat -v ws://...` to see raw handshake |
| 101 upgrade succeeds but no messages arrive | Application | Log first message on both ends |
| Auth failure on connect | Auth | Check if token is sent in query param or `Sec-WebSocket-Protocol` |
| Works in dev, fails in prod | Proxy/TLS | Check nginx/Caddy WebSocket proxy headers (`Upgrade`, `Connection`) |

## OAuth / Token Debugging Checklist

1. Is the token expired? Decode JWT (`jwt.io`) and check `exp` field.
2. Is the wrong grant type being used (`password` vs `client_credentials` vs `authorization_code`)?
3. Is the `redirect_uri` an exact match (trailing slash matters)?
4. Is the `scope` correct for this endpoint?
5. Is the token being sent as `Bearer <token>` (not `Token` or bare value)?
6. Is PKCE required but not being sent?

## GraphQL Error Patterns

| Error | Meaning | Fix |
|---|---|---|
| `field X does not exist` | Schema mismatch | Update query to match current schema via introspection |
| `Unauthorized` in `errors` with 200 status | Auth OK at transport, fails at resolver | Check resolver auth logic or API key scopes |
| Partial data + errors array | Some resolvers failed | Check each error's `path` to pinpoint failing field |
| `Variable $X got invalid value` | Wrong variable type | Match variable type to schema type |
