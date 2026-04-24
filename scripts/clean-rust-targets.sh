#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

find "$repo_root" \
  -path "$repo_root/.git" -prune -o \
  -type d -name target -prune -print0 |
  xargs -0 rm -rf

