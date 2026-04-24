# runbooks

- 维护入口：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --host-integration run-memory-automation --repo-root <repo_root> --workspace <workspace>`。
- 召回记忆：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --framework-memory-recall-json --repo-root <repo_root> --framework-memory-mode stable --query <关键词> --limit <N>`。
- 收口记忆：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --codex-hook-command session-end --repo-root <repo_root> --framework-max-lines 4`。
- Skill 变更后用 `scripts/skill-compiler-rs` 重新生成路由产物并跑对应测试。
