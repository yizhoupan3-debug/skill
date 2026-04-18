# runbooks

## 标准操作

- 统一维护入口：python3 scripts/run_memory_automation.py --workspace <workspace>
- 合并记忆：python3 scripts/consolidate_memory.py --workspace <workspace>
- 召回上下文：python3 scripts/retrieve_memory.py --workspace <workspace> --mode stable|active|history|debug --topic <关键词>
- 审计存储：python3 scripts/audit_codex_storage.py --root ~/.codex
- 并发时优先使用 --artifact-source-dir 指向独立 artifact 目录，避免误读别的任务状态。
- 建议先运行 consolidate，再运行 retrieve 进行回读验证。
- 当 source artifact 缺失时，先补齐短期工件，再考虑写入长期记忆。
