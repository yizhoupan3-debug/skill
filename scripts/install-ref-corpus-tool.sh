#!/usr/bin/env bash
# RETIRED 2026-06: ref_corpus_tool_rs archived before v6. This script is no longer maintained.
echo "ERROR: install-ref-corpus-tool.sh has been retired. ref_corpus_tool_rs was archived before v6." >&2
exit 1
# Install the Rust `ref-corpus` CLI to ~/.local/bin (or REF_CORPUS_BIN_DIR).
set -euo pipefail

FRAMEWORK_ROOT="${SKILL_FRAMEWORK_ROOT:-${FRAMEWORK_ROOT:-}}"
if [[ -z "${FRAMEWORK_ROOT}" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  FRAMEWORK_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
fi

MANIFEST="${FRAMEWORK_ROOT}/rust_tools/ref_corpus_tool_rs/Cargo.toml"
BIN_DIR="${REF_CORPUS_BIN_DIR:-${HOME}/.local/bin}"
DEST="${BIN_DIR}/ref-corpus"

if [[ ! -f "${MANIFEST}" ]]; then
  echo "error: ref_corpus_tool_rs manifest not found at ${MANIFEST}" >&2
  exit 1
fi

mkdir -p "${BIN_DIR}"
cargo build --release --manifest-path "${MANIFEST}"
install -m 755 "${FRAMEWORK_ROOT}/rust_tools/ref_corpus_tool_rs/target/release/ref-corpus" "${DEST}"

echo "Installed ref-corpus -> ${DEST}"
echo "Add to PATH if needed: export PATH=\"${BIN_DIR}:\$PATH\""
