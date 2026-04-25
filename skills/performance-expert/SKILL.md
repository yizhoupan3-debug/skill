---
name: performance-expert
description: |
  Audit and improve web performance with emphasis on Core Web Vitals, asset weight, bundle shape,
  rendering cost, and frontend delivery efficiency.
  Use when pages load slowly, Lighthouse scores regress, media is heavy, JavaScript is bloated,
  or the user wants a ranked performance review or optimization plan.
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags:
    - web-performance
    - core-web-vitals
    - lighthouse
    - bundle-size
    - frontend-optimization
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - a ranked performance review
  - optimization plan
  - web performance
  - core web vitals
  - lighthouse
  - bundle size
  - frontend optimization
---

# performance-expert

This skill owns **frontend performance review and optimization planning**.
It focuses on user-visible slowness, CWV metrics, asset weight, and delivery cost.

## When to use

- The user wants a dedicated web-performance audit
- Lighthouse / LCP / INP / CLS are poor or regressed
- Images, videos, fonts, or JS bundles are too heavy
- The task is to rank frontend performance bottlenecks and fixes

## Do not use

- React/Next rendering architecture as the main issue -> use `$nextjs` with the Vercel best-practices reference
- Backend / DB / API latency debugging → use the relevant backend skill
- Implementation-level code acceleration such as pandas → polars, faster serializers, or hot-path library swaps → use `$code-acceleration`
- Pure visual redesign with no performance concern

## Core workflow

1. Determine whether the task is audit mode or implementation guidance.
2. Check in this order:
   - Core Web Vitals
   - asset weight
   - bundle / JS cost
   - delivery and caching
   - rendering / interaction cost
3. Prioritize the biggest user-visible bottleneck first.
4. Tie every recommendation to a metric or resource class.
5. Deliver a ranked fix plan.

## Hard constraints

- Do not recommend blanket optimizations with no visible payoff.
- Do not treat backend latency as frontend performance by default.
- Prefer structural wins before micro-optimizations.
- State uncertainty clearly when no real metrics exist.
