#!/usr/bin/env bash
# Generic installer for Rust MCP tool binaries.
# Usage: install-rust-tool.sh --crate <dir> --bin <name> [--env-prefix <PREFIX>]
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install-rust-tool.sh --crate <dir> --bin <name> [--env-prefix <PREFIX>]

Install a Rust binary from the rust_tools/ workspace to ~/.local/bin.

Options:
  --crate <dir>       Crate directory name under rust_tools/
  --bin <name>        Binary name to install
  --env-prefix <PREFIX>  Env prefix for custom BIN_DIR (e.g. PDF → PDF_BIN_DIR)
EOF
  exit 1
}

if ! command -v cargo &>/dev/null; then
  echo "error: cargo not found — install Rust first: https://rustup.rs" >&2
  exit 1
fi

CRATE="" BIN="" ENV_PREFIX=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --crate)     CRATE="$2"; shift 2 ;;
    --bin)       BIN="$2"; shift 2 ;;
    --env-prefix) ENV_PREFIX="$2"; shift 2 ;;
    -h|--help)   usage ;;
    *) echo "Unknown arg: $1" >&2; usage ;;
  esac
done

if [[ -z "${CRATE}" || -z "${BIN}" ]]; then
  usage
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRAMEWORK_ROOT="${SKILL_FRAMEWORK_ROOT:-${FRAMEWORK_ROOT:-}}"
if [[ -z "${FRAMEWORK_ROOT}" ]]; then
  FRAMEWORK_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
fi

MANIFEST="${FRAMEWORK_ROOT}/rust_tools/${CRATE}/Cargo.toml"

# Resolve BIN_DIR from env var or default
if [[ -n "${ENV_PREFIX}" ]]; then
  _ENV_VAR="${ENV_PREFIX}_BIN_DIR"
  BIN_DIR="${!_ENV_VAR:-${HOME}/.local/bin}"
else
  BIN_DIR="${HOME}/.local/bin"
fi

DEST="${BIN_DIR}/${BIN}"

if [[ ! -f "${MANIFEST}" ]]; then
  echo "error: ${CRATE} manifest not found at ${MANIFEST}" >&2
  exit 1
fi

mkdir -p "${BIN_DIR}"
cargo build --release --manifest-path "${MANIFEST}"
SRC="$("${SCRIPT_DIR}/rust-release-bin.sh" "${MANIFEST}" "${BIN}")"
if [[ "$(cd "$(dirname "${SRC}")" && pwd -P)/$(basename "${SRC}")" == "$(cd "${BIN_DIR}" 2>/dev/null && pwd -P)/$(basename "${DEST}")" ]] \
  && [[ -e "${DEST}" ]]; then
  echo "Already linked or installed: ${DEST}"
else
  install -m 755 "${SRC}" "${DEST}"
fi

echo "Installed ${BIN} -> ${DEST}"
echo "Add to PATH if needed: export PATH=\"${BIN_DIR}:\$PATH\""
