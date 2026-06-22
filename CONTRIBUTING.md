# Contributing to Skill Framework

> 完整文档索引见 [`docs/README.md`](docs/README.md)（含文档地图、已合并文件记录）。

## 角色阅读路径

| 角色 | 推荐阅读顺序 |
|------|-------------|
| **框架开发者** | [`docs/README.md`](docs/README.md) → [`docs/spec.md`](docs/spec.md)（架构） → [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md)（宿主） → [`AGENTS.md`](AGENTS.md)（策略） |
| **Skill 作者** | [`README.md`](README.md) §系统包含内容 → [`docs/spec.md`](docs/spec.md) §路由 → `skills/<skill-name>/SKILL.md` 模板 |
| **普通用户** | [`README.md`](README.md) → [`docs/operations/index.md`](docs/operations/index.md)（安装/升级） → [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md)（对应宿主节） |
| **宿主实现者** | [`docs/hosts/_common.md`](docs/hosts/_common.md) → [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md) → [`docs/hosts/opencode.md`](docs/hosts/opencode.md) |

## 开发环境

- **Rust**: stable (edition 2024, rust-version 1.85)
- **Python**: 3.12 (uv-only, 禁止 pip)
- **OS**: macOS / Linux / Windows (CI 覆盖 Ubuntu + macOS)

## Crate 目录导览

| Crate | 路径 | 职责 |
|-------|------|------|
| **router-rs** | `core/router-rs/` | CLI 入口，宿主 hook 分发、MCP server 管理 |
| **runtime-core** | `core/runtime-core/` | 运行层编排、session/context/loop 管理 |
| **host-projection** | `core/host-projection/` | 宿主适配层（Claude/Cursor/Codex/OpenCode hook 分发） |
| **core-policy** | `core/core-policy/` | 跨宿主策略（hook 公共逻辑、env flags） |
| **routing-engine** | `core/routing-engine/` | 路由引擎：意图匹配、Skill/Tool 路由 |
| **framework-kernel** | `core/framework-kernel/` | Skill 加载、RUNTIME_REGISTRY 解析 |
| **research-harness** | `core/research-harness/` | 科研日志、知识图谱、Barrier 系统 |
| **codegraph-rs** | `tools/codegraph-rs/` | 代码图谱索引工具（MCP 八工具） |
| **evolution-rs** | `tools/evolution-rs/` | 路由日志演化分析（零 crate 依赖） |

依赖方向：`router-rs` → `runtime-core` → `host-projection`, `core-policy`, `routing-engine`, `framework-kernel`。B0 crates（`core-policy`/`routing-engine`/`framework-kernel`）不依赖 `router-rs`。

## 快速开始

```bash
# 克隆仓库
git clone <repo-url> && cd skill

# 构建（默认 profile，不含 codegraph 特性）
cargo build --release --manifest-path core/router-rs/Cargo.toml

# 运行全量测试（常用三 crate）
cargo test -p router-rs -p codegraph-rs -p evolution-rs

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
- 测试分层：单元测试（`#[cfg(test)]`） → 集成测试（`tests/`） → 合约测试（`smoke_*`）

## 文档

- 公共 API 必须有文档注释
- 完整文档体系见 [`docs/README.md`](docs/README.md)（含文档地图）
- 架构决策记录在 [`docs/adr/`](docs/adr/)
- 宿主差异文档在 [`docs/hosts/`](docs/hosts/)
- Skill 开发指南见 [`README.md`](README.md) §框架路径与路由

## Pull Request

1. 确保 CI 全绿
2. PR 描述说明变更内容和动机
3. 大型变更请先开 Issue 讨论

## License

贡献即表示你同意你的代码在 MIT License 下发布。
