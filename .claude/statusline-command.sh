#!/usr/bin/env bash
# Claude Code statusline — mirrors the user's zsh prompt style
# All output in English. Reads JSON from stdin.

input=$(cat)

# Single jq invocation to extract all fields at once (avoids 3 separate process spawns)
read -r model effort used < <(printf '%s' "$input" | jq -r '[.model.display_name // .model.id // "unknown", .effort.level // "", .context_window.used_percentage // ""] | @tsv')

# Build left segment: Model name with optional effort level
if [ -n "$effort" ]; then
  left="${model}[${effort}]"
else
  left="${model}"
fi

# Append context usage when available (English)
if [ -n "$used" ]; then
  printf '%s | ctx %s%% used' "$left" "$used"
else
  printf '%s' "$left"
fi