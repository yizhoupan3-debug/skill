# Planning Rubric

Use this rubric when auditing output from `idea-to-plan` or any other planning-first owner.

## Pass Conditions

- The plan restates the problem in operational terms instead of copying the prompt.
- The plan separates goals from non-goals.
- The plan compares at least two plausible routes before selecting one.
- The chosen route is justified with explicit tradeoffs, not only preference language.
- Assumptions are listed explicitly and are materially relevant to the chosen route.
- Open questions are listed explicitly and are not buried inside prose.
- Execution decomposition is separated from strategic reasoning.
- Stop conditions or blockers are named before implementation begins.
- The handoff into checklist / implementation is explicit.

## Fail Signals

- The output jumps straight into file-by-file implementation steps without route selection.
- The plan is only a checklist with no tradeoff analysis.
- Assumptions are implicit, missing, or mixed with low-level coding notes.
- Open questions are absent even though external facts or user choices are unresolved.
- The plan selects one route without naming discarded alternatives.
- Risks are generic and not tied to this repo or task.
- The plan has no clear boundary between strategic plan and execution checklist.

## Review Questions

1. What problem is being solved, and what is explicitly out of scope?
2. Which alternative routes were considered, and why were they rejected?
3. Which assumptions must hold for the chosen route to remain valid?
4. Which unknowns still require user input or external evidence?
5. What conditions should stop implementation from starting immediately?
6. What belongs in the later execution checklist rather than in the strategy document?
