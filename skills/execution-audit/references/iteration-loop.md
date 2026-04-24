# Iteration Loop

Use this reference when the user asks for repeated optimize-review-fix-rescore
cycles. `execution-audit` owns the loop as a verification overlay; it should not
compete as a separate owner.

Loop:

1. Define the scorecard and stop condition.
2. Run one bounded improvement pass.
3. Verify with evidence.
4. Re-score against the same criteria.
5. Stop when the target is met or the next pass has diminishing returns.

Do not run open-ended iterations without a bounded acceptance line.
