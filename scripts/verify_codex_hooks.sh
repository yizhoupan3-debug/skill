#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="${1:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
ROUTER_RS_BIN=""

echo "Verifying Codex hook projection"
echo

if [ -x "$REPO_ROOT/scripts/router-rs/target/release/router-rs" ]; then
    ROUTER_RS_BIN="$REPO_ROOT/scripts/router-rs/target/release/router-rs"
elif [ -x "$REPO_ROOT/scripts/router-rs/target/debug/router-rs" ]; then
    ROUTER_RS_BIN="$REPO_ROOT/scripts/router-rs/target/debug/router-rs"
elif [ -x "$REPO_ROOT/target/release/router-rs" ]; then
    ROUTER_RS_BIN="$REPO_ROOT/target/release/router-rs"
elif [ -x "$REPO_ROOT/target/debug/router-rs" ]; then
    ROUTER_RS_BIN="$REPO_ROOT/target/debug/router-rs"
else
    ROUTER_RS_BIN="$(command -v router-rs 2>/dev/null || true)"
fi

if [ -z "$ROUTER_RS_BIN" ] || [ ! -x "$ROUTER_RS_BIN" ]; then
    echo "router-rs binary not found"
    echo "Build it with: cargo build --release --manifest-path scripts/router-rs/Cargo.toml"
    exit 1
fi

for file in .codex/config.toml .codex/hooks.json .codex/README.md AGENTS.md; do
    test -f "$REPO_ROOT/$file" || {
        echo "Missing $file"
        exit 1
    }
done

grep -q "hooks = true" "$REPO_ROOT/.codex/config.toml" || {
    echo ".codex/config.toml must enable hooks"
    exit 1
}

if grep -q "codex_hooks" "$REPO_ROOT/.codex/config.toml"; then
    echo ".codex/config.toml must not use deprecated codex_hooks"
    exit 1
fi

for event in SessionStart PreToolUse UserPromptSubmit PostToolUse Stop; do
    grep -q "\"$event\"" "$REPO_ROOT/.codex/hooks.json" || {
        echo "Missing Codex hook event: $event"
        exit 1
    }
done

if grep -q "scripts/codex_hook_entrypoint.sh" "$REPO_ROOT/.codex/hooks.json"; then
    echo ".codex/hooks.json must call router-rs codex hook directly"
    exit 1
fi

if grep -q "sessionEnd\\|Kiro" "$REPO_ROOT/.codex/README.md" "$REPO_ROOT/.codex/hooks.json"; then
    echo "Codex hook projection contains stale lifecycle or host wording"
    exit 1
fi

echo '{"hook_event_name":"SessionStart","session_id":"verify-session","source":"startup"}' |
    "$ROUTER_RS_BIN" codex hook --event=SessionStart --repo-root "$REPO_ROOT" >/dev/null

echo '{"hook_event_name":"PreToolUse","session_id":"verify-session","tool_name":"functions.exec_command","tool_input":{"cmd":"true"}}' |
    "$ROUTER_RS_BIN" codex hook --event=PreToolUse --repo-root "$REPO_ROOT" >/dev/null

echo '{"hook_event_name":"UserPromptSubmit","session_id":"verify-session","prompt":"review this PR"}' |
    "$ROUTER_RS_BIN" codex hook --event=UserPromptSubmit --repo-root "$REPO_ROOT" >/dev/null

echo '{"hook_event_name":"PostToolUse","session_id":"verify-session","tool_name":"functions.spawn_agent","tool_input":{"agent_type":"explorer"}}' |
    "$ROUTER_RS_BIN" codex hook --event=PostToolUse --repo-root "$REPO_ROOT" >/dev/null

echo '{"hook_event_name":"Stop","session_id":"verify-session","prompt":"review this PR","stop_hook_active":true}' |
    "$ROUTER_RS_BIN" codex hook --event=Stop --repo-root "$REPO_ROOT" >/dev/null

echo "Codex hook projection verified"
