---
description: Build the next-turn execution prompt, copy it to the clipboard, and reply with one fixed sentence.
allowed-tools: Bash(cargo run --quiet --manifest-path */scripts/router-rs/Cargo.toml -- *), Bash(*/scripts/router-rs/target/debug/router-rs *)
---

If `scripts/router-rs/Cargo.toml` exists in the current repository, run:

`cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --framework-recap-json`

Then copy `recap.workflow_prompt` to the macOS clipboard yourself, and reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`
