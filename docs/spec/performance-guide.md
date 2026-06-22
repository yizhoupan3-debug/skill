---
parent: docs/spec.md
version: unified-v7
---

## 20. 性能指南

### 20.1 热点摘要

| 热点 | 文件 | 问题 | 严重度 |
|------|------|------|--------|
| Snapshot clone | `framework_profile.rs` | 65 clone/1692 lines | P0 |
| Error alloc | `execution_contract.rs` | 166 alloc/1121 lines | P0 |
| Regex compile | `hook_common.rs` | ~13 static Regex | P0 (已解决) |
| Clone density | `status.rs` | 40 clone/265 lines = 15% | P1 |
| Alloc density | `routing.rs` | 122 alloc/1113 lines | P1 |
| Serde calls | `stdio_dispatch.rs` | 21 calls | P1 |

### 20.2 Regex 缓存模式

所有 static regex 必须使用 `OnceLock<Regex>`：

```rust
static RE: OnceLock<Regex> = OnceLock::new();
RE.get_or_init(|| Regex::new(r"...").expect("..."))
```

### 20.3 克隆减少策略

1. 函数签名优先用 `&str` 而非 `String`。
2. 共享不可变数据使用 `Arc<T>`。
3. 条件所有权字符串使用 `Cow<'_, str>`。
4. 避免 `.clone()` 链 — 重构为引用传递。

### 20.4 编译配置

- Development: `cargo build`（debug，快速迭代）
- Release: `cargo build --release`（优化，用于部署）
- Target dir: `/tmp/skill-cargo-target/`（通过 `.cargo/config.toml`）
