# tdd-workflow — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- identifying the right test surface
- writing a failing test first
- confirming failure before implementation
- making the smallest change to pass
- re-running tests and then refactoring safely

This skill does not own:
- generic testing theory detached from the repository
- end-to-end performance benchmarking as the main task
- planning a multi-agent delegation strategy

If the task shifts to adjacent skill territory, route to:
- `$systematic-debugging` when the bug is not yet reproduced or isolated
- `$python-pro`, `$typescript-pro`, or another domain skill for language-specific implementation judgment

## Finding-driven framework role

This skill is a **Phase-2 executor / verifier overlay** in the shared finding-driven framework. Use it after a detector or gate has already established the failing behavior strongly enough to encode it as a regression test. It can consume structured findings or execution items as long as they preserve the target behavior and verification intent.

Typical consumed fields:
- `source_findings`
- `change_scope`
- `verification_strategy`
- `status`

Its main framework output is a verification result: fail-first confirmed, pass-after-fix confirmed, or blocked because the behavior could not be captured in a meaningful automated test.

## Required workflow

1. Identify the target behavior and the right test framework in the repo.
2. Write or update a test that should fail for the current behavior.
3. Run the test and confirm the failure.
4. Implement the smallest change needed to pass.
5. Re-run tests.
6. Refactor only while keeping the suite green.

## Core workflow

### 1. Intake

- Determine:
  - target behavior
  - affected module
  - existing test framework
  - smallest relevant test scope

Prefer the narrowest relevant test runner:
- Python: `pytest path/to/test_file.py -k case_name`
- JS/TS: `vitest`, `jest`, or repo-specific test command
- Other ecosystems: use the repository's existing test workflow

### 2. RED

- Write a test that expresses the desired behavior or reproduces the bug.
- Keep the test focused on one behavior.
- Run the smallest relevant test command.
- Confirm the test fails for the expected reason.

If the test does not fail:
- the test is wrong
- the bug is not reproduced
- or the behavior is already implemented

### 3. GREEN

- Implement the minimum change needed to make the failing test pass.
- Avoid speculative cleanup in this phase.
- Re-run the same narrow test first.
- Then run the broader relevant test slice if needed.

### 4. REFACTOR

- Improve names, structure, and duplication only after the tests are green.
- Keep refactors small and reversible.
- Re-run tests after each meaningful refactor.

## Framework-specific guidance

### Bug fix TDD

Use this sequence:
1. reproduce bug in a failing test
2. confirm failure
3. implement minimal fix
4. run regression test
5. run nearby suite

### New feature TDD

Use this sequence:
1. encode one behavior at a time
2. pass the smallest slice
3. iterate in small increments

### Refactor with TDD

Use this sequence:
1. add missing safety tests first
2. confirm green baseline
3. refactor in tiny steps
4. keep behavior unchanged

## Output defaults

Default output should contain:
- what behavior was captured
- what test was added or changed
- the fail → pass path
- verification status for the linked finding or execution item

Recommended structure:

````markdown
## TDD Summary
- Target behavior: ...
- Test surface: ...

## RED
- Added/updated test: ...
- Observed failing result: ...

## GREEN
- Minimal implementation change: ...

## REFACTOR
- Cleanup performed: ...

## Verification
- Ran: ...
- Result: ...
````

## Hard constraints

- Do not claim TDD if the failing test was never actually run.
- Do not skip the failing-test step unless the repo truly cannot support it, and say so explicitly.
- Do not mix large opportunistic refactors into the GREEN phase.
- Do not test implementation details when behavior-level testing is possible.
- If the bug is not reproduced yet, say that and switch to a debugging-first posture.

## Trigger examples

- "Use $tdd-workflow to fix this bug with a failing test first."
- "Write the regression test before changing the implementation."
- "先红后绿后重构，按 TDD 做这次改动。"
