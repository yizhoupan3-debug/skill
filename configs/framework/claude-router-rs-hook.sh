#!/usr/bin/env bash
# Claude hook launcher wrapper — delegates to unified hook.sh with claude host_id.
exec "$(dirname "${BASH_SOURCE[0]}")/hook.sh" "claude" "$@"
