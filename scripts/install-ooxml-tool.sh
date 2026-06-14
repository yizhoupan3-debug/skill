#!/usr/bin/env bash
# Install the Rust `ooxml` CLI to ~/.local/bin (or OOXML_BIN_DIR).
# Delegates to install-rust-tool.sh which calls rust-release-bin.sh.
exec "$(dirname "${BASH_SOURCE[0]}")/install-rust-tool.sh" --crate ooxml_parser_rs --bin ooxml --env-prefix OOXML "$@"
