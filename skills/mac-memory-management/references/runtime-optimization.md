# Mac Runtime Optimization Reference

Use this reference when the main question is not only "how do I stop the OOM" but
"how do I make the Apple Silicon runtime safe and reasonably fast."

## Optimization order

On Mac, use this order unless repo-specific evidence says otherwise:

1. make the device path stable
2. recover memory headroom
3. remove obvious runtime waste
4. benchmark throughput and latency
5. escalate to generic hot-path rewrites only if the Mac runtime layer is no longer the blocker

## Device-path policy

Start by proving the intended backend path works.

- Smoke test `mps` before large runs.
- Keep a controlled `cpu` fallback available when MPS is unstable, unsupported, or slower in practice.
- Avoid backend-specific checkpoint assumptions that make fallback impossible.
- Do not commit to a throughput optimization if it only works on one unstable backend path.

## Throughput levers that are Mac-specific

Use these before jumping to generic algorithm rewrites:

- batch size and microbatch shape
- gradient accumulation versus live activation size
- dataloader worker count
- preprocessing location and cache policy
- retained tensor cleanup
- host-device transfer reduction
- `inference_mode()` and `no_grad()` on non-training paths

Typical rule:
- if memory is tight, lower live state first
- if throughput is low but memory is safe, tune batch shape and worker count before adding complexity
- if MPS is unstable, prefer a measured CPU fallback over repeated retries

## Data pipeline policy

Data movement and preprocessing often dominate on Mac before model math does.

Check:
- duplicated dataset state across workers
- large collate outputs
- cached tokenization or preprocessing that exceeds memory headroom
- unnecessary tensor conversions and transfers
- logging or metrics that retain full predictions

Prefer:
- low worker counts unless measurement proves otherwise
- bounded caches
- vectorized preprocessing over more worker processes when workers mostly add memory pressure
- late materialization and streaming metrics

## Short benchmark protocol

When claiming an improvement, capture at least:

- device path used: `mps` or `cpu`
- macOS version
- Python and PyTorch versions
- chip class and unified memory capacity
- MPS availability and relevant runtime flags
- batch size and accumulation policy
- worker count
- precision policy
- peak RSS, swap, or memory-pressure behavior, including the measurement source
- throughput or latency over a fixed short window with the same warmup and sample count before and after the change
- statistic reported, such as median step time, p95 step time, samples/sec, or tokens/sec

If stability is the blocker, record:
- whether the run completed
- whether fallback was required
- the first safe batch size or worker policy that passed

## Escalation and ownership handoff

Hand work back to the current implementation owner when:

- the Mac runtime is stable
- the workload is still bottlenecked by serialization, Python overhead, data layout, or generic hot paths
- a change is no longer primarily about Apple Silicon runtime policy

At that point, keep this skill as the Mac platform guardrail while the
implementation owner handles generic performance rewrites.
