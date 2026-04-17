# AI/ML Research Toolchain Reference

## Training & Fine-tuning

| Tool | Purpose | When to use |
|------|---------|-------------|
| **Hugging Face Transformers** | Model hub, training pipelines | Default for transformer-based work |
| **PyTorch Lightning / Fabric** | Training loop abstraction | Multi-GPU, mixed precision, logging |
| **DeepSpeed / FSDP** | Distributed training, ZeRO optimization | Large model training (7B+) |
| **Unsloth** | Efficient LoRA fine-tuning | Budget-constrained fine-tuning |
| **TRL (Transformer Reinforcement Learning)** | RLHF, DPO, PPO | Post-training alignment |
| **Axolotl** | Fine-tuning config orchestration | Multi-format fine-tuning setups |

## Experiment Tracking

| Tool | Purpose | When to use |
|------|---------|-------------|
| **Weights & Biases (wandb)** | Metric logging, artifact tracking | Production experiments |
| **MLflow** | Experiment lifecycle management | Self-hosted, open-source needs |
| **TensorBoard** | Lightweight metric visualization | Quick local experiments |
| **Aim** | Experiment comparison UI | Open-source W&B alternative |

## Inference & Serving

| Tool | Purpose | When to use |
|------|---------|-------------|
| **vLLM** | High-throughput LLM serving | PagedAttention, continuous batching |
| **TGI (Text Generation Inference)** | HuggingFace inference server | HF ecosystem compatibility |
| **SGLang** | Structured generation serving | Constrained decoding, multi-turn |
| **Ollama** | Local model running | Local development, privacy |
| **llama.cpp** | CPU/small-GPU inference | Edge deployment, quantized models |

## Quantization & Optimization

| Tool | Purpose | When to use |
|------|---------|-------------|
| **AWQ** | Activation-aware weight quantization | 4-bit with quality preservation |
| **GPTQ** | Post-training quantization | 4-bit for smaller models |
| **bitsandbytes** | In-situ quantization | QLoRA, NF4 training |
| **ONNX Runtime** | Cross-platform inference optimization | Deployment pipelines |
| **TensorRT-LLM** | NVIDIA-optimized inference | NVIDIA GPU deployments |

## RAG & Retrieval

| Tool | Purpose | When to use |
|------|---------|-------------|
| **LangChain / LlamaIndex** | RAG pipeline orchestration | Complex retrieval workflows |
| **pgvector** | Vector similarity search (PostgreSQL) | PostgreSQL-based apps |
| **Chroma / Weaviate / Qdrant** | Vector databases | Dedicated vector search |
| **sentence-transformers** | Embedding model fine-tuning | Custom embedding needs |
| **Cohere Rerank / ColBERT** | Reranking retrieved results | Improving retrieval precision |

## Evaluation & Benchmarking

| Tool | Purpose | When to use |
|------|---------|-------------|
| **lm-eval-harness** | Standardized LLM benchmarks | Comparing against published scores |
| **bigcode-evaluation-harness** | Code generation benchmarks (HumanEval, MBPP) | Evaluating code LLMs |
| **HELM** | Holistic LLM evaluation | Multi-metric evaluation |
| **ragas** | RAG system evaluation | Measuring retrieval + generation |
| **inspect_ai** | Model safety evaluation | Safety and alignment testing |
| **promptfoo** | Prompt evaluation | Systematic prompt comparison |
| **NeMo Evaluator** | NVIDIA NeMo evaluation framework | NeMo ecosystem models |

## Mechanistic Interpretability

| Tool | Purpose | When to use |
|------|---------|-------------|
| **TransformerLens** | Transformer internals access, hooks, activation caching | Circuit analysis, attention pattern study |
| **nnsight** | Remote model intervention and tracing | Interpretability on large models without local GPU |
| **pyvene** | Causal intervention framework | Interchange intervention, causal mediation analysis |
| **SAELens** | Sparse Autoencoder training and analysis | Feature extraction, superposition study |

## Common Pitfalls

1. **Data leakage in evaluation**: Always check for train/test overlap, especially with web-scraped data
2. **Tokenizer mismatch**: Ensure tokenizer matches the model checkpoint exactly
3. **Mixed precision gotchas**: bf16 is NOT the same as fp16; check hardware support
4. **Gradient accumulation math**: effective_batch = micro_batch × grad_accum × num_gpus
5. **OOM debugging**: Use `torch.cuda.max_memory_allocated()` before expensive runs
6. **Checkpoint corruption**: Always validate checkpoint loading before long training runs
7. **Random seed scope**: Set seeds for `random`, `numpy`, `torch`, AND dataloader workers
8. **Metric reporting**: Always report the metric definition used, not just "accuracy" or "F1"
