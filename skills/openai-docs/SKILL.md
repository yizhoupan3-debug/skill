---
name: openai-docs
description: Query current official OpenAI developer documentation.
routing_layer: L1
routing_owner: gate
routing_gate: source
routing_priority: P1
session_start: required
short_description: Use official OpenAI docs first for current OpenAI guidance
trigger_hints:
  - OpenAI docs
  - OpenAI API
  - Responses API
  - Apps SDK
  - Codex docs
  - 官方文档
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags:
    - openai
    - documentation
    - api
    - codex
    - mcp
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

# OpenAI Docs

This visible shared skill mirrors the OpenAI-docs docs-first workflow so both
Codex uses this skill entry and the same MCP-backed source.

## When to use

- The user asks about current OpenAI APIs, models, SDKs, or product docs
- The answer must follow the latest official OpenAI developer documentation
- The task is model selection, upgrade planning, or docs-backed troubleshooting
- The user explicitly wants OpenAI official guidance instead of generic advice

## Do not use

- The main question is about a non-OpenAI provider or non-Codex host
  configuration -> answer in the current source-research context, not this OpenAI docs gate
- The task is broad external research rather than OpenAI docs lookup
  -> answer in the current source-research context, not this OpenAI docs gate

## Required workflow

At 每轮对话开始 / first-turn / conversation start, use this skill whenever the
task depends on current OpenAI product or API documentation.

1. Use the OpenAI Developer Docs MCP tools first.
2. Search before fetching exact sections.
3. Quote briefly and prefer paraphrase with citations.
4. Only fall back to web search if the MCP server returns no meaningful result.
5. When falling back to web search, restrict to `developers.openai.com` and
   `platform.openai.com`.

## Preferred tools

- `mcp__openaiDeveloperDocs__search_openai_docs`
- `mcp__openaiDeveloperDocs__fetch_openai_doc`
- `mcp__openaiDeveloperDocs__list_openai_docs` only when discovery is needed

## If the MCP server is missing

1. Install `openaiDeveloperDocs` for Codex from `https://developers.openai.com/mcp`.
2. Retry the doc lookup after installation.
3. If installation or lookup is unavailable, return a source blocker and do not
   answer volatile OpenAI product behavior from memory alone.

## Output contract

- Lead with the answer.
- Keep it concise unless the user asked for depth.
- Cite the official OpenAI doc page or section used.
- Verification path: name how to verify the MCP doc result or official URL used; if web
  fallback was required, say that the fallback was restricted to official OpenAI
  domains.
- Failure path: surface missing-doc, stale-source, or tool error states as
  blockers with the next retry step, not as successful guidance.
