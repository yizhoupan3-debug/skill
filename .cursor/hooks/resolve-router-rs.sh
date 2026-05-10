#!/usr/bin/env bash
# 解析可执行的 router-rs 路径。仓库根 `.cargo/config.toml` 可能把 target-dir 指到全局目录，
# 此时 `scripts/router-rs/target/*` 会是陈旧占位；须优先用 cargo metadata 的 target_directory。
#
# Usage: resolve-router-rs.sh <repo_root>
# Prints absolute path to router-rs, or empty line if not found.

set -euo pipefail

root="${1:?resolve-router-rs.sh: repo_root required}"

try_paths() {
  local c
  for c in "$@"; do
    [[ -n "$c" ]] || continue
    if [[ -x "$c" ]]; then
      printf '%s' "$c"
      return 0
    fi
  done
  return 1
}

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  try_paths \
    "${CARGO_TARGET_DIR%/}/release/router-rs" \
    "${CARGO_TARGET_DIR%/}/debug/router-rs" && exit 0
fi

meta_dir=""
if command -v cargo &>/dev/null && [[ -f "$root/scripts/router-rs/Cargo.toml" ]]; then
  _meta_json="$(cd "$root/scripts/router-rs" && cargo metadata --format-version 1 --no-deps 2>/dev/null || true)"
  if [[ -n "$_meta_json" ]]; then
    if command -v jq &>/dev/null; then
      meta_dir="$(printf '%s' "$_meta_json" | jq -r '.target_directory // empty' 2>/dev/null || true)"
    elif command -v python3 &>/dev/null; then
      meta_dir="$(printf '%s' "$_meta_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("target_directory") or "")' 2>/dev/null || true)"
    fi
  fi
fi
if [[ -n "$meta_dir" && "$meta_dir" != "null" ]]; then
  try_paths \
    "${meta_dir%/}/release/router-rs" \
    "${meta_dir%/}/debug/router-rs" && exit 0
fi

try_paths \
  "$root/scripts/router-rs/target/release/router-rs" \
  "$root/scripts/router-rs/target/debug/router-rs" \
  "$root/target/release/router-rs" \
  "$root/target/debug/router-rs" && exit 0

command -v router-rs 2>/dev/null || printf ''
