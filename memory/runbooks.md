# runbooks

- 维护入口：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml framework host-integration run-memory-automation --repo-root <repo_root> --workspace <workspace>`。
- 召回记忆：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml framework memory-recall --repo-root <repo_root> --mode stable --limit <N> <关键词>`。
- 下一轮提示：`./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml framework refresh --repo-root <repo_root> --max-lines 4`。
- Skill 变更后用 `scripts/skill-compiler-rs` 重新生成路由产物并跑对应测试。
