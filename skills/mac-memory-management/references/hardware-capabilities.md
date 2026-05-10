# Apple Silicon Memory Envelope Reference

Use this reference to translate chip class into a practical memory policy.

## Memory Envelope by Chip Class

| Chip class | Typical envelope | Operational implication |
|---|---|---|
| Base M1 / M2 / M3 / M4 | Small unified-memory budgets | Favor tiny batches, low worker counts, and aggressive retention cleanup |
| Pro | Moderate unified-memory budgets | Can absorb larger activations, but DataLoader and cache pressure still matter |
| Max | Larger unified-memory budgets | Still bounded by unified memory; do not treat it like discrete VRAM |

## Conservative Starting Policy

Use these as starting points, not promises:

| Chip class | Batch fallback posture | DataLoader posture | Cache posture |
|---|---|---|---|
| Base M1 / M2 / M3 / M4 | Start with the smallest practical batch and add bounded shrink-on-failure fallback | Start at `num_workers=0`; try `1-2` only after measuring memory pressure | Disable or tightly bound preprocessing caches until the loop is stable |
| Pro | Start from the base-safe policy, then increase batch or accumulation after a stable smoke run | Try `0-2` workers before wider fan-out | Bound caches by measured headroom and clear retained histories |
| Max | Increase live state only after confirming swap and pressure stay flat | Worker count can be tuned for throughput, but revert if RSS or swap grows across epochs | Larger caches still need explicit caps and eviction |

## What Matters Operationally

- Unified memory is shared across model state, activations, preprocessing buffers, and the OS.
- A larger chip class extends the envelope, but it does not remove the need for batch fallback or retention cleanup.
- If the workload is close to the limit, assume DataLoader duplication and inference histories will matter before raw parameter count does.
- When moving from base to Pro/Max chips, re-measure worker count, batch size, and precision rather than copying defaults.

## Practical Rule

- Use the weakest chip class the repo must support as the default safety target.
- Treat chip capability as a ceiling for policy, not as a promise that a configuration is safe everywhere.
