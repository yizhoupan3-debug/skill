#!/usr/bin/env bash
# Resolve release binary path for a workspace member manifest (honors .cargo/config.toml target-dir).
# Usage: rust-release-bin.sh <Cargo.toml> <bin-name>
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: rust-release-bin.sh <manifest> <bin-name>" >&2
  exit 2
fi

MANIFEST="$1"
BIN_NAME="$2"

if [[ ! -f "${MANIFEST}" ]]; then
  echo "error: manifest not found: ${MANIFEST}" >&2
  exit 1
fi

TARGET_DIR="$(
  cargo metadata --manifest-path "${MANIFEST}" --format-version 1 --no-deps \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])'
)"
BIN_PATH="${TARGET_DIR}/release/${BIN_NAME}"

if [[ ! -f "${BIN_PATH}" ]]; then
  echo "error: release binary not found after build: ${BIN_PATH}" >&2
  exit 1
fi

printf '%s\n' "${BIN_PATH}"
