#!/bin/bash
# Claude statusLine script
# Colors: 141=purple (dir/git), 183=green (success), 177=red (fail/high-ctx)
#         75=cyan (model), 215=orange (effort), 243=gray (dimmed text)
set -eu

input=$(cat)

# ── Directory (show ~ instead of full home path) ──────────────────────────────
dir=$(echo "$input" | jq -r '.workspace.current_dir // empty')
if [[ -n "$dir" ]]; then
  home="${HOME}"
  if [[ "$dir" == "$home"* ]]; then
    dir="~${dir#"$home"}"
  fi
fi

# ── Exit status indicator (green for success, red for failure) ─────────────────
last_exit="${CLAUDE_LAST_EXIT_STATUS:-0}"
if [[ "$last_exit" == "0" ]]; then
  arrow=$'\033[38;5;183m❯\033[0m'
else
  arrow=$'\033[38;5;177m❯\033[0m'
fi

# ── Git branch ────────────────────────────────────────────────────────────────
git_branch=""
_target_dir="$(echo "$input" | jq -r '.workspace.current_dir // empty')"
if [[ -n "$_target_dir" && -d "$_target_dir" ]]; then
  cd "$_target_dir"
fi
git_branch=$(git -c safe.directory='*' branch --show-current 2>/dev/null)

# ── Model name ────────────────────────────────────────────────────────────────
model=$(echo "$input" | jq -r '.model.display_name // empty')

# ── Reasoning effort level ────────────────────────────────────────────────────
effort=$(echo "$input" | jq -r '.effort.level // empty')

# ── Context used percentage ───────────────────────────────────────────────────
used=$(echo "$input" | jq -r '.context_window.used_percentage // empty')

# ── Build output ──────────────────────────────────────────────────────────────
sep=$'\033[38;5;243m·\033[0m'
out=""

# dir + arrow
[[ -n "$dir" ]] && out=$'\033[38;5;141m'"$dir"$'\033[0m'" $arrow"

# git branch
[[ -n "$git_branch" ]] && out="$out"$'\033[38;5;141m'" $git_branch"$'\033[0m'

# model name (cyan)
if [[ -n "$model" ]]; then
  out="$out $sep "$'\033[38;5;75m'"$model"$'\033[0m'
fi

# reasoning effort (orange, only when not default "medium")
if [[ -n "$effort" && "$effort" != "medium" ]]; then
  out="$out $sep "$'\033[38;5;215m'"$effort"$'\033[0m'
fi

# context usage (color-coded: green <75%, orange 75-89%, red >=90%)
if [[ -n "$used" ]]; then
  used_int=$(printf '%.0f' "$used")
  if [[ "$used_int" -ge 90 ]]; then
    ctx_color='38;5;177'   # red
  elif [[ "$used_int" -ge 75 ]]; then
    ctx_color='38;5;215'   # orange
  else
    ctx_color='38;5;150'   # green
  fi
  out="$out $sep "$'\033['"${ctx_color}"'m'"ctx:${used_int}%"$'\033[0m'
fi

echo "$out"
