# Contributing to Skill Framework

感谢你对 Skill Framework 的兴趣！以下是贡献指南。

## 开发环境

- **Rust**: stable (edition 2024, rust-version 1.85)
- **Python**: 3.12 (uv-only, 禁止 pip)
- **OS**: macOS / Linux / Windows (CI 覆盖 Ubuntu + macOS)

## 快速开始

```bash
# 克隆仓库
git clone <repo-url> && cd skill

# 构建
cargo build --release --manifest-path core/router-rs/Cargo.toml

# 运行全量测试
cargo test --workspace

# 格式化 + lint
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## 分支与提交

- 从 `main` 创建功能分支
- 提交信息使用英文，格式：`<type>: <description>`（type: feat/fix/refactor/docs/test/ci）
- 提交前确保：`cargo fmt` + `cargo clippy -- -D warnings` + `cargo test` 全通过

## 代码规范

- **Rust**: 遵循 clippy.toml 配置（too-many-arguments=8, cognitive-complexity=30）
- **错误处理**: 核心 crate 使用 `FrameworkError`（thiserror），工具 crate 可使用 anyhow
- **异步测试**: 优先使用 `#[tokio::test]`
- **热路径优化**: Regex 使用 `OnceLock`/`LazyLock` 缓存，避免裸 `Regex::new`

## 测试要求

- 新功能必须附带测试
- CI 运行全量 workspace 测试 + feature matrix（default/codegraph/host-*）
- 测试文件 ≤2000 行，超过请拆分

## 文档

- 公共 API 必须有文档注释
- 架构决策记录在 `docs/adr/`
- 宿主差异文档在 `docs/hosts/`

## Pull Request

1. 确保 CI 全绿
2. PR 描述说明变更内容和动机
3. 大型变更请先开 Issue 讨论

## License

贡献即表示你同意你的代码在 MIT License 下发布。
