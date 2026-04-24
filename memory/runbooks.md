# runbooks

- 维护入口：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --host-integration run-memory-automation --repo-root <repo_root> --workspace <workspace>`。
- 召回记忆：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --framework-memory-recall-json --repo-root <repo_root> --framework-memory-mode stable --query <关键词> --limit <N>`。
- 收口记忆：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --claude-hook-command session-end --repo-root <repo_root> --claude-hook-max-lines 4`。
