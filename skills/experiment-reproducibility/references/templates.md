# Experiment Reproducibility Templates

## Seed Helper (Python)

```python
import random
import numpy as np
import torch

def set_all_seeds(seed: int = 42):
    """Set seeds for all common random number generators."""
    random.seed(seed)
    np.random.seed(seed)
    torch.manual_seed(seed)
    torch.cuda.manual_seed_all(seed)
    # For full determinism (may reduce performance):
    torch.backends.cudnn.deterministic = True
    torch.backends.cudnn.benchmark = False
```

**Rules**:
- Set seeds at the START of every script, before any random operations
- Log the seed value in experiment metadata
- Be aware: some operations are non-deterministic even with seeds (multi-GPU, certain CUDA ops)
- Document known non-determinism and its expected impact
- Use `CUBLAS_WORKSPACE_CONFIG=:4096:8` for fully deterministic CUDA

## Experiment Metadata Template (YAML)

```yaml
experiment:
  name: <descriptive-name>
  date: <YYYY-MM-DD>
  researcher: <name>

environment:
  python: <version>
  cuda: <version>
  gpu: <model, count>
  os: <version>
  key_libraries:
    torch: <version>
    transformers: <version>
    # ... other critical libraries

config:
  seed: <value>
  data:
    dataset: <name, version>
    split: <train/val/test sizes>
    preprocessing: <description or script path>
  model:
    architecture: <name>
    parameters: <count>
  training:
    epochs: <N>
    batch_size: <N>
    learning_rate: <value>
    optimizer: <name>
    scheduler: <name>
    # ... all hyperparameters

results:
  metrics:
    <metric_name>: <value ± std>
  training_time: <hours>
  checkpoint: <path>

notes: <any observations or anomalies>
```
