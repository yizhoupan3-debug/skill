# Skill Reduction Checklist

Goal: keep skills only when they provide domain knowledge, non-obvious tool/source constraints, artifact semantics, evidence standards, failure-mode expertise, or work-product constraints. Move generic control flow into runtime without turning runtime into a mega-skill, and delete obsolete files/docs/contracts only after proving they are not live sources of truth, required fixtures, or compatibility surfaces.

## Remaining execution tasks from current review

- [x] Enforce generated-artifact drift through the manifest-backed path in CI: update `.github/workflows/skill-ci.yml` to use `framework host-integration generated-artifacts-status` instead of only a hard-coded `git diff --quiet` file list.
- [x] Add `cargo test --test host_integration` to the configured CI test set so host projection, generated-artifact, and compatibility-surface assertions in `tests/host_integration.rs` fail CI.
- [x] Broaden undeclared generated-artifact detection beyond `configs/framework/*.json` with `generated-by-` markers; include generated manifests, host entrypoints, CI, docs, tests, `.codex/**`, `AGENTS.md`, and `skills/SKILL_*` surfaces required by the reverse-reference gate.
- [x] Validate `configs/framework/GENERATED_ARTIFACTS.json` schema version when loading the generated-artifacts manifest; reject missing or unsupported `schema_version` instead of accepting any object with `generated_artifacts`.
- [x] Finish hot-path cleanup for generic control skills: remove or downgrade `systematic-debugging`, `idea-to-plan`, `plan-to-code`, and `skill-framework-developer` from `skills/SKILL_ROUTING_RUNTIME.json` unless the trigger is exact, explicit, or a precise retained domain/artifact/source gate.
- [x] Decide and document whether `autopilot` and `team` generated alias stubs may remain in the hot runtime index for exact invocation discovery; if yes, add negative routing tests proving ordinary coding/planning/debugging requests do not select them.
- [x] Resolve `team` command routing policy: either make it explicit-only or justify `implicit_route_policy: strong-orchestration-only` with negative routing tests and a clear framework-command rationale.
- [x] Remove broad bare alias triggers from canonical owner skills when they cause implicit routing competition, especially bare `autopilot` / `team`; keep only `$...` / `/...` exact-entrypoint wording when documenting explicit aliases.
- [x] Add deletion inventory rows for deleted or retired plugin/generated/legacy surfaces, including `plugins/skill-framework-native/**`: file class, canonical owner/input, consumers checked, successor or retirement rationale, deletion safety status, tests run, and restore plan.
- [x] Track and commit or intentionally exclude the new generated/reference artifacts needed by this reduction round, especially `configs/framework/GENERATED_ARTIFACTS.json`, `skills/plan-to-code/references/autopilot-mode.md`, and `skills/agent-swarm-orchestration/references/team-mode.md`. `configs/framework/GENERATED_ARTIFACTS.json` and both reference files are intentionally retained; `.codex/host_entrypoints_sync_manifest.json` is explicitly unignored so it can be tracked; generated alias stubs under `artifacts/codex-skill-surface/` remain ignored disposable host projections.

## Core principle

Runtime owns control flow. Skills own narrow deltas. Canonical contracts own invariants. Generated host projections are disposable only when their generator and canonical source are known.

A skill should remain only if it satisfies at least one of:

- It contains domain knowledge that Codex should not infer from generic runtime behavior.
- It owns non-obvious tool invocation constraints, artifact semantics, external-source access rules, or safety constraints that runtime cannot reliably infer.
- It defines correctness constraints specific to a domain, platform, source, artifact, evidence class, failure mode, investigation standard, or work product.
- Removing it would make routing or execution materially worse for that specific domain/tool/source/artifact/evidence/work-product boundary.

A skill should be removed, merged, narrowed, made explicit-only, or sunk into runtime if it mainly describes:

- planning
- checklist creation
- delegation
- generic verification selection/reporting
- context compression
- routing strategy
- clarification protocol
- generic debugging discipline
- generic code quality rules
- generic research workflow
- generic execution control

## Runtime concision guardrail

When sinking behavior into runtime, reduce it to the smallest always-on invariant.

Do not move long procedures, examples, rubrics, domain-specific cases, source-specific rules, artifact acceptance checks, or reusable diagnostic/research playbooks into runtime. Those either stay in retained skills, move to narrower skills, or become explicit-only references.

## Glossary

- **Runtime policy**: always-on behavior for planning, routing, clarification, delegation, generic verification selection/reporting, context management, and default coding discipline.
- **Skill**: a loaded domain/tool/source/artifact/evidence/work-product instruction bundle. It should not be required for generic Codex behavior.
- **Hot path**: the small first-turn routing surface loaded or checked before normal work, especially entries emitted into `skills/SKILL_ROUTING_RUNTIME.json`.
- **Explicit-only**: a skill remains available only when the user explicitly invokes it or the request has a precise domain/tool/source/artifact trigger. It is not eligible for generic hot-path routing.
- **Fallback**: discoverable through fallback manifest/search after hot routing has no confident hit; not part of the first-turn hot set.
- **Unknown route status**: metadata is missing, generated, stale, or ambiguous enough that the reviewer must inspect canonical routing sources before changing it.
- **Front door**: a skill that routes among narrower skills for a bounded object, such as a manuscript, artifact family, source family, or research project type.
- **Overlay**: a secondary behavioral modifier selected by runtime. Skills must not self-compose, chain, preload, or request additional overlays unless the user or runtime explicitly asks.
- **Narrow delta**: the useful part of a skill that cannot be replaced by runtime control flow.
- **Generated host projection**: host-specific generated skill/command/registry output. It must point back to canonical sources and is disposable only when the generator and canonical input are known.
- **Canonical machine registry/config**: machine-readable registry or config emitted from compiler/Rust/source inputs that may itself be authoritative for runtime behavior. Do not treat all generated files as disposable; identify the canonical source before editing.
- **Compatibility pointer**: intentionally retained shim, redirect, or migration note for old host/user entrypoints. Delete only after supported callers and host adapters no longer depend on it.
- **Retired contract**: formerly normative behavior intentionally removed. Delete only with an explicit replacement/removal decision and tests proving runtime no longer promises it.

## Docs/contracts/file classification

Every non-skill file touched by reduction must be classified before edit/delete:

- **Live source-of-truth contract**: normative requirements consumed by humans and/or tests. Edits require downstream regeneration/test updates.
- **Canonical machine contract/config**: authoritative runtime/config/registry input or output used by runtime/tests. It may be generated, but is not disposable unless its generator and canonical source are confirmed.
- **Generated projection**: host-specific, compiled, emitted, or mirrored output. It should contain pointer/provenance and be regenerated, not manually maintained.
- **Obsolete generated projection**: generated output for a retired host/surface whose generator or registry declaration has been removed or intentionally retired.
- **Compatibility pointer**: retained shim/redirect/migration note for old host/user entrypoints. Deletion requires evidence no supported caller depends on it.
- **Stale duplicate**: overlapping content with no authoritative role and no live references. Delete after reference scan and parity check.
- **Retired contract**: formerly normative behavior intentionally removed. Delete only with explicit replacement/removal rationale and tests proving the old behavior is no longer promised.
- **Historical doc**: archival context with no normative role. Delete or archive only after confirming no tests/configs/docs rely on it as a contract.
- **Active contract doc**: human-readable normative contract for runtime, host adapters, generated artifacts, schemas, or policies.
- **Test fixture/baseline**: expected output, fixture, snapshot, or drift baseline. Do not delete until tests are updated or retired.
- **Plugin/legacy host artifact**: old plugin or host projection. Candidate for deletion only after confirming it is not shared source of truth, current sync input, generated manifest entry, or test fixture.
- **Unknown**: deletion is blocked until the canonical owner and consumers are found.

## Narrow delta types

Classify every retained delta as one or more of:

- domain knowledge
- tool invocation constraint
- source access rule
- artifact semantics
- safety constraint
- evidence class
- failure mode
- work product
- investigation standard
- mandatory domain/artifact-specific verification
- ecosystem-specific idiom, lifecycle constraint, build/test command, or failure mode

Examples:

- Strong owner: `pdf` uniquely owns PDF rendering, repair, page/layout, and extraction constraints.
- Invalid owner: `coding-standards` uniquely owns good code quality. This is generic unless it names ecosystem-specific correctness deltas.
- Mixed owner: `information-retrieval` may own external evidence standards and source-quality rules, but not generic pre-action research control.

## Runtime-owned behaviors

Move these to runtime policy instead of keeping them as skills:

- Extract object / action / constraints / deliverable / success criteria first.
- Ask clarifying questions only when execution would otherwise be unsafe or ambiguous.
- If root cause is unknown, reproduce and gather evidence before fixing.
- If a plan/spec is already concrete, implement it directly.
- For complex work, split into bounded execution slices.
- Select the primary owner and any overlay; do not let skills self-compose route chains.
- Use at most one behavioral overlay unless runtime or the user explicitly needs more.
- Select generic verification for the task and report the result compactly.
- Keep main-thread output compact; sink raw evidence into artifacts only when useful.
- Do not preload the full skill library.

Retained skills may still specify mandatory domain/artifact/source-specific verification, required evidence, failure signatures, or artifact acceptance checks, such as PDF rendering checks, citation truth checks, accessibility checks, framework behavior checks, or source provenance checks.

## Fast-pass minimum

For high-volume audits, record at least:

1. owner statement;
2. narrow delta types;
3. generic sections to sink;
4. overlap or competing skill;
5. final decision label;
6. concrete action;
7. rationale.

Run the full source-of-truth, framework-command, generated-surface, and file/contract deletion checks whenever an action changes routing, registries, command ownership, generated surfaces, docs/contracts, file deletion, or deletes/merges a skill.

## Deterministic audit procedure

Use this order for each skill or related file/doc/contract:

1. **Inspect routing metadata**
   - Record `routing_layer`, `routing_owner`, `routing_gate`, `routing_priority`, `session_start`, trigger hints, and current route status.
2. **Determine route status**
   - `hot`: present in the emitted hot routing set or loaded at session start.
   - `explicit`: direct invocation/manual command only; no generic hot-path trigger.
   - `fallback`: only discoverable through fallback manifest/search when hot routing has no confident hit.
   - `unknown`: cannot be determined without checking canonical routing sources.
3. **Read the skill body and related docs/contracts**
   - Identify what the skill actually instructs, not just the description.
   - Identify any docs/contracts/fixtures/generated artifacts that define or test the behavior.
4. **Write a one-sentence owner statement**
   - Format: `This skill uniquely owns ...`.
   - The sentence should name a domain, tool, source, artifact, evidence class, failure mode, work product, or investigation standard.
   - If it cannot, mark for deeper review rather than immediate deletion.
5. **Classify narrow deltas by type**
   - Use the narrow delta type list above.
6. **Classify related files/docs/contracts**
   - Use the docs/contracts/file classification list above.
   - `unknown` blocks deletion.
7. **Separate generic control flow from narrow deltas**
   - Mark sections about planning, clarification, routing, delegation, generic verification selection/reporting, context compression, or generic quality discipline as runtime candidates.
   - Mark domain/tool/artifact/source/evidence/failure/work-product-specific sections as retained-delta candidates.
8. **Run a pre-delete reverse-reference gate**
   - Before deleting/merging any skill/file/doc/contract, search and record references across `skills/SKILL_ROUTING_RUNTIME.json`, `skills/SKILL_MANIFEST.json`, `configs/framework/RUNTIME_REGISTRY.json`, `configs/framework/GENERATED_ARTIFACTS.json`, `configs/framework/*.json`, `scripts/router-rs/src`, `scripts/skill-compiler-rs/src`, `tests`, `docs`, host adapter docs, generated artifact manifests, CI scripts, README/entrypoint docs, and `AGENTS.md` entrypoints.
   - Check exact paths and semantic references: filenames, titles, contract names, registry keys, schema fields, skill names, command aliases, old paths, fixture names, generated artifact names, host paths, and MUST/SHOULD normative claims.
   - Deletion is blocked until every route, fixture, test, generated artifact declaration, host alias path, and normative claim is updated or explicitly retired.
9. **Check routing overlap and front-door collision**
   - Identify narrower skills that already own the same object.
   - Identify competing front doors or workbenches.
10. **Check self-composition**
   - Flag any instruction that tells the skill to invoke, chain, preload, or require other skills without runtime/user selection.
11. **Check manifest path existence**
   - Every `skill_path` in hot routing and fallback manifests must exist after reduction unless it is an explicit generated alias whose generator and owning runtime registry entry still exist.
12. **Check framework command impact**
   - If changing an owner/gate/control skill, inspect command registry fields such as `canonical_owner`, `reroute_when_ambiguous`, `reroute_when_root_cause_unknown`, `delegation_gate`, `execution_owners`, `review_lanes`, `local_adaptations`, verifier fields, and aliases.
   - Update or justify every affected `framework_commands` binding before regeneration.
13. **Audit hardcoded runtime fallbacks and generated aliases**
   - Check Rust/runtime defaults and routing heuristics for compiled fallback JSON, alias defaults, special-case boosts/suppressions, exact alias matching, pinned skill lists, alias templates, and command projections.
   - Decide whether generated framework aliases survive independently of deleted control skills.
14. **Check generated/source-of-truth integrity**
   - Identify the canonical source before editing.
   - Generated host projections must remain pointers and be regenerated after routing changes.
   - Canonical machine registries/configs may be authoritative even if generated by compiler/Rust inputs.
15. **Assess doc/contract dependency and claims**
   - For each doc/contract candidate, identify inbound references from tests, configs, runtime registries, generated manifests, host adapters, entrypoints, scripts, CI, and README-like docs.
   - Identify outbound normative claims such as MUST/SHOULD, schema fields, command names, host paths, generated artifact names, compatibility surfaces, and supported aliases.
   - Mark each claim as test-enforced, code-generated, registry-backed, or descriptive.
16. **Assign a final decision label using the precedence rules below**.
17. **Record concrete routing, body, and file actions**
   - Hot-path removal, explicit-only conversion, trigger reduction, priority change, body trimming, merge target, destination mapping, file deletion/retirement, successor path, generator/regeneration path, or deletion rationale.
18. **Run behavioral parity checks for merge/delete**
   - Verify the destination preserves triggerability, explicit invocation discoverability, required tools, artifact outputs, source gates, mandatory verification, user mental model, host adapter expectations, runtime registry loading, and generated artifact drift behavior.
19. **Regenerate and verify**
   - Routing changes must update canonical routing sources and regenerate generated registries/host projections instead of patching generated mirrors.
   - Run relevant contract/tests after changes.

## Decision labels and section actions

The table's **Decision label** should describe the final retained-skill state:

- `DELETE`
- `MERGE`
- `NARROW`
- `EXPLICIT_ONLY`
- `KEEP`

`SINK_RUNTIME` is usually a section action, not the final state. Use `SINK_RUNTIME` as a whole-skill final label only when every useful part of the skill is generic runtime behavior and no narrow delta remains.

### Label precedence

Apply labels in this order for mixed cases:

1. `MERGE`: a useful delta exists, but another narrower skill owns the boundary better.
2. `NARROW`: useful, but currently too broad or mixed with generic control flow.
3. `EXPLICIT_ONLY`: useful only for direct invocation or precise trigger; not eligible for generic hot-path routing.
4. `KEEP`: strong, unique, concrete ownership remains after trimming.
5. `DELETE`: only after delta extraction, reference scan, destination mapping, and deletion safety checks are complete and no unique delta or live contract remains.

Uncertain or mixed cases default to `NARROW`, `MERGE`, or `EXPLICIT_ONLY`, not `DELETE`.

Choose `MERGE` when no bounded owner/front-door role remains after deltas move. Choose `NARROW` when the skill remains useful as a bounded owner/front door after trimming.

## Concrete routing and cleanup actions

When a decision mentions routing or deletion, specify the exact operation:

- **Remove from hot path**: change the canonical routing source that emits `skills/SKILL_ROUTING_RUNTIME.json`; never patch host-generated copies except via regeneration.
- **Reduce triggers**: delete generic trigger hints and keep only precise domain/tool/source/artifact/user-invocation triggers.
- **Lower priority**: reduce priority when the skill should lose owner competition but remain discoverable.
- **Make explicit-only**:
  - remove from generic hot `skills[]` entries;
  - retain slash/manual route in the manifest or command registry;
  - if retained in a hot index only for alias discovery, keep exact-invocation triggers only and remove generic hints;
  - ensure implicit routing policy is effectively never, if such a field exists.
- **Merge**: move narrow delta into the named retained skill, then delete or deprecate the source skill only after behavioral parity is checked.
- **Trim body**: remove generic runtime-control sections while retaining domain/tool/source/artifact/evidence/work-product deltas.
- **Retire/delete old artifact**: classify the artifact, map still-valid requirements to a canonical owner, remove or regenerate references, update tests/configs in the same change, then delete stale duplicates/retired contracts.
- **Regenerate surfaces**: regenerate host registries/projections from canonical source; do not hand-edit generated host artifacts.
- **Update framework commands**: update or justify command-owner, reroute, delegation, verifier, execution owner, review lane, local adaptation, and alias bindings that referenced the changed skill.
- **Update tests/fixtures**: update or retire assertions and fixtures that assert old control-skill selection, generated alias presence, command owners, routing cases, or contract paths.

Avoid leaving deprecation stubs unless they are deliberate compatibility pointers with an owner, expiry, and removal condition.

## File/contract deletion inventory

Before deleting files/docs/contracts, create an inventory row for every candidate path.

Required fields:

- file path(s);
- file class;
- canonical owner/input;
- generator and regeneration path, if generated;
- runtime/test/config/host consumers;
- inbound refs checked;
- outbound normative claims;
- successor/replacement path or no-successor rationale;
- deletion safety status;
- restore plan.

Deletion requires one of:

- superseded by a named current contract;
- merged into a named retained doc/skill/contract;
- converted into runtime policy, registry field, schema, or contract test;
- confirmed stale with no runtime/test/reference consumers;
- intentionally retired with tests proving the old behavior is no longer promised.

## Git deletion safety

- Keep deletion diffs explicit and reviewable.
- Review `git status` before and after deletion.
- Do not use bulk deletion globs until the inventory is reviewed.
- Do not delete untracked files unless positively identified as generated/obsolete.
- Preserve deleted file paths in the audit table so `git restore -- <path>` is straightforward during review.
- If a file is a test fixture/baseline, treat it as live until the owning test is updated or retired.

## Do-not-delete guardrail

Do not delete any control, workbench, source-gated, debugging, research, language/framework, orchestrator, doc, contract, fixture, generated artifact, or legacy host/plugin file until every narrow delta, normative claim, and live reference has been handled:

- moved to a retained narrower skill/doc/contract;
- converted into explicit-only maintenance behavior;
- sunk into runtime policy because it is genuinely generic and reduced to a minimal invariant;
- preserved as a reusable cross-domain evidence/diagnostic/research playbook;
- replaced by a named canonical registry/config/schema/test;
- intentionally retired with behavior-removal evidence; or
- confirmed nonexistent and recorded as deletion rationale.

This guardrail is especially important for debugging, research, framework governance, source-gated, language/framework, workbench, host adapter, generated artifact, and compatibility files.

## Old file / doc / contract deletion gate

Before deleting any file that looks old, legacy, retired, transitional, generated, mirrored, or compatibility-only, classify it as exactly one of:

- **Canonical source of truth**: the authoritative human or machine source for a live invariant.
- **Generated-but-drift-checked artifact**: generated output that tests or manifests compare byte-for-byte, such as routing/runtime/generated-artifact outputs.
- **Active host compatibility pointer**: a retained path, shim, redirect, or entrypoint still loaded by supported hosts.
- **Executable documentation contract or test fixture**: prose or baseline read by tests to freeze policy, wording, schema, route, or host-adapter invariants.
- **Intentional absence contract**: a retired path whose absence is itself asserted by tests or policy.
- **Disposable stale artifact**: no authoritative role, no live references, no drift-check requirement, no compatibility caller, and no retained invariant.

Deletion is allowed only for disposable stale artifacts, or when the owning manifest, tests, contracts, and host pointers are intentionally updated in the same change.

Required pre-delete checks:

1. Search tests, Rust source, configs, generated-artifact manifests, CI, host entrypoint sync code, and docs for references.
2. If the file appears in `configs/framework/GENERATED_ARTIFACTS.json`, regenerate and compare; do not delete it merely because it is generated.
3. If a doc is read by policy/contract tests, preserve the invariant in an equivalent retained contract before renaming or deleting it.
4. If a retired path is asserted absent by tests, record it as an intentional absence contract, not a missing cleanup task.
5. If active hosts still load the path as a pointer, replace the pointer and prove host startup/routing still works before deletion.

Every deletion record must include classification, reference-closure evidence, retained-invariant destination, affected host surfaces, tests run, and restore plan.

## Presumptive retained-delta categories

The following categories are presumptive retained-delta categories, not keep exemptions. Retain their domain/tool/source/artifact/evidence/work-product delta, but still trim generic planning, runbook, routing, delegation, and generic verification control flow.

The default label may still be `NARROW`, not `KEEP`, until generic sections are removed and overlap is reviewed.

### Artifact/tool skills

- `pdf`
- `doc`
- `slides`
- `ppt-pptx`
- `ppt-beamer`
- `source-slide-formats`
- `jupyter-notebook`
- `diagramming`
- `screenshot`
- `visual-review`
- `image-generated`
- `latex-compile-acceleration`
- `github-actions-authoring`
- `cloudflare-deploy`
- `docker`
- `sentry`
- `openai-docs`
- `citation-management`
- `email-template`
- `chrome-extension-dev`

### Language/framework/platform skills

Treat framework idioms, lifecycle constraints, unsafe footguns, build/test commands, portability constraints, and ecosystem-specific failure modes as narrow deltas even when they resemble coding standards.

- `rust-pro`
- `python-pro`
- `typescript-pro`
- `javascript-pro`
- `go-pro`
- `sql-pro`
- `react`
- `vue`
- `svelte`
- `nextjs`
- `node-backend`
- `css-pro`
- `web-platform-basics`
- `shell-cli`
- `linux-server-ops`
- `native-app-debugging`
- `mcp-builder`
- `monorepo-tooling`
- `npm-package-authoring`

### Domain skills

- `auth-implementation`
- `security-audit`
- `security-threat-model`
- `observability`
- `performance-expert`
- `accessibility-auditor`
- `api-design`
- `api-integration-debugging`
- `api-load-tester`
- `datastore-cache-queue`
- `dependency-migration`
- `env-config-management`
- `i18n-l10n`
- `seo-web`
- `web-scraping`
- `data-wrangling`
- `financial-data-fetching`
- `algo-trading`

## First-pass control-skill review list

Review these first. They are mostly control-flow skills or broad orchestration wrappers:

- `idea-to-plan`
- `plan-to-code`
- `systematic-debugging`
- `tdd-workflow`
- `coding-standards`
- `deepinterview`
- `prompt-engineer`
- `information-retrieval`
- `research-workbench`
- `skill-framework-developer` as a normal hot owner

Recommended handling:

1. Remove from hot path unless explicitly invoked or clearly domain/tool/source/artifact-specific.
2. Move useful generic rules into runtime policy as minimal invariants.
3. Keep only narrow domain/tool/source/artifact/evidence/work-product deltas, if any.
4. Prefer `EXPLICIT_ONLY`, `NARROW`, or `MERGE` before deletion when a narrow delta remains.
5. Delete the skill only after delta extraction and destination mapping show no retained value.

## Special review cases

### `idea-to-plan`

Default action: sink generic planning control into runtime and remove from hot path.

Keep only if it is limited to repo-local planning work products such as:

- `outline.md`
- `decision_log.md`
- `assumptions.md`
- `open_questions.md`
- `plan_rubric.md`
- `code_list.md`

Delete only if it means "think before coding" or generic route comparison and no repo-local work-product contract remains.

### `plan-to-code`

Default action: sink generic spec-to-implementation behavior into runtime.

Codex should implement concrete specs by default. Keep no separate skill unless there is a repo-specific spec-to-code artifact/work-product contract.

### `systematic-debugging`

Default action: convert the generic rule into a runtime evidence gate.

The rule "unknown root cause -> reproduce and gather evidence first" should always apply without loading a skill.

Before deleting or narrowing, relocate or preserve:

- domain-specific reproduction surfaces;
- signal interpretation rules;
- log triage heuristics;
- bisection patterns;
- flake isolation;
- environment capture;
- minimal reproduction construction;
- incident evidence preservation;
- reusable diagnostic playbooks that materially improve root-cause quality even when cross-domain.

Potential destinations include:

- `frontend-debugging`
- `backend-runtime-debugging`
- `api-integration-debugging`
- `native-app-debugging`
- explicit-only diagnostic reference material

### `tdd-workflow`

Default action: convert generic TDD selection into a runtime overlay.

Keep only the explicit user-triggered behavior: when the user asks for TDD, run red-green-refactor. Do not let this become a generic development controller.

### `coding-standards`

Default action: merge generic rules into runtime coding policy as minimal invariants.

Generic simplicity, naming, no speculative abstraction, and style discipline should not require a skill. Language/framework-specific correctness constraints should remain in narrow skills such as:

- `rust-pro`
- `python-pro`
- `typescript-pro`
- `react`
- `nextjs`
- `sql-pro`
- `shell-cli`

### `deepinterview`

Default action: merge generic clarification protocol into runtime or delete after delta mapping.

One-question-at-a-time clarification is interaction policy, not a skill. Keep only if a repo-specific artifact/tool-backed interview workflow, work product, or explicit review mode remains and is explicit-only.

### `prompt-engineer`

Default action: split.

- External prompt writing may remain as a narrow copywriting/AI-prompt skill.
- Internal delegation prompt quality should move to runtime as minimal prompt-quality invariants.

### `agent-swarm-orchestration`

Default action: `NARROW` or `EXPLICIT_ONLY`.

Keep only for designing or debugging multi-agent systems as a product. Do not use it to control the current Codex session.

### `information-retrieval`

Default action: sink generic pre-action research control into runtime, then review the remainder.

Do not delete until source access rules, evidence grading, freshness checks, citation/provenance constraints, authoritative-source selection, forbidden substitutes, and tool/source-specific retrieval behavior are retained or explicitly declared absent.

Potential retained deltas:

- source-quality rules;
- external evidence standards;
- multi-round retrieval strategy;
- GitHub/source timeline reconstruction;
- citation, provenance, and freshness discipline;
- credentialed/private/source-of-truth boundaries;
- authoritative-source selection and forbidden substitutes.

### `research-workbench`

Default action: `NARROW`, `MERGE`, or `EXPLICIT_ONLY`.

Keep only if it carries non-manuscript research-project domain judgment, evidence standards, or work-product contracts. Generic "decide next step" orchestration belongs in runtime.

Define its boundary against `paper-workbench`:

- `paper-workbench`: manuscript artifact lifecycle, reviewer response, submission readiness, paper revision/writing routes.
- `research-workbench`: non-manuscript research investigation, experiment direction, novelty/risk judgment, and research-project next moves.

### `paper-workbench`

Default action: keep but narrow.

A manuscript front door is useful, but it should route only manuscript-level work and not become a generic research controller.

Before narrowing or merging any workbench, enumerate owned work products and artifact contracts, such as reviewer matrices, rebuttal constraints, experiment logs, benchmark tables, annotated bibliographies, submission checklists, sequencing rules, or quality constraints.

### `skill-framework-developer`

Default action: `EXPLICIT_ONLY`.

Keep for framework/routing/runtime governance, schema work, generated surface policy, command registry impacts, and skill-boundary maintenance. Do not use it as a generic session controller.

## Workbench/front-door collision test

A front-door or workbench skill may remain only if all are true:

- It has a bounded object, artifact family, source, work product, or project domain.
- It enumerates owned work products and artifact contracts before narrowing.
- It routes among narrower skills only inside that bounded object.
- It does not replace runtime task control, planning, generic verification, or generic orchestration.
- It has a clear split from neighboring front doors.
- It is not hot-path by default; broad front doors/workbenches require a precise artifact/project trigger or explicit invocation.

## Per-skill/file audit questions

For each skill/file/doc/contract, answer:

1. What domain, tool, source, artifact, evidence class, failure mode, work product, or investigation standard does this skill uniquely own?
2. What would get worse if this skill or file were deleted?
3. Which sections are just runtime control flow?
4. Can the useful content be reduced to a short runtime invariant without losing domain/tool/source/artifact value?
5. Does it compete with another front door, workbench, or orchestrator?
6. Should it be hot path, explicit-only, merged, narrowed, sunk section-by-section, or deleted after delta mapping?
7. Does it specify generic verification selection/reporting, or mandatory domain/artifact/source-specific verification?
8. Does it duplicate generated host projection content instead of pointing to canonical sources?
9. If deleted or merged, where do its narrow deltas or normative claims go?
10. Does it instruct the agent to invoke, chain, or preload other skills without runtime/user selection?
11. Does it rely on source gates, authoritative systems, credentialed/private access, or forbidden substitutes?
12. Does merge/delete preserve triggerability, explicit invocation discoverability, required tools, artifact outputs, source gates, and mandatory verification?
13. Is this doc/contract the only human-readable explanation of a live machine registry or runtime invariant?
14. Is this file a fixture, baseline, generated artifact manifest entry, host adapter expectation, or compatibility pointer?
15. Does deleting it create silent compatibility fallback to retired plugin directories, `.codex` mirrors, or legacy generated surfaces?

## Audit output template

Use this table for batch audits:

| Skill/file | Current route status | File class | Unique owner statement | Narrow delta types | Overlap / competing skill | Source gates / authoritative systems / forbidden substitutes | Inbound refs checked | Canonical owner/input | Generator/regeneration path | Successor/replacement or retirement rationale | Generic sections to sink | Decision label | Concrete action | Behavioral parity / deletion evidence | Restore plan | Rationale / notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `path-or-skill` | hot / explicit / fallback / unknown | source-of-truth / generated / fixture / stale / unknown | This skill uniquely owns ... | domain / tool / source / artifact / evidence / failure / work product / investigation | `other-skill` / none | required source or none | paths/names/aliases/tests checked | canonical path/input | generator/test command | replacement path or no-successor rationale | planning, generic verification, etc. | KEEP / NARROW / MERGE / EXPLICIT_ONLY / DELETE | remove hot entry, trim body, merge to X, delete path, regenerate | triggers/tools/artifacts/source gates/tests preserved? | `git restore -- path` | why |

## Suggested phases

### Phase 1: hot path cleanup

- Remove or downgrade control skills from the canonical source that emits `SKILL_ROUTING_RUNTIME.json`.
- Keep hot path focused on hard gates: artifact, source, evidence, or explicit framework maintenance.
- Convert framework/workbench/control skills to `EXPLICIT_ONLY` where narrow deltas remain.
- Add negative routing collision scenarios before and after hot-path changes.

### Phase 2: runtime absorption

- Move generic planning, debugging, generic verification selection/reporting, delegation, clarification, and coding discipline into runtime policy as minimal invariants.
- Keep runtime policy concise; do not bloat it with language/framework-specific correctness rules, long procedures, rubrics, or examples.

### Phase 3: file/contract deletion inventory

- Inventory old files, docs, contracts, fixtures, generated projections, compatibility pointers, and legacy plugin/host artifacts proposed for deletion.
- Classify every path and block deletion for `unknown`.
- Record successor/replacement or intentional retirement rationale.
- Run reverse-reference and semantic-reference scans before deletion.
- Keep deletion diffs explicit and reviewable.

### Phase 4: skill body cleanup

- Delete or merge skills whose remaining content is only generic control flow after delta extraction and destination mapping.
- Trim retained skills so they contain only domain/tool/source/artifact/evidence/work-product-specific guidance.
- Relocate narrow deltas before deleting any control, workbench, source-gated, debugging, research, language/framework, or orchestrator skill.

### Phase 5: framework command and generated surface refresh

- Update or justify affected framework command bindings, including canonical owner, ambiguity reroute, root-cause reroute, delegation gate, verifier, execution owners, review lanes, local adaptations, and aliases.
- Update hardcoded Rust/runtime fallbacks, pinned aliases, special-case routing heuristics, and generated alias templates.
- Regenerate routing registries and generated host projections from canonical source.
- Do not hand-edit generated host projections as source of truth.
- Run policy/contract tests that cover routing, registry drift, command bindings, generated artifact drift, and host surface generation.

## Acceptance criteria

The reduction is successful when:

- Startup routing remains thin.
- No generic-control skills remain in hot routing.
- Hot skills are mostly artifact/source/evidence gates or clearly narrow owners.
- Broad front doors/workbenches are not hot-path by default and require precise artifact/project triggers or explicit invocation.
- Ordinary coding, planning, debugging, and generic verification work no longer depends on loading control skills.
- Each retained skill has a one-sentence owner statement naming a domain, tool, source, artifact, evidence class, failure mode, work product, or investigation standard.
- Each retained broad skill has been audited for narrowing and overlap.
- Each retained delta is classified by type.
- Each merge/delete has a target or rationale and a behavioral parity check.
- Each whole-skill runtime sink states why no narrow delta remains.
- Runtime owns overlay selection; skills do not self-compose route chains.
- Generated host projections remain pointers to canonical sources and are regenerated after routing changes.
- Canonical machine registries/configs are updated or regenerated from their proper source, not ignored as disposable generated files.
- Framework command bindings no longer point to removed or sunk generic-control skills unless explicitly justified.
- Every `skill_path` in hot routing and fallback manifests exists after reduction unless it is an explicit generated alias with a live generator and runtime registry owner.
- Removed/downgraded control skills do not re-enter through fallback manifest broad trigger hints.
- Old docs/contracts/files classified as stale duplicate, obsolete generated projection, compatibility pointer, historical doc, or retired contract are deleted or intentionally retained with rationale.
- No tests, configs, runtime registries, framework command bindings, host adapters, generated manifests, generated artifact declarations, or docs reference deleted docs/contracts/skills/files.
- Remaining generated projections point to canonical sources and do not duplicate normative policy.
- Retired contracts have replacement coverage or tests demonstrating the old behavior is no longer promised.
- No live source-of-truth, canonical registry/config, active contract, required fixture, intentional absence contract, active compatibility pointer, executable documentation contract, or current generated artifact manifest entry points to a deleted path.
- Every deleted old file/doc/contract has source-of-truth classification, reference-closure evidence, retained-invariant destination, affected host surface assessment, tests run, and proof that it is not a compatibility pointer, contract fixture, drift-checked generated artifact, or intentional absence contract.
- All generated surfaces regenerate cleanly from canonical inputs.
- Dependency search finds no stale references to deleted paths, aliases, registry keys, or contract names.
- Negative routing collision tests prove generic requests do not select removed/downgraded control skills through hot routing or fallback generic hints.
- For each deleted or merged skill/file/contract, a representative routing or behavior scenario proves valuable domain/tool/source/artifact/evidence/work-product behavior still activates or was intentionally retired.
- Runtime fails closed with a clear missing-retired-surface error or routes to an explicit retained owner; it must not search retired plugin directories, `.codex` mirrors, or legacy generated surfaces to recover behavior.
- Routing/contracts/tests pass after changes.

## Negative routing collision scenarios

At minimum, document or test representative prompts such as:

- `fix this bug` should not select a removed generic debugging skill unless the runtime evidence gate condition applies.
- `make a plan` should not select `idea-to-plan` unless repo-local planning work products are explicitly requested.
- `implement this spec` should not require `plan-to-code` unless a retained repo-specific spec-to-code contract applies.
- `review code` should not select `deepinterview` unless explicit deep interview/review-to-convergence behavior is requested.
- ordinary skill questions should not select `skill-framework-developer` unless framework/routing/runtime governance is the object.
- broad research requests should preserve source-quality behavior without turning `research-workbench` into a generic next-step controller.
- deleted compatibility pointers should not silently reactivate behavior through retired plugin directories, `.codex` mirrors, or legacy generated surfaces.
