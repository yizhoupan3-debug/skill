---
description: Generate and copy the next-turn execution prompt with the Rust refresh command.
allowed-tools: Bash(cargo run --quiet --manifest-path */scripts/router-rs/Cargo.toml -- *)
---

Run:

`cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --framework-refresh-json`

Then reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`
