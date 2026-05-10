#!/usr/bin/env bash
# preCompact hook — shows context window stats when compaction is about to occur.
#
# PURPOSE: Token awareness. Lets the user know when context is filling up so
# they can decide whether to start a fresh session (which resets context cost
# to zero) rather than continuing in a degraded compressed context.
#
# INPUT:  { "context_usage_percent", "context_tokens", "context_window_size",
#            "message_count", "messages_to_compact", "trigger", … }
# OUTPUT: { "user_message": "…" }

input=$(cat)

usage=$(echo "$input"   | jq -r '.context_usage_percent  // "?"' 2>/dev/null)
tokens=$(echo "$input"  | jq -r '.context_tokens          // "?"' 2>/dev/null)
size=$(echo "$input"    | jq -r '.context_window_size     // "?"' 2>/dev/null)
msgs=$(echo "$input"    | jq -r '.message_count           // "?"' 2>/dev/null)
compact=$(echo "$input" | jq -r '.messages_to_compact     // "?"' 2>/dev/null)
trigger=$(echo "$input" | jq -r '.trigger                 // "auto"' 2>/dev/null)
first=$(echo "$input"   | jq -r '.is_first_compaction     // false' 2>/dev/null)

notice="⚡ Context compacting (${trigger}): ${usage}% used · ${tokens}/${size} tokens · ${msgs} messages · ${compact} being summarised."
if [[ "$first" == "true" ]]; then
    notice="$notice First compaction — earlier details may be summarised."
fi
notice="$notice Consider starting a new session if the current task scope is complete."

jq -n --arg msg "$notice" '{"user_message": $msg}'
