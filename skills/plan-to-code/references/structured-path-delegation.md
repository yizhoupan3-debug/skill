# Structured-Path Delegation

Use this reference when `plan-to-code` owns the content work and
`$subagent-delegation` is being consulted for runtime execution.

## Ownership split

`plan-to-code` keeps ownership of:

- interpreting the plan/spec
- deciding execution slices
- choosing slice order
- deciding what "complete" means
- final integration judgment

`$subagent-delegation` owns:

- whether delegation is worthwhile
- which sidecars to spawn
- which role each sidecar gets
- when the main thread should wait

## Recommended pattern

Main thread acts as **controller**:

1. read and classify the brief
2. inspect the repo
3. write the compact execution map
4. decide which slices remain local
5. delegate only bounded sidecars
6. review and integrate results

Useful prompt templates for this pattern:

- [delegation-prompts.md](delegation-prompts.md)
  when dispatching structured-path sidecar reviewers or workers.

Subagents act as **slice workers or sidecar reviewers** when runtime policy permits; otherwise preserve the same slice boundaries in a local-supervisor queue:

- read-only exploration
- bounded implementation slice
- targeted verification or review pass

## Good delegation targets

- a read-only codebase exploration that does not block local planning
- a bounded implementation slice with a disjoint write scope
- a focused verification pass after a local or delegated slice lands
- a spec-compliance review against a clearly described slice

## Bad delegation targets

- "implement the whole spec"
- "figure out the whole repo and do whatever is needed"
- multiple workers editing the same files without explicit ownership
- asking a worker to infer the slice boundaries that the controller has not defined

## Minimal contract per delegated slice

- slice name
- exact goal
- relevant files or write boundary
- acceptance condition
- verification expectation

If you cannot state that contract clearly, the slice is not ready to delegate.
