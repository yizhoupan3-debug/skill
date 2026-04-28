# Checklist Fixer — Verification Protocol

This reference defines the mandatory per-item and integration verification
procedures. Every fix item MUST pass verification before being marked `✅`.

## Per-item verification

### Step 1 — Choose the narrowest verifier

| Fix type | Recommended verifier |
|---|---|
| Source code change | Run affected unit / integration tests; or lint on the file |
| Config / env change | App start-up, health-check endpoint, or `env` diff |
| Data migration / SQL | Query the affected rows; diff before/after |
| Build / dependency change | Full build; lock-file diff |
| Documentation / copy | Render (markdown preview, `mdbook build`); spellcheck |
| Refactor (behavior-preserving) | Full test suite on affected modules |
| Security / auth fix | Run relevant security test or curl the protected route |

### Step 2 — Capture evidence

Always capture one of:
- `stdout` / `stderr` from the verifier command
- Test result output (`PASS N tests`, `0 failures`)
- Build log (last 20 lines is enough)
- File diff showing the targeted change is present

Paste or summarize this output inline with the fix report. **Never skip.**

### Step 3 — Deliver binary verdict

```
✅ PASS — <verifier> confirms fix for item #N
❌ FAIL — <verifier output summary> — action: <revert/isolate/investigate>
```

### Step 4 — On FAIL

1. Revert or isolate the broken change (keep codebase green).
2. Mark item failed with the failure reason.
3. Check if next item is independent; if yes, continue.
4. If the failure is unexpected, route that item to `$systematic-debugging`.
5. **Do NOT continue on a P0 failure without explicit user approval.**

---

## Integration check (after all items)

Run once after completing the full queue (or a selected batch):

```
[ ] Full test suite (or affected test modules)
[ ] Build / compilation
[ ] Lint / type-check
[ ] Smoke-test the feature/flow touched by fixes (if applicable)
```

Report aggregate result in the Completion Summary:

```markdown
### Integration Check
- Tests:  PASS (N/N) / FAIL (details)
- Build:  PASS / FAIL
- Lint:   PASS / N warnings (list)
- Smoke:  PASS / SKIP (reason)
```

---

## Anti-laziness evidence standards

These are hard minimums — anything less invalidates the `✅`:

| Claim | Required evidence |
|---|---|
| "tests pass" | Paste last line of test runner output |
| "build succeeds" | Paste last 5 lines of build log |
| "lint clean" | Paste lint exit code + 0-issue confirmation |
| "fix applied" | Show the actual diff or file excerpt |
| "works now" | Not accepted without one of the above |

---

## False-convergence scan (end of session)

After marking all items done, run a broad scan to catch regressions:

```bash
# Example: find remaining TODOs or known-bad patterns
grep -rn "TODO\|FIXME\|HACK" <changed files>

# Run the full test suite one more time
<test command>

# Verify no file outside scope was accidentally modified
git diff --name-only
```

Report any findings. If any are non-trivial, add them to the residual-risk
section of the Completion Summary.
