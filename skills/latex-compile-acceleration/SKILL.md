---
name: latex-compile-acceleration
description: |
  Speed up LaTeX compile, watch, and preview workflows for papers, books, theses, and Beamer.
  Use when the user asks about LaTeX 编译太慢, preamble 预编译, draft 加速, 编译稳定性, CI 缓存优化, or latexmk/Tectonic/live preview/TikZ externalization/\includeonly/mylatexformat/.latexmkrc. At 每轮对话开始 / first-turn / conversation start, check this skill for compile speed or stability; prefer it over $ppt-beamer or $build-tooling.
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
short_description: Speed up LaTeX compile and preview workflows
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - latex
    - compile
    - acceleration
    - latexmk
    - tectonic
    - texpresso
    - tikz
    - pgfplots
    - preamble
    - draft
    - ci
    - stability
    - mylatexformat
    - latexmkrc
framework_roles:
  - planner
  - executor
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: true
  emits_verification_results: true
risk: low
source: local
---
- **Dual-Dimension Audit (Pre: Tex-Structure/Logic, Post: PDF-Fidelity/Build-Speed Results)** → `$execution-audit-codex` [Overlay]

# latex-compile-acceleration

This skill owns **generic LaTeX compile-stack optimization** across article,
report, book, thesis, paper, and Beamer repositories. Check it early at
conversation start / first turn when the main ask is **slow compile / rebuild /
watch / preview loops**, LaTeX engine choice, figure externalization,
**preamble precompilation**, **draft-mode acceleration**, **compilation stability
/ error recovery**, or **CI caching strategies**.

## Priority routing rule

If the user's request is mainly about:

- compile latency
- watch / preview loop speed
- LaTeX engine or build-driver choice
- figure externalization for faster builds
- preamble precompilation (`mylatexformat`, `.fmt`)
- draft mode for faster iterative editing
- compilation stability, error recovery, or interaction modes
- CI build optimization (caching, Docker, GitHub Actions)
- `.latexmkrc` configuration

then check this skill **before** domain owners such as `$ppt-beamer` and before
generic tooling owners such as `$build-tooling`.

This skill owns the **compile stack, speed path, and build stability**. Domain
skills still own authoring, design, content, and rendered-output review once
compile concerns are settled.

## When to use

- Local LaTeX edit → compile → preview loops are too slow
- CI LaTeX builds are slow or repeatedly cold-start dependencies
- The user wants to choose between `latexmk`, Tectonic, TeXpresso, or adjacent wrappers
- The document is large and needs partial compile strategies such as `\includeonly`
- Heavy TikZ / PGFPlots figures dominate build time and should be externalized
- The preamble loads many packages and the user wants precompilation (`mylatexformat`)
- The user is in a writing phase and wants draft-mode acceleration
- The user wants to reduce PDF compression overhead during development
- Compilation keeps failing or producing stale results (stability / error recovery)
- The user wants to optimize GitHub Actions / Docker CI for LaTeX
- The user wants guidance on `.latexmkrc` best practices
- The user wants reproducible PDF output or dependency pinning
- Best for requests like:
  - "LaTeX 编译太慢了，怎么提速"
  - "给我一个快速编译方案，不只是 beamer"
  - "latexmk / Tectonic / TeXpresso 该怎么选"
  - "TikZ 图太重，怎么别每次都重编"
  - "大论文只改一章，怎么局部编译"
  - "CI 编译 LaTeX 太慢，怎么缓存"
  - "preamble 太重了，能不能预编译"
  - "写东西的时候不需要看图，怎么跳过"
  - "编译老是出错，怎么稳定下来"
  - ".latexmkrc 怎么配最优"
  - "GitHub Actions 编译 LaTeX 怎么优化"

## Do not use

- The main task is authoring or revising a Beamer deck, not optimizing its compile stack → use `$ppt-beamer`
- The main task is academic paper writing / prose polish → use `$paper-writing`
- The build is failing and root cause is still unknown → use `$systematic-debugging` first
- The task is generic JS / TS / Python / package-manager build tooling → use `$build-tooling`
- The primary need is rendered PDF inspection or layout QA → use `$pdf` or `$visual-review`

## Minimal workflow

1. Detect the bottleneck shape: local vs CI, clean vs warm vs watch loop, engine, hotspots, and error pattern.
2. Choose the narrowest speed lever first: `latexmk`, `\includeonly`, externalization, preamble precompilation, draft mode, TeXpresso, or Tectonic cache.
3. For stability issues: check interaction mode, clean aux files, review log with `!` search.
4. Keep one full-build fallback path for sign-off.
5. Verify clean/warm/watch timings plus references, bibliography, cache invalidation, and error recovery behavior.

## Framework fit

Default Detect → Plan → Execute → Verify mapping:

- **findings**: current bottlenecks such as slow watch loops, heavy TikZ, cold CI, package-heavy preamble, or recurring compile errors
- **execution items**: chosen interventions such as `latexmk`, externalization, partial compile, preamble precompilation, draft mode, CI caching, or stability fixes
- **verification**: clean/warm/watch timing checks, convergence of refs/bib, cache invalidation, error recovery, and CI cold/warm timing checks

## Resource Guide

- Read [references/techniques.md](./references/techniques.md) for the sourced tool / technique matrix, concrete command patterns, `.latexmkrc` best practices, stability strategies, and CI optimization recipes.
- **Superior Quality Audit**: For large-scale production LaTeX build systems, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples
- "强制进行 LaTeX 构建深度审计 / 检查编译速度与 PDF 渲染结果。"
- "Use $execution-audit-codex to audit this build pipeline for speed-fidelity idealism."
