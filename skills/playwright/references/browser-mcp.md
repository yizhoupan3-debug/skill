# browser-mcp integration

When Codex has a local `browser-mcp` MCP server configured, prefer it for agent-facing browser work:

- use `browser_get_state` instead of dumping full DOM or repeated screenshots
- use `browser_get_elements` to obtain stable refs before acting
- use `browser_click`, `browser_fill`, `browser_press`, and `browser_wait_for` for stepwise execution
- use `browser_get_network` for API-first inspection
- use `browser_screenshot` only when visual evidence is required

Keep the existing Playwright CLI workflow as the fallback path when:

- the MCP server is unavailable
- the user explicitly asks for CLI commands or a Playwright script
- the task is about authoring test code rather than interactive agent tool use

Local setup in this repo:

```bash
python3 /Users/joe/Documents/skill/scripts/install_browser_mcp_codex.py
```

Then restart Codex so it reloads MCP servers from `~/.codex/config.toml`.
