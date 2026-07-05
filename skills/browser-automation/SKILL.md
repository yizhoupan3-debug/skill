---
allowed_tools:
- shell
- browser
approval_required_tools: []
description: 'Browser automation: page navigation, click, fill, screenshot, network monitoring, session management.'
metadata:
  platforms:
  - supported
  tags:
  - browser
  - automation
  - screenshot
  - web
  - scraping
  version: '1.0.0'
name: browser-automation
scene: general
network_access: required
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P2
session_start: n/a
source: local
risk: low
trigger_hints:
- browser
- automation
- screenshot
- 浏览器
- 截图
- 截屏
- 打开网页
- 点击
- 表单填写
- 输入
- 网络请求
- network
- 页面状态
- 页面元素
- 页面文本
- 浏览器会话
- 等待
- 自动填充
- 爬虫
- browser session
- 浏览器持久化
- save session
- restore session
- 浏览器诊断
- browser tabs
- 标签页
- 页面跳转
- 网页抓取
- web scrape
- 网络监听
- 网页操作
short_description: 浏览器自动化操作：打开页面、点击、填表、截图、网络监听等
tags:
- browser
- automation
- web-scraping
- screenshot
when_to_use: 用户需要操作浏览器（打开网页、点击、填表单）、截图、检查网络请求、管理浏览器会话时
do_not_use: 用户需要视觉审查截图内容（走 visual-review）；用户需要将截图转为 TikZ 配图（走 tikz-paper-figure）
---
# Browser Automation

## Persona

Act as a **browser automation specialist** with expertise in controlling headless browsers for web interaction, data extraction, screenshot capture, state inspection, and session management.

## When to use

- User needs to **navigate, click, fill forms, press keys** in a browser
- User needs a **screenshot** of a page or element
- User needs to **inspect page state, DOM elements, visible text, or network requests**
- User needs to **wait for page conditions** (text appears, element appears, URL changes, network idle)
- User needs to **save/restore browser sessions** for persistence across tasks
- User needs to **diagnose browser health** or check runtime events

## How to use

This skill provides access to all browser-mcp tools. Decide which tool to use based on the user's request:

| Task | Tool |
|------|------|
| Open a URL | `browser_open` |
| Click an element | `browser_click` (by index ref) |
| Fill a form field | `browser_fill` |
| Press a keyboard key | `browser_press` |
| Take screenshot | `browser_screenshot` |
| Get page text | `browser_get_text` |
| Get page state/elements | `browser_get_state` / `browser_get_elements` |
| Inspect network requests | `browser_get_network` |
| Wait for condition | `browser_wait_for` |
| Manage browser tabs | `browser_tabs` / `browser_close` |
| Save/restore session | `browser_save_session` / `browser_restore_session` |
| Diagnostics | `browser_diagnostics` |

## Do not use

- Task is about visual review of an already-captured screenshot → use `visual-review`
- Task needs to convert screenshots to TikZ figures → use `tikz-paper-figure`
- Task only needs web search results (no browser interaction) → use `deep-search` or `research-discovery`
