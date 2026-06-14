#!/usr/bin/env bash
# Install the Rust `ppt` CLI to ~/.local/bin (or PPT_BIN_DIR).
# Binary installed as /ppt via install-rust-tool.sh.
exec "$(dirname "${BASH_SOURCE[0]}")/install-rust-tool.sh" --crate pptx_tool_rs --bin ppt --env-prefix PPT "$@"
