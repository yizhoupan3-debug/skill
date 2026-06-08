---
status: retired
canonical: antigravity.md
---

# 已退役：`antigravity-app`

> **2026-06**：宿主 id **`antigravity-app`** 已合并为 canonical **`antigravity`**。闭集真源见 `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`。

请阅读 **[`antigravity.md`](antigravity.md)**。

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to antigravity --repo-root "$PWD"
```
