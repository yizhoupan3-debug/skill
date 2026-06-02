# Mac Memory Policy Reference

This reference covers the memory behavior that matters for ML workloads on macOS and Apple Silicon.

## Unified Memory Pressure

- Treat CPU, GPU, and framework allocations as competing for the same unified memory budget.
- Watch for swap growth, pressure warnings, and resident-set expansion before the machine becomes unusable.
- Prefer smaller live tensor sets over relying on the OS to recover memory pressure later.
- Assume large preprocessing buffers, cached batches, and histories can be just as expensive as model weights.

## MPS Behavior

- Keep `mps` paths behind a smoke test; fall back to `cpu` if the intended train or inference path fails.
- Do not assume CUDA memory semantics, allocator behavior, or transfer tricks carry over to MPS.
- Treat mixed precision on MPS as an explicit tradeoff, not a default.
- If an op or model slice is unstable on MPS, prefer a controlled CPU fallback over repeated retries.
- Keep checkpoint loading and tensor movement backend-neutral.

## DataLoader Pressure

- Keep `num_workers` low unless measurement proves more workers help without memory blow-up.
- Start from `0` to `2` workers on Mac unless the repo already has a measured safe default.
- Avoid `pin_memory=True` as a default for MPS-first paths.
- Keep `persistent_workers=False` unless non-zero workers and dataset size justify it.
- Watch for duplicated dataset state, forked caches, and large collate outputs.
- Prefer simple, vectorized preprocessing over multiprocessing when the extra workers are mainly increasing memory use.

## Gradient and Inference Patterns

- Wrap validation and inference in `torch.no_grad()` when gradients are not needed.
- Prefer `torch.inference_mode()` for pure inference paths where autograd metadata is unnecessary.
- Detach tensors before logging, metric accumulation, visualization, or queueing work to another thread.
- Do not retain full prediction histories when streaming metrics are enough.
- Clear stale references inside long loops when memory growth is due to object retention, not model size.

## Memory-Saving Tradeoffs

- Reduce batch size first when peak memory is the blocker.
- Use gradient accumulation when you need the effective batch size but cannot afford the live activation set.
- Use gradient checkpointing only where recomputation is cheaper than the extra memory pressure.
- Use lower precision only when the numerical and backend behavior has been validated on the Mac path.
- Keep automatic batch fallback bounded: try, shrink, retry, and record the winning size.

## Stability Diagnostics

- Start with an import/config smoke test.
- Run one tiny forward and backward pass before full training.
- Run one short train loop or inference slice on the intended device path.
- Save and reload a checkpoint before claiming the path is safe.
- Record the exact failure mode: OOM, backend allocation error, swap thrash, worker duplication, or retained tensors.
- Log the winning batch size, worker count, precision choice, and device path so the fix is repeatable.
