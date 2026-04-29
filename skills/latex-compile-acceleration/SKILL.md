---
name: latex-compile-acceleration
description: |
  Speed up LaTeX build, watch, preview, and CI workflows with measurement-first tactics:
  latexmk, Tectonic, TeXpresso, TikZ/PGFPlots externalization, \includeonly/subfiles,
  mylatexformat, draft mode, .latexmkrc, caching, and stable error recovery. Use for
  LaTeX 编译太慢, watch 太慢, preamble 预编译, TikZ 很慢, CI 缓存优化, or LaTeX build stability.
  Prefer this skill over ppt-beamer or build-tooling only when the main problem is clearly LaTeX compile speed.
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: false
disable-model-invocation: true
short_description: Speed up LaTeX compile and preview workflows
trigger_hints:
  - LaTeX 编译太慢
  - watch 太慢
  - preview loop
  - 编译稳定性
  - CI 缓存优化
  - latexmk
  - Tectonic
  - TeXpresso
  - TikZ externalization
  - PGFPlots externalization
  - \includeonly
  - subfiles
  - standalone
  - mylatexformat
  - preamble 预编译
  - .latexmkrc
metadata:
  version: "2.1.0"
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

# latex-compile-acceleration

Owns LaTeX compile-stack speed and stability for papers, theses, books,
reports, and Beamer when the pain is build latency rather than slide/content
authoring.

Default posture: **measure first, choose the narrowest speed lever, keep one
serial full-build path for sign-off**.

## When to use

- LaTeX clean, warm, or watch builds are slow.
- The user asks about `latexmk`, Tectonic, TeXpresso, `.latexmkrc`, or engine choice.
- Heavy TikZ/PGFPlots figures, image conversion, bibliography passes, or preamble
  loading dominate compile time.
- A large thesis/book needs `\includeonly`, `subfiles`, `standalone`, or chapter
  preview strategy.
- The user wants `mylatexformat`, draft mode, lower PDF compression, or other
  local iteration speedups.
- CI LaTeX builds need package caching, reproducible installs, Docker tuning, or
  build sharding.
- Compilation is flaky because of stale aux files, interaction modes, or rerun
  churn, and the root error is already visible enough to optimize recovery.

## Do not use

- Main task is Beamer content/layout/design authoring, not build speed: use
  `$ppt-beamer`.
- Main task is rendered PDF inspection or visual QA: use `$pdf` or
  `$visual-review`.
- Build is failing and the root cause is unknown: use `$systematic-debugging`
  first, then return here for speed/stability.
- Task is generic JS/TS/Python/Rust build tooling: use `$build-tooling`.
- User asks to rewrite TeX compilation itself in Rust: route to `$rust-pro` only
  after confirming this is a new tool project, not normal LaTeX acceleration.

## Execution Contract

1. Identify the target workflow: local edit loop, full release build, CI build,
   or error-recovery workflow.
2. Measure or request timings for clean build, warm build, and watch/edit loop
   unless the user only wants conceptual advice.
3. Classify the bottleneck: preamble, figures, bibliography/refs, chapter
   structure, image conversion, CI cold start, engine choice, or aux churn.
4. Pick the smallest matching lever from the playbook below.
5. Preserve a serial full build as the final correctness check.
6. Verify timing, ref/bib convergence, cache invalidation, and error output.

## Default Playbook

| Bottleneck | First move | Next move |
|---|---|---|
| Unknown local slowness | `latexmk` baseline with `-file-line-error` and isolated `build/` output | inspect `.log`, `.fls`, and warm vs clean delta |
| Slow edit/watch loop | `latexmk -pvc`, draft mode, lower PDF compression | TeXpresso if near-live preview matters |
| Heavy TikZ/PGFPlots | externalize figures | isolate figures with `standalone` |
| Package-heavy preamble | `mylatexformat` | split static vs dynamic preamble with `\endofdump` |
| Large thesis/book | `\includeonly` or `subfiles` | chapter-local preview targets |
| Bibliography/refs dominate | keep serial convergence | reduce unnecessary reruns, clean stale aux |
| CI cold starts | cache TeX Live/Tectonic bundle | minimal package profile or pinned Docker image |
| Output chaos/stale files | `.latexmkrc` with `out_dir`/`aux_dir` | `latexmk -c`/`-C` discipline |

## Parallelism Gate

Only use parallel compile lanes when compile units are explicit and outputs do
not fight over the same aux tree:

- Good: independent `\include` chapters, `subfiles`, `standalone` figures,
  TikZ externalized outputs, or CI shards with isolated output directories.
- Bad: one monolithic document, shared bibliography/ref convergence,
  package-heavy preamble load, unclear `build/` ownership, or a short watch loop
  where orchestration costs more than recompilation.
- Single-writer rule: one integrator owns shared `.aux`, `.bbl`, `.toc`, output
  directories, and final PDF sign-off.

## Rust Boundary

This skill is **not fully Rust** and should not be described that way.

Rust owns host entrypoints, alias projection, durable lane orchestration, and
resume state. LaTeX diagnosis and tactic choice stay in this skill and its
reference docs. Do not present Rustification as the default fix for ordinary
LaTeX slowness.

## Output Defaults

- For advice: give the ranked diagnosis, exact command/config snippet, expected
  speed effect, caveats, and verification command.
- For repo edits: modify only build config/scripts needed for the chosen lever,
  keep rollback simple, and avoid touching document content unless required.
- For CI: include cache keys, invalidation conditions, and cold/warm timing
  checks.

## Reference Map

Read [references/techniques.md](./references/techniques.md) for command recipes,
`.latexmkrc`, externalization, preamble precompilation, CI caching, stability
checks, and validation details.
