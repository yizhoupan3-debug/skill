---
name: "playwright"
description: |
  Browser automation gate for live navigation, login, form filling, screenshots, UI reproduction, and page extraction.
  In Codex, prefer local `browser-mcp`; otherwise fall back to the Playwright CLI wrapper.
routing_layer: L3
routing_owner: gate
routing_gate: evidence
session_start: required
short_description: Use a real browser when live evidence or page interaction is required
trigger_phrases:
  - 浏览器自动化
  - live browser
  - browser-mcp
  - login flow
  - UI 重现
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - playwright
allowed_tools:
  - browser
  - shell
approval_required_tools:
  - gui automation
filesystem_scope:
  - repo
  - artifacts
network_access: required
artifact_outputs:
  - screenshot.png
  - ui_verification.md
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once
---
- **Dual-Dimension Audit (Pre: Selector-Logic/Flow, Post: Trace-Fidelity/Visual-Regression Results)** → `$execution-audit-codex` [Overlay]
# Playwright CLI Skill

At conversation start or first turn, check this execution gate before abstract advice whenever the task requires a live browser session to obtain evidence.


Drive a real browser from the terminal using `playwright-cli`, but in Codex prefer the local `browser-mcp` MCP server for agent-facing interactive browsing when it is available. Prefer the bundled wrapper script so the CLI works even when it is not globally installed.
Treat this skill as MCP-first for Codex interactive work and CLI-first as the fallback path. Do not pivot to `@playwright/test` unless the user explicitly asks for test files.

## Priority routing rule

If the task requires live navigation, login, form filling, browser-side state,
UI reproduction, or extraction from an interactive page, check this skill
before static HTML inspection or abstract frontend advice.

In that case:

1. this skill owns the real-browser execution path
2. paired domain skills should build on the browser evidence it gathers
3. if local `browser-mcp` tools are available in Codex, prefer them before the CLI wrapper for interactive agent work

## Prerequisite check (required)

Before proposing commands, check whether `npx` is available (the wrapper depends on it):

```bash
command -v npx >/dev/null 2>&1
```

If it is not available, pause and ask the user to install Node.js/npm (which provides `npx`). Provide these steps verbatim:

```bash
# Verify Node/npm are installed
node --version
npm --version

# If missing, install Node.js/npm, then:
npm install -g @playwright/cli@latest
playwright-cli --help
```

Once `npx` is present, proceed with the wrapper script. A global install of `playwright-cli` is optional.

## Skill path (set once)

```bash
export CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
export PWCLI="$CODEX_HOME/skills/playwright/scripts/playwright_cli.sh"
```

User-scoped skills install under `$CODEX_HOME/skills` (default: `~/.codex/skills`).

## Quick start

Use the wrapper script:

```bash
"$PWCLI" open https://playwright.dev --headed
"$PWCLI" snapshot
"$PWCLI" click e15
"$PWCLI" type "Playwright"
"$PWCLI" press Enter
"$PWCLI" screenshot
```

If the user prefers a global install, this is also valid:

```bash
npm install -g @playwright/cli@latest
playwright-cli --help
```

## Core workflow

1. Open the page.
2. Snapshot to get stable element refs.
3. Interact using refs from the latest snapshot.
4. Re-snapshot after navigation or significant DOM changes.
5. Capture artifacts (screenshot, pdf, traces) when useful.

Minimal loop:

```bash
"$PWCLI" open https://example.com
"$PWCLI" snapshot
"$PWCLI" click e3
"$PWCLI" snapshot
```

## When to snapshot again

Snapshot again after:

- navigation
- clicking elements that change the UI substantially
- opening/closing modals or menus
- tab switches

Refs can go stale. When a command fails due to a missing ref, snapshot again.

## Recommended patterns

### Form fill and submit

```bash
"$PWCLI" open https://example.com/form
"$PWCLI" snapshot
"$PWCLI" fill e1 "user@example.com"
"$PWCLI" fill e2 "password123"
"$PWCLI" click e3
"$PWCLI" snapshot
```

### Debug a UI flow with traces

```bash
"$PWCLI" open https://example.com --headed
"$PWCLI" tracing-start
# ...interactions...
"$PWCLI" tracing-stop
```

### Multi-tab work

```bash
"$PWCLI" tab-new https://example.com
"$PWCLI" tab-list
"$PWCLI" tab-select 0
"$PWCLI" snapshot
```

### Content extraction

Use when the goal is to extract structured data from a page rather than interact with it:

```bash
"$PWCLI" open https://example.com/products
"$PWCLI" snapshot                          # get DOM tree
"$PWCLI" run-code "JSON.stringify([...document.querySelectorAll('.product')].map(el => ({name: el.querySelector('h2')?.textContent, price: el.querySelector('.price')?.textContent})))"
"$PWCLI" screenshot                        # visual verification
```

Key patterns:
- Prefer `snapshot` + `run-code` with `querySelectorAll` for structured extraction
- Use `screenshot` to visually verify extracted data matches page state
- For paginated data, loop: extract → click next → re-snapshot → extract
- For infinite scroll, use `run-code "window.scrollTo(0, document.body.scrollHeight)"` then re-snapshot
- Save extracted data to a local JSON/CSV file for downstream processing

### Handoff to `$visual-review`

When Playwright is used to gather visual evidence for another skill:

1. Capture: `"$PWCLI" screenshot` → saves to `output/playwright/`
2. Hand off: tell the downstream skill (typically `$visual-review`) to analyze the captured screenshot
3. The evidence screenshot path should be passed explicitly; do not assume screenshots are auto-inspected

Standard handoff pattern:
```bash
# 1. Gather evidence
"$PWCLI" open https://example.com
"$PWCLI" screenshot   # → output/playwright/screenshot_YYYY-MM-DD_HH-MM-SS.png
# 2. Then invoke visual-review on that screenshot
```

## Wrapper script

The wrapper script uses `npx --package @playwright/cli playwright-cli` so the CLI can run without a global install:

```bash
"$PWCLI" --help
```

Prefer the wrapper unless the repository already standardizes on a global install.

## References

Open only what you need:

- browser-mcp routing and local setup: `references/browser-mcp.md`
- CLI command reference: `references/cli.md`
- Practical workflows and troubleshooting: `references/workflows.md`

## Guardrails

- Always snapshot before referencing element ids like `e12`.
- Re-snapshot when refs seem stale.
- Prefer explicit commands over `eval` and `run-code` unless needed.
- When you do not have a fresh snapshot, use placeholder refs like `eX` and say why; do not bypass refs with `run-code`.
- Use `--headed` when a visual check will help.
- When capturing artifacts in this repo, use `output/playwright/` and avoid introducing new top-level artifact folders.
- Default to CLI commands and workflows, not Playwright test specs.
- **Superior Quality Audit**: For mission-critical browser automation, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## When to use

- The user wants browser automation, E2E testing, or web scraping with Playwright
- The task involves browser testing, page interaction, or automated screenshots
- The user says "Playwright", "E2E 测试", "浏览器自动化", "headless browser"
- "强制进行 Playwright 深度审计 / 检查选择器稳定性与视觉回归结果。"
- "Use $execution-audit-codex to audit this browser script for trace-fidelity idealism."
- The user wants to automate web interactions programmatically

## Do not use

- The task is system-level desktop screenshot → use `$screenshot`
- The task is visual review of existing images → use `$visual-review`
- The task is manual testing strategy → use `$test-engineering`
- The task is web data extraction strategy (pagination, rate limiting, anti-bot) → use `$web-scraping` (which may overlay `$playwright` for dynamic pages)
