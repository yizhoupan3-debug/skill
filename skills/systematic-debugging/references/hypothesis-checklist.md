# Hypothesis Checklist for Systematic Debugging

> Use this when you have evidence but need to form and test a hypothesis before fixing.

## Pre-hypothesis gate

Before forming a hypothesis, confirm you have collected at least ONE of:
- [ ] Full error message / stack trace (from tool output, not paraphrased)
- [ ] Failing command + stdout/stderr captured verbatim
- [ ] A reproduction step that consistently triggers the failure
- [ ] A diff showing what changed before the failure started

If none of the above: **do not hypothesize — collect evidence first.**

## Hypothesis formation rules

1. **One at a time**: state a single causal chain (X causes Y because Z).
2. **Falsifiable**: the hypothesis must predict a specific observable outcome.
3. **Minimal**: prefer the simplest explanation consistent with the evidence.
4. **Labeled**: mark each hypothesis as `[INFERRED]` or `[OBSERVED]`.

## Hypothesis testing matrix

| Hypothesis Type | Fastest Falsification Method |
|---|---|
| Wrong environment variable | `env \| grep VAR_NAME` or `cat .env` |
| Dependency version mismatch | `npm ls pkg` / `pip show pkg` / `cargo tree \| grep pkg` |
| Race condition | Add `sleep` or reduce concurrency; see if failure changes |
| Wrong file/config path | `ls -la expected/path` |
| Off-by-one / logic bug | Add `console.log`/`print` at suspected line with boundary values |
| Network/port issue | `curl -v http://localhost:PORT` / `nc -zv host port` |
| Permission/auth issue | Run with elevated permissions or check token expiry |
| Cache/stale build | Clear cache (`rm -rf .next`, `cargo clean`, `npm run clean`) and rebuild |

## Failure routing after hypothesis confirmed

| Root cause type | Route to |
|---|---|
| Frontend runtime (blank screen, state desync) | `$frontend-debugging` |
| Backend crash/OOM/hang | `$backend-runtime-debugging` |
| API auth/transport/schema mismatch | `$api-integration-debugging` |
| Native app Web-Native boundary | `$native-app-debugging` |
| Build / dependency resolution | `$build-tooling` |
| Monitoring / alerting gap | `$observability` |
| Error propagation design | `$error-handling-patterns` |

## Anti-spinning anti-pattern

If you are on your 3rd hypothesis for the same symptom:
1. Stop and list what you have **ruled out** (not just ruled in).
2. Re-read the original error from scratch.
3. Check: did you verify reproduction after each fix attempt?
4. If no: do a **baseline reset** — revert all changes, reproduce cleanly, start over.
