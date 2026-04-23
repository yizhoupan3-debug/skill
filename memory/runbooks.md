# runbooks

## 标准操作

- 统一维护入口：cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --host-integration run-memory-automation --repo-root <repo_root> --workspace <workspace>
- 需要迁移旧 artifact 布局时显式执行：cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --host-integration run-memory-automation --repo-root <repo_root> --workspace <workspace> --apply-artifact-migrations
- 合并记忆：cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --claude-hook-command session-end --repo-root <repo_root> --claude-hook-max-lines 4
- 召回上下文：cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --framework-memory-recall-json --repo-root <repo_root> --framework-memory-mode stable|active|history|debug --query <关键词> --limit <N>
- 生命周期收口：./scripts/router-rs/target/release/router-rs --claude-hook-command session-end --repo-root <repo_root> --claude-hook-max-lines 4
- 审计存储：查看 `artifacts/ops/memory_automation/<run_id>/storage_audit.json`
- 并发时优先使用 --artifact-source-dir 指向独立 artifact 目录，避免误读别的任务状态。
- 建议先运行 consolidate，再运行 retrieve 进行回读验证。
- 当 source artifact 缺失时，先补齐短期工件，再考虑写入长期记忆。
