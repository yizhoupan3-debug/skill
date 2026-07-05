#!/usr/bin/env bash
# Sync framework global config (~/.claude/) after skill repo updates.
#
# Strategy:
#   1. Run install-claude.sh --scope user for hooks/settings/framework.md
#   2. Merge ALL MCP servers from skill/.mcp.json into ~/.claude/mcp.json
#      with binary names resolved to absolute paths.
#
# Run manually:  ./scripts/sync-framework-global.sh
# Auto-trigger: git pull → .git/hooks/post-merge
#
# After any git pull on the skill directory, the global config is
# automatically up to date.

set -euo pipefail

FRAMEWORK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLAUDE_USER_DIR="$HOME/.claude"
MCP_FILE="$CLAUDE_USER_DIR/mcp.json"   # generated, NOT a symlink
SKILL_MCP="$FRAMEWORK_ROOT/.mcp.json"

# Ensure ~/.local/bin is in PATH (fallback, should already be set)
export PATH="$HOME/.local/bin:$HOME/.npm-global/bin:$PATH"

echo "==> Syncing Claude global framework config..."

# ---------- step 1: install-claude for hooks/settings/framework.md ----------
if [ -x "$FRAMEWORK_ROOT/scripts/install-claude.sh" ]; then
  echo "  -> Running install-claude.sh --scope user ..."
  "$FRAMEWORK_ROOT/scripts/install-claude.sh" \
    --framework-root "$FRAMEWORK_ROOT" \
    --project-root "$FRAMEWORK_ROOT" \
    --scope user \
    --skip-build \
    2>&1 | grep -v '^{' || true   # suppress JSON status, show only human lines
fi

# ---------- step 2: merge all MCP servers from skill .mcp.json ----------
# Use Python to resolve binaries and merge — handles JSON correctly
python3 << PYEOF
import json, shutil, os, sys

FRAMEWORK_ROOT = os.environ.get('FRAMEWORK_ROOT', '')
HOME = os.environ.get('HOME', '')
CLAUDE_USER_DIR = os.path.join(HOME, '.claude')
MCP_FILE = os.path.join(CLAUDE_USER_DIR, 'mcp.json')
SKILL_MCP = os.path.join(FRAMEWORK_ROOT, '.mcp.json')

# Read skill .mcp.json (full 10 servers)
with open(SKILL_MCP) as f:
    skill_cfg = json.load(f)
skill_servers = skill_cfg.get('mcpServers', {})

# Read existing generated config (if any)
existing = {}
if os.path.exists(MCP_FILE):
    with open(MCP_FILE) as f:
        existing = json.load(f).get('mcpServers', {})

# Resolve binary name to absolute path
def resolve_bin(name):
    """Resolve a short command name to absolute path, handling known patterns."""
    # Special case: npx
    if name == 'npx':
        npx_path = os.path.join(HOME, '.npm-global', 'bin', 'npx')
        if os.path.isfile(npx_path):
            return npx_path
        return shutil.which('npx') or 'npx'

    # Known install location: ~/.local/bin/
    local_bin = os.path.join(HOME, '.local', 'bin', name)
    if os.path.isfile(local_bin):
        return local_bin

    # Fallback: PATH
    resolved = shutil.which(name)
    if resolved:
        return resolved

    return name  # keep original, will fail gracefully at start

def resolve_args(args):
    """Resolve --repo-root to the skill framework regardless of CWD."""
    # Don't change the args structure itself — keep the intent.
    # --repo-root should always point to the skill framework
    # unless the server specifically needs the project root.
    return args

# Build merged server list
merged = {}
for name, srv in skill_servers.items():
    merged[name] = {
        'command': resolve_bin(srv.get('command', '')),
        'args': srv.get('args', []),
        'type': srv.get('type', 'stdio'),
        'description': srv.get('description', ''),
    }
    env = {}
    # Add framework root env var for servers that need it
    if name in ('browser-mcp', 'router-rs-framework', 'mcp-codegraph'):
        env['SKILL_FRAMEWORK_ROOT'] = FRAMEWORK_ROOT
    if env:
        merged[name]['env'] = env

# Write merged config
output = {'mcpServers': merged}

# Atomic write
tmpfile = MCP_FILE + '.tmp'
with open(tmpfile, 'w') as f:
    json.dump(output, f, indent=2)
    f.write('\n')
os.replace(tmpfile, MCP_FILE)

# Report
servers = sorted(merged.keys())
print(f"  -> Generated ~/.claude/mcp.json: {len(servers)} servers: {', '.join(servers)}")
PYEOF

# ---------- step 3: sync registered project directories ----------
PROJECT_REGISTRY="$FRAMEWORK_ROOT/configs/framework/PROJECT_REGISTRY.json"
SYNC_PROJECT="$FRAMEWORK_ROOT/scripts/sync-project.sh"
if [ -f "$PROJECT_REGISTRY" ] && [ -x "$SYNC_PROJECT" ]; then
  echo "==> Syncing registered project directories..."
  python3 << 'PYREG'
import json, subprocess, sys

FRAMEWORK_ROOT = "/Users/joe/Developer/skill"

with open(f"{FRAMEWORK_ROOT}/configs/framework/PROJECT_REGISTRY.json") as f:
    registry = json.load(f)

projs = registry.get("projects", [])
count = 0
for p in projs:
    pid = p["id"]
    path = p["path"]
    status = p.get("status", {})
    need_mcp = status.get("mcp_json", "missing") == "missing"
    need_settings = status.get("settings_json", "missing") == "missing"
    need_claude_md = status.get("claude_md_framework_ref", "missing") == "missing"

    if not need_mcp and not need_settings and not need_claude_md:
        print(f"  -> [{pid}] already synced")
        continue

    print(f"  -> [{pid}] syncing...")
    result = subprocess.run(
        [f"{FRAMEWORK_ROOT}/scripts/sync-project.sh", path],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        print(f"    !! FAILED ({pid}): {result.stderr.strip()}", file=sys.stderr)
    else:
        count += 1
        for line in result.stdout.strip().split("\n"):
            print(f"    {line}")

print(f"  -> Synced {count} project(s)")
PYREG
else
  echo "==> Skipping project sync (no PROJECT_REGISTRY.json or sync-project.sh found)"
fi

# ---------- step 4: verify binaries ----------
echo "  -> Verifying MCP binaries..."
MISSING=0
for cmd in router-rs-cli mcp-codegraph mcp-pdf mcp-ooxml mcp-pptx mcp-financial-data mcp-citation mcp-gh-source-gate; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "    ⚠️  $cmd not found in PATH"
    MISSING=1
  fi
done
if [ "$MISSING" -eq 0 ]; then
  echo "    ✅ All MCP binaries found in PATH"
fi

echo "==> Done. Framework global config + project directories are up to date."
echo "   (Restart Claude Code for changes to take effect in the skill repo)"
