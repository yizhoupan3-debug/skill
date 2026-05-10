#!/usr/bin/env bash
# afterFileEdit hook — auto-formats Rust files with rustfmt after every agent edit.
#
# PURPOSE: Code quality. Keeps Rust files consistently formatted without the
# agent needing to run a separate format command or read back the file to verify.
# Silent on success — zero token cost.
#
# INPUT:  { "file_path": "...", "edits": [...] }
# OUTPUT: (none required; afterFileEdit is observe-only)

export PATH="$HOME/.cargo/bin:$PATH"
source "$HOME/.cargo/env" 2>/dev/null || true

input=$(cat)
file_path=$(echo "$input" | jq -r '.file_path // empty' 2>/dev/null)

[[ "$file_path" == *.rs ]] || exit 0
[[ -f "$file_path" ]] || exit 0

# 同步格式化，避免与紧随其后的 Read / cargo check 产生竞态
rustfmt --edition 2021 "$file_path" 2>/dev/null || true

exit 0
