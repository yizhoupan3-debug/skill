---
name: deep-research
description: |
  Deep research harness — fan-out web searches, fetch sources, adversarially verify claims,
  synthesize a cited report. - When the user wants a deep, multi-source, fact-checked
  research report on any topic. BEFORE invoking, check if the question is specific enough
  to research directly — if underspecified (e.g., "what car to buy" without budget/use-case/region),
  ask 2-3 clarifying questions to narrow scope. Then pass the refined question as args, weaving
  the answers in.
routing_layer: L2
routing_owner: user
routing_gate: approve
routing_priority: P2
session_start: preferred
user-invocable: true
disable-model-invocation: true
short_description: Deep research harness
trigger_hints:
  - 深度调研
  - deep research
  - 全面研究
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags:
    - research
    - harness
    - web
risk: low
source: local
---

# Deep Research

This skill provides a deep research harness that fans out across multiple web searches,
fetches source documents, adversarially verifies claims, and synthesizes a cited report.

## Input

The user provides a topic or question to research.

## Execution

The harness runs as a workflow using the `browser-mcp` tools for web fetching and
searching.

1. **Plan Searches**: Generate a set of diverse search queries to cover different aspects of the topic.
2. **Execute Searches**: Run the searches in parallel and collect URLs.
3. **Fetch Content**: Fetch the content of the top URLs in parallel.
4. **Extract Claims**: Extract claims and evidence from the fetched content.
5. **Verify Claims**: Adversarially verify the extracted claims against each other.
6. **Synthesize Report**: Synthesize a final report with citations to the verified sources.
