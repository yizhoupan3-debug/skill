---
name: "screenshot"
description: |
  Capture desktop or system screenshots including full screen, a specific app
  window, or a pixel region. Use when the user explicitly asks for 截图、全屏截图、
  窗口截图、区域截图, or when tool-specific capture is unavailable and an OS-level
  screenshot is needed for debugging, visual review, bug reports, or
  documentation.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 截图
  - 全屏截图
  - 窗口截图
  - 区域截图
  - visual review
  - bug reports
  - documentation
  - screenshot
runtime_requirements:
  commands:
    - osascript
    - screencapture
    - swift
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - screenshot
---

# Screenshot Capture

Follow these save-location rules every time:

1) If the user specifies a path, save there.
2) If the user asks for a screenshot without a path, save to the OS default screenshot location.
3) If Codex needs a screenshot for its own inspection, save to the temp directory.

## Tool priority

- Prefer tool-specific screenshot capabilities when available (for example: a Figma MCP/skill for Figma files, or Playwright/agent-browser tools for browsers and Electron apps).
- Use this skill when explicitly asked, for whole-system desktop captures, or when a tool-specific capture cannot get what you need.
- Otherwise, treat this skill as the default for desktop apps without a better-integrated capture tool.

## macOS permission preflight

```bash
bash <path-to-skill>/scripts/ensure_macos_permissions.sh
```

## Quick capture

```bash
# macOS: preflight + app capture to temp
bash <path-to-skill>/scripts/ensure_macos_permissions.sh && \
python3 <path-to-skill>/scripts/take_screenshot.py --app "<App>" --mode temp
```

> Full OS-specific commands (macOS/Linux/Windows): [references/os_commands.md](references/os_commands.md)

The script prints one path per capture. When multiple windows/displays match, it prints multiple paths.

## Error handling

- Run `ensure_macos_permissions.sh` first to request Screen Recording permission.
- If app/window capture returns no matches, run `--list-windows --app "AppName"` and retry with `--window-id`.
- Always report the saved file path in the response.

## When to use

- The user explicitly asks for a desktop, window, or region screenshot
- The user says "截图", "全屏截图", "窗口截图", "区域截图", "系统截图"
- Tool-specific screenshot capture is unavailable and a system-level capture is needed

## Do not use

- Analyzing or reviewing an existing screenshot → use `$visual-review`
- Browser automation to capture web page content → use `$playwright`
- Generating or editing images with AI → use `$imagegen`
- Web page screenshots needing navigation/login/interaction → use `$playwright`
