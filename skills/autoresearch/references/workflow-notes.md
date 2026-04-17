# Workflow Notes

These notes summarize the operating constraints that shaped `autoresearch`.

## Looping behavior

- ReAct shows that reasoning traces and actions should be interleaved so the agent can
  update plans from observations rather than planning once and drifting.
- Reflexion shows that post-trial reflection should be stored as reusable memory, not
  discarded after each run.
- The AI Scientist demonstrates the full research loop: idea generation, code, experiments,
  visualization, paper drafting, and review can be automated as a repeated cycle.

## State discipline

- LangGraph persistence treats state as checkpoints attached to a thread; you resume
  from checkpoint boundaries, not arbitrary in-flight states.
- A thread identifier is the primary handle for resuming a run, so each project or
  branch needs a stable state key.
- MLflow tracking organizes metadata and artifacts around runs and experiments, with local
  or server-backed storage for cleaner management and shared access.
- DVC tracks experiments on top of Git, with explicit bookkeeping for code, data,
  parameters, artifacts, and metrics, which reinforces reproducibility and comparison.

## Practical implications for `autoresearch`

- Write the protocol before a run starts.
- Keep experiment artifacts separate from conversation state.
- Record enough metadata to restart, compare, and explain a run later.
- Let one orchestrator own merges and direction changes.
- Use parallel workers only for independent branches with clear outputs.
- Treat resumability as a design constraint, not a cleanup step after failure.

## Source links

- [ReAct: Synergizing Reasoning and Acting in Language Models](https://arxiv.org/abs/2210.03629)
- [Reflexion: Language Agents with Verbal Reinforcement Learning](https://arxiv.org/abs/2303.11366)
- [The AI Scientist: Towards Fully Automated Open-Ended Scientific Discovery](https://arxiv.org/abs/2408.06292)
- [LangGraph Persistence](https://docs.langchain.com/oss/python/langgraph/persistence)
- [MLflow Tracking](https://mlflow.org/docs/latest/ml/tracking/)
- [DVC Experiment Tracking](https://dvc.org/doc/use-cases/experiment-tracking)
