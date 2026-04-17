# Anti-Fatigue Protocol — Multi-Round Optimization Reference

> This file contains **multi-round optimization specific** anti-fatigue mechanisms.
> For general anti-laziness patterns, see [`anti-laziness`](../../anti-laziness/SKILL.md).

## Root Causes of Multi-Round Fatigue

| Root Cause | Mechanism | Countermeasure |
|-----------|-----------|----------------|
| **Context bloat** | Long conversations → shorter outputs | Delta-only carry-forward; per-round context reset |
| **Stop pressure** | RLHF rewards "good enough" stopping | Explicit quality bar: ≥1 actionable item per round |
| **False convergence** | Model "believes" prior rounds covered all | 3-step false-convergence challenge protocol |
| **Repetition suppression** | Internal de-dup blocks similar suggestions | Dimension rotation forces genuinely new angles |
| **Role fatigue** | Role constraints weaken over long conversations | Per-round role re-statement |

## Dimension Rotation — Detailed Audit Questions

### D1: Readability & Naming
- Variable/function names self-documenting? Abbreviations consistent?
- Nesting depth ≤3? Code understandable without comments?

### D2: Performance & Efficiency
- O(n²) reducible to O(n)? Repeated computations? N+1 queries?

### D3: Security & Robustness
- Input validated? Injection risks? Secrets in config? Error messages leak info?

### D4: Architecture & Modularity
- SRP followed? Tight coupling? God objects? Circular dependencies?

### D5: Error Handling & Edge Cases
- Null/empty handled? Network failures? Timeout/retry? Race conditions?

### D6: Test Coverage & Testability
- Critical paths covered? Hard-to-test code? Edge cases in tests?

### D7: Documentation & Contracts
- Public API documented? "Why" comments? README current? Breaking changes noted?

### D8: Consistency & Standards
- Naming conventions consistent? Linter config followed? Same patterns reused?

### D9: User Experience & API Ergonomics
- Error messages helpful? Defaults sensible? API minimal? Feedback for long ops?

### D10: Maintainability & Tech Debt
- Magic numbers named? Dead code? Copy-paste DRY? TODOs tracked?

## False-Convergence Challenge — Steps

### Step 1: Switch Perspective

| Perspective | Question |
|------------|---------|
| Hostile reviewer | "What would I flag to reject this PR?" |
| New team member | "What would confuse me on day 1?" |
| Security auditor | "Where would I attack this?" |
| Production SRE | "What breaks at 3 AM on a holiday?" |
| End user | "What error messages frustrate me?" |

### Step 2: Switch Granularity

| Current | Switch to |
|---------|----------|
| Architecture | Function-level bodies |
| Function | Line-level micro-improvements |
| Line | Cross-file interactions |
| Cross-file | Cross-concern intersections |

### Step 3: Switch Dimension & Orthogonal Verification (R4 Convergence Math)

Pick next uncovered from rotation table. If all 12 covered, pick the one with fewest findings for a deeper pass.

**Strict Convergence Mathematics**: Genuine convergence can ONLY be declared if you achieve **Two Consecutive Null Deltas** across two entirely **orthogonal** dimensions (e.g., UI vs Security). A single perspective switch finding nothing is NOT convergence.

### Convergence Declaration Template

```
✅ Genuine convergence confirmed (2 Orthogonal Null Deltas achieved).
   - Test 1 (Perspective: [X], Dimension: [Y]): 0 Findings
   - Test 2 (Perspective: [A], Dimension: [B]): 0 Findings
```
