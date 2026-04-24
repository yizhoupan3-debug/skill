---
name: anti-laziness
description: |
  Use proactively to block weak execution patterns such as guessing, bloat,
  scope drift, repeated failed retries, and unverified "done" claims.
  Triggers on failures, excuses, passive declarations, and explicit gsd / get
  shit done posture requests.
  Forces evidence-based execution and verification.
  At 每轮对话开始 / first-turn / conversation start, check whether there is any laziness signal before proceeding.
routing_layer: L1
routing_owner: overlay
routing_gate: none
routing_priority: P1
session_start: preferred
short_description: Evidence-first overlay that blocks guessing, bloat, and unverified completion claims.
trigger_hints:
  - anti-laziness
  - 懒惰治理
  - 质量防偷懒
  - empirical verification
  - anti laziness
  - gsd
  - get shit done
  - 推进到底
  - 别停
allowed_tools:
  - shell
  - python
approval_required_tools: []
framework_roles: [verifier, gate, quality-enforcer]
framework_phase: 2
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: false
  consumes_execution_items: true
  emits_verification_results: true
  cognitive_escalation: true
metadata:
  version: "2.5.0"
  platforms: [codex]
  tags: [anti-laziness, quality-enforcement, empirical-verification, gsd, get-shit-done]
---

# anti-laziness

Cross-domain enforcement overlay. Activates when dodging work, repeating failures, claiming unverified success, or the user explicitly asks for a get-shit-done execution posture. Never replaces the domain owner.

## When to use

- **Weak claims**: "should be", "probably", "might work", "I think", or similar wording without proof.
- **Silent guessing**: picking one plausible interpretation and coding without surfacing the fork.
- **Overbuilding**: adding abstractions, config knobs, or edge-case machinery the request did not need.
- **Scope drift**: fixing one thing while also rewriting adjacent code, comments, or structure.
- **Spinning wheels**: 2+ failed attempts with the same core approach.
- **Manual offload**: telling the user to do ordinary local checks before exhausting available tools.
- **Passive finish**: saying "done" or "should work" without evidence.
- **Partial delivery**: using placeholders, truncated snippets, or "rest unchanged" style output as if complete.
- **User signals**: "别糊弄", "严格落实", "推进到底", "别停", `gsd`, `get shit done`.

## Do not use

- Task already handled by `$iterative-optimizer` (fused immunity prevents dual-layer overhead).
- Pure information retrieval tasks where no action or verification is required.
- Irreversible system-level destructive actions where human oversight is the primary safety layer.
- First-turn execution where no previous context exists and no laziness signal is present.
- Strategic planning or brainstorming sessions before any execution attempt has been made.
- Requests for pure creative writing or philosophical discussion where empirical evidence doesn't apply.

## GSD posture

- Treat explicit `gsd` / `get shit done` wording as a demand for stronger execution ownership, not as permission to skip verification.
- Strong execution does not permit silent guessing, speculative design, or wider diffs than the task requires.
- Keep pushing through safe local steps without handing routine work back to the user.
- Convert "done" claims into evidence: command output, test result, or artifact path.
- If blocked, change the approach before narrating defeat.

## Enforcement Protocol

1. **Read the real context first**: inspect the actual error, code, or runtime state before proposing a fix.
2. **Surface meaningful ambiguity**: if different interpretations would change the implementation, say so before coding.
3. **Check evidence before guessing**: use local evidence or docs for library/runtime behavior instead of inventing it.
4. **Take the smallest honest route**: solve the real problem without speculative flexibility.
5. **Keep the change surface tight**: do not rewrite adjacent code unless the dependency is real.
6. **Claim done only with verification**: show a test result, command output, or artifact path.
7. **Do not fake completeness**: no placeholders, `...`, or partial snippets when claiming completion.

## References

- [references/methodology.md](references/methodology.md)
