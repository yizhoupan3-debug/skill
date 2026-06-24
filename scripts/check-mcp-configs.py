#!/usr/bin/env python3
"""Check all MCP configs have absolute --repo-root paths."""
import json, os, sys

configs = [
    (os.path.expanduser('~/.claude/mcp.json'), 'mcpServers'),
    (os.path.expanduser('~/.gemini/mcp.json'), 'mcpServers'),
    ('.mcp.json', 'mcpServers'),
    ('.gemini/mcp.json', 'mcpServers'),
    (os.path.expanduser('~/Library/Application Support/Claude-3p/claude_desktop_config.json'), 'mcpServers'),
]

errors = 0
for path, key in configs:
    try:
        d = json.load(open(path))
        for name, srv in d.get(key, {}).items():
            args = srv.get('args', [])
            if '--repo-root' in args:
                root = args[args.index('--repo-root') + 1]
                status = '✅' if root.startswith('/') else '❌'
                print(f'{status} {os.path.basename(path)}: {name} → {root}')
                if not root.startswith('/'):
                    errors += 1
    except Exception as e:
        print(f'⚠️  {path}: {e}')

sys.exit(errors)
