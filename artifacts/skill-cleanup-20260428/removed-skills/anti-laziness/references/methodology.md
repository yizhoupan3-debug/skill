# Anti-Laziness Methodology — Detailed Reference

> Expanded detection rules, cognitive escalation, and debugging methodology
> for the `anti-laziness` skill.

## Five-Step Methodology (adapted from PUA debugging + NoPUA wisdom)

### Step 1: Stop (止)

Stop and list all previous attempts. Find the common failure pattern.
If all attempts share the same core approach (only varying parameters),
you are spinning wheels — not making progress.

### Step 2: Observe (观)

Execute these 5 dimensions in order:

1. **Read failure signals word-by-word**. Not skim — read every character
   of the error message / rejection / empty result. 90% of answers are
   in the error you ignored.

2. **Active search**. Do not rely on memory:
   - Code: search full error message
   - Research: search multiple keyword angles
   - API/tools: search official docs + Issues

3. **Read original material**. Not summaries or your memory:
   - Code: 50 lines of context around the failure point
   - API: official documentation text
   - Research: primary source, not second-hand citations

4. **Verify all assumptions**. Every condition you assumed true —
   verify with tools:
   - Code: version, path, permissions, dependencies
   - Data: fields, format, value range
   - Logic: edge cases, exception paths

5. **Flip assumption**. If you've been assuming "problem is in A",
   now assume "problem is NOT in A" and investigate from the opposite direction.

Complete dimensions 1-4 before asking the user questions.

### Step 3: Reflect (转)

- Am I repeating the same approach with different parameters?
- Am I only looking at surface symptoms, not root cause?
- Should I have searched but didn't? Should I have read a file but didn't?
- Did I check the simplest possibility? (typo, format, precondition)

### Step 4: Act (行)

Every new approach must satisfy three conditions:
- **Fundamentally different** from previous approaches (not parameter tweaks)
- Has a clear **verification criterion**
- Produces **new information** on failure

### Step 5: Learn (悟)

What solved it? Why didn't I think of it earlier? What's left untried?

**Post-solution extension**: After solving, don't stop. Check for similar
issues, verify the fix is complete, look for preventive measures.

**Detection signals**:
- Concluded without using search when search was available
- Concluded without reading source file when it was accessible
- Concluded without running a diagnostic command when possible

**Forced action**:
1. Identify available but unused tools
2. Use at least one before concluding
3. Incorporate tool output into finding

4. Use a completely different debugging tool or pivot from code changes to state inspection.

### Pattern 5: Passive Wait (False Convergence Detection)

**Detection signals**:
- Fixed one issue and immediately declared completion.
- Claiming "it works now" without empirical proof.
- No check for ripple effects or similar anti-patterns in adjacent files.
- Refusal to run test suites or manual verification commands.

**Forced action**:
1. Prove the fix: execute build, test suite, or run a manual verification command (e.g., `curl`, `npm test`, Python script execution).
2. Broad-phase scan: use `grep` or search tools to find if the same mistake exists elsewhere in the repo.
3. Only declare victory after providing explicit stdout/stderr evidence of success.

### Pattern 6: Code Truncation Laziness (Proactive Self-Catch)

**Detection signals**:
- Generating code blocks with `...`, `// remains unchanged`, `// insert your code here`.
- Supplying a partial `write_to_file` when the file needs complete definitions.

**Forced action**:
1. Immediately halt generation.
2. If the edit is small, use `multi_replace_file_content` tightly.
3. If the user explicitly requested "完整代码" (complete code), you MUST write the entire file top-to-bottom without skipping a single line.

## Cognitive Escalation Details (Strict Enforcement)

### Level: Switch Eyes (2nd failure)

The goal is to see the same problem from a different vantage. It is FORBIDDEN to stay in the original file/logic layer:
- If you've been looking at code, switch to reading configuration / build tools / dependency trees.
- If you've been looking at one file, you MUST inspect the caller/callee or execute a test script (`grep -r`, `pytest`, etc.).
- If you've been assuming the problem is in layer X, look at layer X±1 and verify with runtime logs.

### Level: Elevate (3rd failure)

Force yourself into broader context. You are PENALIZED (operationally blocked) if you guess without reading:
- Document the FULL error message. Do not abbreviate. Use `cat` or `tail` to read the log explicitly.
- Read the actual source code of the external libraries/frameworks involved. Do not hallucinate API specs.
- Formulate 3 mutually exclusive hypotheses. Test each one systematically, and output a table of `[Hypothesis | Test Command | Result]`.

### Level: Reset to Zero (4th failure)

All assumptions are compromised:
- Execute the full `7-item clarity checklist` and output the results explicitly.
- Propose 3 completely orthogonal hypotheses (e.g., hardware exhaustion, bad file descriptor, silent upstream truncation).
- Try each in sequence with predefined empirical pass/fail criteria.

### Level: Structured Handoff (5th+ failure)

Do not give up passively, but do not hallucinate fixes. Organize knowledge responsibly:
1. Verified Fact List (Only include facts proven by shell command snippets).
2. Falsified Hypotheses (What you tried and why it failed).
3. Minimal Repro Case (A 1-line curl or script segment demonstrating the issue).
4. Recommended next directions (e.g., "Require root", "Wait for DNS propagation").

## Anti-Circumvention (The Iron Rule)

1. **NO HALLUCINATED EVIDENCE**: If you find yourself outputting: "The issue should be resolved now," but you have not actually executed a tool command (`npm run test`, `python script.py`, or `cat` for system logs) that conclusively displays `stdout/stderr` proof in this turn, YOU ARE VIOLATING THIS SKILL.
2. **NO MANUAL WORKOFFLOAD**: You are FORBIDDEN from asking the user to manually perform a check (e.g., "Please check if your port is open") unless you first output a `[Self-Diagnostic Evidence]` block demonstrating that you invoked `curl`, `lsof`, `netstat`, or `ping`.

### Pattern 2: Blame Shifting & Context Dodging
**Signals**: Suggesting manual checks, claiming upstream/env bugs without logs, or asking for context before workspace search.
**Action (Hard Block)**: Execute diagnostic commands (`env`, `cat`, github search) BEFORE making claims. If claiming a third-party bug, you **MUST** provide a GitHub issue link or a minimal reproduction script proving it. Output `[Self-Diagnostic Evidence]` block.
3. **TOOL IMPOTENCY ESCAPE**: If you lack the required tool, you MUST explicitly state: `[UNVERIFIED: Cannot test due to lack of network/root access]`. Do not fake a successful outcome.
4. **HANGING COMMAND PREVENTION**: If a verification command (e.g., a dev server) hangs indefinitely, you MUST background it or term it. You cannot use "the terminal is hanging" as an excuse for Passive Wait. Test with a local curl or timeout wrapper instead.

### Edge Case: Environment Genuinely Unreachable
If you are entirely blocked from self-verification (e.g., firewall strict drop, no runtime environment), you must STOP guessing. Instead, provide a 1-click **Copy-Pasteable Diagnostic Script** (Bash/Python) for the user to run on their end, and explicitly wait for their output.

## Prescribed Evidence Formats

### [Self-Diagnostic Evidence]
When escalating to the user under "Blame Shifting", print:
```markdown
[Self-Diagnostic Evidence]
- Command 1: `...` -> Failed (Connection refused)
- Command 2: `...` -> Timeout
- Conclusion: Confirmed blocked port, escalating to user.
```

### [Verification Evidence]
Mandatory when declaring a task "Complete" or "Verified".
```markdown
[Verification Evidence]
- Test Command: `...`
- Result: SUCCESS (stdout: "...")
- Ripple Check: Verified no regressions in [file1], [file2]
- Final Status: 100% Empirical Match
```

### Forced Reflection Format (The Penalty Box)

When intercepted by this skill, output exactly:
```yaml
---
anti_laziness_intervention: true
pattern_detected: "[Pattern Number / Name]"
assumptions_falsified:
  - "[List previous bad assumptions]"
forced_next_step: "[What you MUST do instead]"
---
```
