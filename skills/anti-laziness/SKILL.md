---
name: anti-laziness
description: |
  Use proactively to detect and counter AI laziness patterns (spinning wheels, manual work offload, idling, truncation).
  Triggers on failures, excuses, or passive declarations. Forces empirical execution and evidence-based verification.
  At 每轮对话开始 / first-turn / conversation start, check whether there is any laziness signal before proceeding.
routing_layer: L1
routing_owner: overlay
routing_gate: none
routing_priority: P1
session_start: preferred
short_description: Fused overlay to detect/counter cognitive laziness and force empirical evidence.
trigger_hints:
  - anti-laziness
  - 懒惰治理
  - 执行力审计
  - 质量防偷懒
  - empirical verification
  - pua
  - anti laziness
  - quality enforcement
  - mental rigor
  - token optimized
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
  version: "2.2.0"
  platforms: [codex, antigravity]
  tags: [pua, anti-laziness, quality-enforcement, mental-rigor, token-optimized]
---

# anti-laziness

Cross-domain enforcement overlay. Activates when dodging work, repeating failures, or claiming unverified success. Never replaces the domain owner.

## When to use (PUA Triggers)

- **Lazy Phrasing**: Use of "should be", "probably", "may", "might work", "I think", "is likely".
- **Complexity Dodging**: Simplifying a user's multi-step request into a single "generic" step.
- **Spinning Wheels**: 2+ failed attempts with identical core approach.
- **Blame Shifting**: Suggesting manual checks before exhausting local diagnostic tools.
- **Passive Finish**: Claims of "It should work" without providing `stdout/stderr` evidence.
- **Code Truncation**: Using `...`, `// remains unchanged`, or partial snippets.
- **Doc Avoidance**: Guessing API usage instead of using `context7`.
- **User Complaints**: "别糊弄", "别装死", "防偷懒", "严格落实", "不许偷工减料".

## Do not use

- Task already handled by `$iterative-optimizer` (fused immunity prevents dual-layer overhead).
- Pure information retrieval tasks where no action or verification is required.
- Irreversible system-level destructive actions where human oversight is the primary safety layer.
- First-turn execution where no previous context exists and no laziness signal is present.
- Strategic planning or brainstorming sessions before any execution attempt has been made.
- Requests for pure creative writing or philosophical discussion where empirical evidence doesn't apply.

## PUA Protocol: Thorough & Token-Efficient

1. **Stop & Observe**: Read error word-by-word. MANDATORY `cat` of 50 lines context.
2. **Search First**: MUST use `context7` for any library-related error. No guessing.
3. **Evidence or Silence**: NO declaration of success without `stdout/stderr`.
4. **Zero Truncation**: Write FULL files or use tight `multi_replace_file_content`.
5. **False Convergence**: `grep` repository for similar patterns after ANY fix.

## Token-Saving Cheat Sheet (Context density: High)
| Pattern | Signal | Mandatory Penalty Action |
| :--- | :--- | :--- |
| **Wheels** | Repeated `ls`/`cat` | Pivot to callers/callees or orthogonal layer. |
| **Shift** | "Check your port" | Run `netstat`/`lsof` FIRST. |
| **Wait** | "It works now" | Run verification script + show logs. |
| **Trunc** | `...` or `// same` | Rewrite full code block. No placeholders. |
| **Avoid** | Guessing API | `context7-query-docs` mandatory. |

## References

- [references/methodology.md](references/methodology.md)
