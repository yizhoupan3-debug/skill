---
name: experiment-reproducibility
description: |
  Ensure and manage research experiment reproducibility: environment capture,
  random seed management, data versioning, configuration tracking, result
  validation, and artifact archival. Use when the user asks "怎么保证可复现",
  "环境管理", "随机种子", "数据版本控制", "实验配置", "reproducibility",
  "environment snapshot", "seed management", "DVC", "MLflow tracking",
  "实验记录", or needs systematic experiment reproducibility procedures
  rather than one-off model training.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: preferred
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 怎么保证可复现
  - 环境管理
  - 随机种子
  - 数据版本控制
  - 实验配置
  - reproducibility
  - environment snapshot
  - seed management
  - DVC
  - MLflow tracking
  - 预注册
  - 方案偏离
  - 主分析
  - 探索性分析
metadata:
  version: "1.0.3"
  platforms: [supported]
  tags:
    - reproducibility
    - experiment-tracking
    - environment
    - seed
    - data-versioning
    - mlops
risk: low
source: local

---

# Experiment Reproducibility

This skill owns **experiment reproducibility management** for research.

## When to use

- The user wants to ensure experiment results are reproducible
- The user needs environment capture / dependency management
- The user wants systematic seed management across frameworks
- The user needs data versioning for experiments
- The user wants experiment configuration tracking and logging
- The user needs result validation protocols

## Do not use

- The user wants one front door for a research-project task rather than reproducibility only -> use `$research-workbench` and keep this skill only for reproducibility constraints
- The task is about training models -> answer in the current implementation context, or use `$mac-memory-management` when Apple Silicon memory/MPS behavior dominates
- The task is about CI/CD pipeline -> answer in the current implementation context, or use `$gh-fix-ci` only for failing GitHub Actions checks
- The task is about general Docker setup -> answer in the current implementation context
- The task is about paper writing → use `$paper-writing`

## Cross-references

- Manuscript workflow map: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md)
- **科研纪录最低清单**（预注册、主/探索分析、方案偏离、数据 DOI、软件 pin）：[`references/research-record-minimum.md`](references/research-record-minimum.md)；与仓库连续性（`artifacts/current/`、`SESSION_SUMMARY` / `NEXT_ACTIONS` / `EVIDENCE_INDEX`）对齐见 [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md) §科研纪录与仓库连续性
- `$research-workbench`, project owners, and experiment owners may use this skill as the rigor / reproducibility lane
- Inner-loop research automation should route deep reproducibility requirements here
- Works with performance work when before/after benchmarks, throughput measurements, or peak-memory evidence must be reproducible across runs
- Works with `$mac-memory-management` when reproducibility must account for Mac memory constraints and fallback behavior
- When invoked as a **gate-chain lane owner** (G2 / G5 / G14 reproducibility
  checks under the manuscript protocol), follow the lane contract in
  [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md); produce only the
  gate-required reproducibility evidence and hand back to the protocol main
  chain.

## User-facing output posture

When this skill is co-invoked with paper review or writing lanes:

- report only the reproducibility blocker that affects acceptance risk
- provide the smallest executable fix next
- avoid dumping full checklist text unless explicitly requested

## Reproducibility Layers

### Layer 1: Environment

**Goal**: Anyone can recreate your exact software environment.

| Tool | When to use | Command |
|------|-----------|---------|
| **conda** | Complex ML environments | `conda env export > environment.yml` |
| **pip** | Python-only projects | `pip freeze > requirements.txt` |
| **uv** | Modern Python projects | `uv pip compile pyproject.toml` |
| **Docker** | Full system reproducibility | `Dockerfile` with pinned versions |
| **Nix** | Absolute reproducibility | `flake.nix` with locked inputs |

**Rules**:
- Pin ALL library versions, never use `>=` in production configs
- Record Python version exactly (3.11.7, not just 3.11)
- Record CUDA version and GPU driver version
- Record OS version for system-level dependencies
- Commit environment files to git alongside code

### Layer 2: Random Seeds

**Goal**: Same code + same data + same seeds = same results.

> See [references/templates.md](references/templates.md) for the `set_all_seeds()` helper and seed rules.

Key rules: set seeds at script START, log seed value, document non-determinism, use `CUBLAS_WORKSPACE_CONFIG=:4096:8` for full CUDA determinism.

### Layer 3: Data Versioning

**Goal**: Track exactly which data was used for each experiment.

| Tool | When to use |
|------|-----------|
| **DVC** (Data Version Control) | Large datasets, git-like workflow |
| **Git LFS** | Medium datasets (< 2GB) |
| **Hugging Face Datasets** | Standardized ML datasets |
| **Manual checksums** | Simple projects |

**Rules**:
- Never modify raw data in place; keep originals immutable
- Record data preprocessing steps as code, not manual operations
- Checksum (SHA-256) input datasets and record in experiment config
- Version train/val/test splits explicitly
- Document any data filtering, cleaning, or augmentation

### Layer 4: Configuration

**Goal**: Every hyperparameter and setting is tracked and recoverable.

**Recommended tools**:
- **Hydra**: YAML-based config composition
- **wandb.config**: Auto-logged with experiment tracking
- **MLflow params**: Parameter logging with experiment tracking
- **Simple YAML/JSON**: For lightweight projects

**Rules**:
- Use config files, NOT command-line arguments scattered in scripts
- Log the complete config with every experiment run
- Include model architecture, optimizer, scheduler, data, and training params
- Record any manual overrides separately from base config
- Git-commit configs before running (protocol locking)

### Layer 5: Result Validation

**Goal**: Verify that results are genuine and consistent.

**Validation checklist**:
- [ ] Run the same experiment 3+ times with different seeds
- [ ] Compare results across runs: Mean ± Std should be stable
- [ ] Check for NaN/Inf in losses and metrics
- [ ] Verify that baseline results match published numbers
- [ ] Spot-check a subset of predictions manually
- [ ] Confirm that test set was never seen during training
- [ ] Log all intermediate metrics, not just final results

## Experiment Metadata Template

Every experiment should log: experiment info, environment (Python/CUDA/GPU/OS/libraries), config (seed/data/model/training), results (metrics/time/checkpoint), and notes.

> See [references/templates.md](references/templates.md) for the full YAML template.

## Hard Constraints

- Do not skip environment recording for "quick" experiments
- Do not trust results from a single run without variance estimates
- Do not modify data or code after recording results without re-running
- Do not use floating seeds (random seeds that change per run) in final results
- Always commit code before running experiments (protocol locking)
- Record negative results with the same rigor as positive results
- If the user only asks "下一步做什么", do not turn that into a reproducibility-only task; return to `$research-workbench` with only the reproducibility blocker noted

## Trigger examples

- "怎么保证实验可复现"
- "帮我设置随机种子管理"
- "实验环境怎么记录"
- "数据版本控制怎么做"
- "Ensure this experiment is reproducible"
- "帮我写实验 metadata 模板"
- "DVC 怎么配置"
- "实验配置管理最佳实践"
