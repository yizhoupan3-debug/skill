---
name: claude-api
description: |
  Use official Anthropic and Claude documentation to guide Claude API,
  Claude Code, SDK, model migration, feature tuning, hooks, memory, MCP, and
  release-note questions. Prefer current Claude docs and release notes before
  guessing.
routing_layer: L1
routing_owner: gate
routing_gate: source
routing_priority: P1
session_start: required
short_description: Use official Claude docs first for Claude API and Claude Code
trigger_hints:
  - Claude API
  - Claude Code
  - Anthropic SDK
  - claude-api
  - release notes
  - hooks
  - memory
  - MCP
metadata:
  version: "1.0.0"
  platforms: [codex, claude-code]
  tags:
    - anthropic
    - claude
    - claude-code
    - api
    - documentation
    - release-notes
allowed_tools:
  - browser
approval_required_tools:
  - web search fallback
filesystem_scope:
  - repo
network_access: conditional
artifact_outputs:
  - EVIDENCE_INDEX.json
---

# Claude API

This shared skill gives Claude and Codex one common entry for official
Anthropic guidance. Use it for both API/SDK work and Claude Code tuning.

## When to use

- The user asks how to configure or tune Claude Code better
- The user asks for Claude API / Anthropic SDK implementation guidance
- The user asks about Claude model migration, prompt caching, thinking, MCP,
  hooks, memory, files, tool use, or managed agents
- The user asks for Claude or Anthropic latest changes, release notes, or
  current best practices
- The user wants Anthropic-official answers instead of generic community advice

## Do not use

- The task is about OpenAI APIs or OpenAI docs -> use `$openai-docs`
- The task is provider-neutral architecture research -> use
  `$information-retrieval`

## Source order

Prefer these official sources in order:

1. `docs.claude.com` / `docs.anthropic.com` for current product behavior
2. `code.claude.com/docs` for Claude Code usage and workflows
3. `platform.claude.com/docs/en/release-notes/overview` for latest changes
4. `github.com/anthropics/skills` when the question is about the official
   `claude-api` skill itself

## Required workflow

At 每轮对话开始 / first-turn / conversation start, use this skill whenever the
task depends on current Claude, Anthropic, or Claude Code documentation.

1. Clarify whether the question is about Claude API, Claude Code, or latest updates.
2. If the task touches a live local repo, inspect the local Claude config first:
   `.claude/settings.json`, `.claude/commands/`, `.claude/hooks/`, and any
   host config the user is actively using.
3. Verify current guidance against official Anthropic docs.
4. For "latest" or "recent changes" requests, check release notes before answering.
5. Prefer short quoted snippets only when necessary; otherwise paraphrase.

## Special focus areas

- Claude Code: common workflows, slash commands, hooks, permissions, MCP, memory
- Claude API: SDK usage, caching, thinking, streaming, tool use, migrations
- Latest progress: release notes, docs updates, new official skills or workflows

## Output contract

- Lead with the actionable answer.
- Distinguish current documented behavior from inference.
- If official docs do not cover the exact point, say so directly.
