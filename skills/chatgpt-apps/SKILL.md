---
name: chatgpt-apps
description: |
  Build, scaffold, refactor, and troubleshoot ChatGPT Apps SDK applications
  that combine an MCP server and widget UI. Use when designing tools,
  registering resources, wiring the MCP Apps bridge, handling `window.openai`
  compatibility, applying Apps SDK metadata, CSP, and domain settings, or
  producing a docs-aligned app scaffold. Prefer a docs-first workflow via the
  `openai-docs` skill or official OpenAI developer docs MCP tools.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - ChatGPT Apps
  - Apps SDK
  - MCP server
  - widget UI
  - window.openai
  - chatgpt
  - apps
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - chatgpt
    - apps
---

# ChatGPT Apps

## Overview

Scaffold ChatGPT Apps SDK implementations with a docs-first, example-first workflow, then generate code that follows current Apps SDK and MCP Apps bridge patterns.

Use this skill to produce:

- A primary app-archetype classification and repo-shape decision
- A tool plan (names, schemas, annotations, outputs)
- An upstream starting-point recommendation (official example, ext-apps example, or local fallback scaffold)
- An MCP server scaffold (resource registration, tool handlers, metadata)
- A widget scaffold (MCP Apps bridge first, `window.openai` compatibility/extensions second)
- A reusable Node + `@modelcontextprotocol/ext-apps` starter scaffold for low-dependency fallbacks
- A validation report against the minimum working repo contract
- Local dev and connector setup steps
- A short stakeholder summary of what the app does (when requested)

## When to use

- The user wants to build or customize a ChatGPT Custom GPT, GPT Action, or plugin
- The task involves OpenAI GPT Builder, actions schema (OpenAPI), or GPT instructions
- The user says "做一个 GPT", "Custom GPT", "GPT Action", "GPT 插件"
- The user wants to configure a ChatGPT app with specific behavior, tools, or knowledge

## Do not use

- The task is building a standalone MCP server → use `$mcp-builder`
- The task is general prompt engineering not specific to ChatGPT GPTs → use `$prompt-engineer`
- The task is building a web app that calls OpenAI API → use appropriate framework skill
