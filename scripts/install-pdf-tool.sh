#!/usr/bin/env bash
# Install the Rust `pdf` CLI to ~/.local/bin (or PDF_BIN_DIR).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRAMEWORK_ROOT="${SKILL_FRAMEWORK_ROOT:-${FRAMEWORK_ROOT:-}}"
if [[ -z "${FRAMEWORK_ROOT}" ]]; then
  FRAMEWORK_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
fi

MANIFEST="${FRAMEWORK_ROOT}/rust_tools/pdf_tool_rs/Cargo.toml"
BIN_DIR="${PDF_BIN_DIR:-${HOME}/.local/bin}"
DEST="${BIN_DIR}/pdf"

if [[ ! -f "${MANIFEST}" ]]; then
  echo "error: pdf_tool_rs manifest not found at ${MANIFEST}" >&2
  exit 1
fi

mkdir -p "${BIN_DIR}"
cargo build --release --manifest-path "${MANIFEST}"
SRC="$("${SCRIPT_DIR}/rust-release-bin.sh" "${MANIFEST}" pdf)"
if [[ "$(cd "$(dirname "${SRC}")" && pwd -P)/$(basename "${SRC}")" == "$(cd "${BIN_DIR}" 2>/dev/null && pwd -P)/$(basename "${DEST}")" ]] \
  && [[ -e "${DEST}" ]]; then
  echo "Already linked or installed: ${DEST}"
else
  install -m 755 "${SRC}" "${DEST}"
fi

echo "Installed pdf -> ${DEST}"
echo "Add to PATH if needed: export PATH=\"${BIN_DIR}:\$PATH\""
