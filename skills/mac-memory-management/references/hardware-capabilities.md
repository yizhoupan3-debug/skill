# Apple Silicon Memory Envelope Reference

Use this reference to translate chip class into a practical memory policy.

## Memory Envelope by Chip Class

| Chip class | Typical envelope | Operational implication |
|---|---|---|
| Base M1 / M2 / M3 / M4 | Small unified-memory budgets | Favor tiny batches, low worker counts, and aggressive retention cleanup |
| Pro | Moderate unified-memory budgets | Can absorb larger activations, but DataLoader and cache pressure still matter |
| Max | Larger unified-memory budgets | Still bounded by unified memory; do not treat it like discrete VRAM |

## What Matters Operationally

- Unified memory is shared across model state, activations, preprocessing buffers, and the OS.
- A larger chip class extends the envelope, but it does not remove the need for batch fallback or retention cleanup.
- If the workload is close to the limit, assume DataLoader duplication and inference histories will matter before raw parameter count does.
- When moving from base to Pro/Max chips, re-measure worker count, batch size, and precision rather than copying defaults.

## Practical Rule

- Use the weakest chip class the repo must support as the default safety target.
- Treat chip capability as a ceiling for policy, not as a promise that a configuration is safe everywhere.
