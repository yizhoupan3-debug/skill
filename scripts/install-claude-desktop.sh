#!/usr/bin/env bash
# RETIRED 2026-06: claude-desktop host removed from closed set. Use install-claude.sh for claude.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install-claude-desktop.sh

RETIRED (2026-06): host id claude-desktop is no longer in host_targets.supported.

Use instead:
  ./scripts/install-claude.sh

See docs/migration.md and docs/hosts/handbook.md
EOF
}

usage >&2
exit 1
