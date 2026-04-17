#!/bin/bash

echo "=== GitHub Copilot Token Extractor ==="

# 1. Search for VS Code config
VSCODE_CONFIG="$HOME/Library/Application Support/Code/User/globalStorage/github.copilot/hosts.json"
if [ -f "$VSCODE_CONFIG" ]; then
    echo "[Found] VS Code Copilot config at $VSCODE_CONFIG"
    TOKEN=$(grep -oE '"oauth_token":"[^"]+"' "$VSCODE_CONFIG" | cut -d'"' -f4)
    if [ ! -z "$TOKEN" ]; then
        echo "Token: $TOKEN"
    fi
else
    echo "[Skip] VS Code config not found."
fi

# 2. Try GitHub CLI
if command -v gh &> /dev/null; then
    echo "[Found] GitHub CLI. Attempting to get token..."
    GH_TOKEN=$(gh auth token 2>/dev/null)
    if [ ! -z "$GH_TOKEN" ]; then
        echo "GH_TOKEN: $GH_TOKEN"
    else
        echo "[Info] 'gh auth token' returned empty. Try 'gh auth login' first."
    fi
fi

echo ""
echo "Instructions:"
echo "1. If you extracted a 'gho_' or 'ghu_' token, use it in the 'new-api' channel setup."
echo "2. For stable login, it is recommended to use the 'Device Flow' directly on the 'copilot-proxy' first."
