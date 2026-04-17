---
name: mac-memory-management
description: Optimize Apple Silicon ML runtimes for memory pressure, throughput, and MPS stability.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "1.2.0"
  platforms: [codex]
  tags:
    - mac
    - apple-silicon
    - mps
    - memory
    - unified-memory
    - oom
    - dataloader
    - pytorch
    - throughput
    - runtime
risk: medium
source: local
---
# mac-memory-management

This skill owns Mac-specific runtime optimization for ML workloads when unified memory, MPS behavior, DataLoader pressure, throughput limits, or device-path stability are the dominant constraint. It is the default Mac runtime owner for Apple Silicon training and inference loops; generic algorithmic rewrites still belong to `code-acceleration`.

## When to use

- The task is to keep a Mac ML workload from running out of memory, stalling, or becoming unstable
- The user mentions Apple Silicon, MPS, unified memory, OOM, memory spikes, or severe swap pressure
- The main work is batch-size fallback, gradient checkpointing, gradient accumulation, memory-safe DataLoader settings, throughput tuning, or explicit `torch.mps` runtime management
- The code must adapt to tight Mac memory budgets without pretending CUDA-style assumptions still apply
- The workload is on Mac and the question is which runtime levers to pull first: device path, batch size, worker count, caching, retention, or fallback behavior
- Best for requests like:
  - "Mac 上这个训练一直 OOM，帮我稳住"
  - "Apple Silicon 上怎么做内存管理"
  - "MPS 内存老爆，给我一套 batch fallback"
  - "DataLoader 在 Mac 上怎么调才不炸内存"
  - "Mac 上这段训练吞吐很差，怎么调 runtime"
  - "MPS 不稳定而且速度也不对，先怎么优化"

## Do not use

- The task is general model architecture, training strategy, or research engineering -> use `$ai-research`
- The task is generic code acceleration such as pandas -> polars, faster serializers, or hot-path rewrites with no Mac runtime constraint -> use `$code-acceleration`
- The task is experiment tracking, seeds, or reproducibility management -> use `$experiment-reproducibility`
- The task is non-Mac hardware or CUDA-first stack tuning -> use `$ai-research`
- The task is broad training setup guidance that is not memory-specific -> use `$ai-research`

## Task ownership and boundaries

This skill owns:
- Apple Silicon runtime policy for training, inference, eval, and preprocessing loops
- unified-memory-aware execution policy on macOS
- MPS vs CPU fallback when memory pressure or runtime instability dominates
- conservative batch sizing and gradient accumulation
- gradient checkpointing and precision tradeoffs when they reduce peak memory
- dataloader worker, caching, and preprocessing memory tradeoffs
- explicit `torch.mps` memory inspection and cleanup tactics
- memory-safe validation, inference, logging, and checkpoint behavior
- throughput tuning that is specific to Mac runtime shape: worker count, caching, host-device movement, microbatching, and stable device-path selection

This skill does not own:
- model architecture choice
- Optuna or hyperparameter search strategy
- 5-seed evaluation policy
- paper-quality experiment design
- generic non-Mac performance engineering
- purely algorithmic rewrites that are not Mac-runtime-specific

## Companion routing policy

- [`$ai-research`](../ai-research/SKILL.md) and [`$autoresearch`](../autoresearch/SKILL.md) should proactively check this skill first before expensive runs on Apple Silicon, MPS, or tight unified-memory paths, not only after an OOM.
- Pair this skill with [`$code-acceleration`](../code-acceleration/SKILL.md) when a Mac runtime bottleneck survives device-path, batching, worker, caching, and retention fixes and still needs generic hot-path rewrites.
- Keep this skill focused on Mac runtime control; generic memory-efficient rewrites and non-Mac accelerations remain in `code-acceleration`.

## Optimization order

1. stabilize the intended device path
2. restore memory headroom
3. remove obvious runtime waste
4. benchmark throughput and latency
5. escalate to generic hot-path rewrites only if the Mac runtime layer is no longer the main blocker

## Required workflow

1. Confirm the task shape:
   - object: Mac training/inference/data path under runtime pressure
   - action: stabilize, reduce memory, recover throughput, add fallback, prevent OOM
   - constraints: Apple Silicon model, unified memory budget, framework, target batch size
   - deliverable: code or config changes plus runtime guidance and verification
2. Identify the dominant runtime limiter:
   - unstable MPS path or op coverage
   - model activations
   - batch size
   - dataloader worker duplication
   - cached preprocessing
   - retained tensors, logs, or prediction histories
   - host-device transfer and synchronization overhead
   - under-filled batches or poor stage overlap
3. Stabilize the device path first:
   - verify `mps` with a smoke pass
   - compare against a controlled `cpu` fallback when MPS behavior is unstable
   - keep checkpoint load/save and tensor movement backend-neutral
4. Apply the highest-signal runtime protections next:
   - reduce batch size
   - add bounded batch fallback
   - add gradient accumulation if needed
   - add checkpointing only where peak activations dominate
   - wrap eval/inference in `no_grad` or `inference_mode`
   - cap worker count and avoid unnecessary `pin_memory`
   - remove retained histories and oversized preprocessing caches
   - tune worker count, microbatching, and preprocessing placement for stable throughput
5. Keep CPU fallback available when MPS memory behavior is unstable.
6. Verify with a smoke run and a short benchmark before claiming the path is safe or fast.

## Policy reference

- Read [references/mac-policies.md](references/mac-policies.md) for detailed memory rules.
- Read [references/hardware-capabilities.md](references/hardware-capabilities.md) when the chip class materially changes the safety envelope.
- Read [references/runtime-optimization.md](references/runtime-optimization.md) for device-path, throughput, and stability playbooks.

## Hard constraints

- Do not assume CUDA memory heuristics apply unchanged on Mac.
- Do not increase worker count or cache size without measuring memory impact.
- Do not trade away runtime stability for a small throughput gain.
- Do not keep full prediction histories when streaming metrics are enough.
- Do not claim stability until a smoke run passes on the intended device path.
- Do not claim a speed win unless the device path, batch policy, and benchmark window are stated.
- Do not treat mixed precision or checkpointing as free wins; verify peak-memory reduction against real throughput.

## Trigger examples

- "Apple Silicon 上这段训练怎么做内存管理？"
- "MPS 一直 OOM，给我 batch fallback。"
- "Mac 上这段训练为什么又慢又不稳？"
- "帮我把 Apple Silicon 上的训练 runtime 调顺。"
- "Use $mac-memory-management to harden this Mac training loop against memory spikes."
