---
status: retired
retired: "2026-06"
canonical: codex.md
---

# 已退役：`codex-cli`

> **2026-06**：宿主 id **`codex-cli`** 已合并为 canonical **`codex`**。闭集真源见 `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`。

请阅读 **[`codex.md`](codex.md)**（Codex 宿主操作手册）。安装与同步示例：

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to codex --repo-root "$PWD"
cargo run --release --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"
```
