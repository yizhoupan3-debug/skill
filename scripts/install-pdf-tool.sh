#!/usr/bin/env bash
# Install the Rust `pdf` CLI to ~/.local/bin (or PDF_BIN_DIR).
exec "$(dirname "${BASH_SOURCE[0]}")/install-rust-tool.sh" --crate pdf_tool_rs --bin pdf --env-prefix PDF "$@"
