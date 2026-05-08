# Runtime Plugin Contract

## Purpose

This document freezes the first plugin ABI for the skill runtime. The goal is
to keep the Rust control plane stable while letting skills, framework commands,
storage backends, route policies, host projections, and execution delegates
evolve as replaceable components.

## Contract Rules

- The Rust runtime remains the control-plane authority.
- Plugin records are declarations, not executable code.
- Unknown capability classes must fail closed.
- New plugin fields must be additive or versioned.
- Legacy `SKILL_ROUTING_RUNTIME.json` row consumers remain supported while
  vNext consumers migrate to named `records`.

## Plugin ABI

Every plugin record must expose:

- `slug`: stable plugin identity.
- `kind`: `skill`, `framework_command`, or a future versioned kind.
- `skill_path`: repo-relative entrypoint path when a skill body exists.
- `entrypoint`: the runtime-facing entrypoint class.
- `capabilities`: declared routing, tool, artifact, network, and gate surface.
- `risk`: priority, approval, and destructive-risk projection.
- `host_support`: supported host projections.
- `lifecycle`: status and source lineage.

The generated source of truth is `skills/SKILL_PLUGIN_CATALOG.json`.

## Routing Metadata ABI

Skill-specific routing behavior should move out of Rust hardcoding and into
declarations:

- `intent_tags`: normalized owner, gate, session, and domain tags.
- `positive_triggers`: trigger hints that raise confidence.
- `negative_triggers`: do-not-use signals that lower confidence.
- `gate_policy`: gate-before-owner behavior.
- `overlay_policy`: primary vs overlay eligibility.
- `fallback_policy`: runtime, explicit, or manifest-fallback eligibility.
- `selection_reason`: why the skill is or is not in the hot runtime.

The generated source of truth is `skills/SKILL_ROUTING_METADATA.json`.
The current router consumes declarative `negative_triggers` from runtime records
and the sidecar metadata file, then applies them through the existing
do-not-use penalty path.

## Runtime vNext Projection

`skills/SKILL_ROUTING_RUNTIME.json` keeps the existing `version = 3` row shape
for compatibility and adds named `records` for plugin-ready consumers.

This lets the runtime migrate in three safe steps:

1. Keep legacy `keys` and `skills` rows as the canonical live input.
2. Add named records and validate them against the plugin catalog.
3. Move strict consumers to records after route parity is proven by evals.

## Health Loop

`skills/SKILL_HEALTH_MANIFEST.json` is the deterministic closed-loop health
surface. It must check at least:

- skill path declaration safety
- plugin record presence
- routing metadata presence
- trigger surface quality

Physical path loadability remains covered by policy tests so this generated
manifest can stay byte-for-byte reproducible in temporary regeneration roots.
Future checks should add eval drift, route mismatch rate, plugin capability
unknowns, and host projection drift without replacing the existing checks.

## Provider Registry

`configs/framework/RUNTIME_PROVIDER_REGISTRY.json` is the component-level
provider registry for runtime lanes that are not individual skills. It declares
execution providers, storage backends, trace/replay providers, observability
SLO loops, sandbox profiles, host projections, and governance eval loops.

The registry is intentionally declarative:

- implemented providers describe the current Rust-owned surfaces
- declared providers define stable future plugin slots
- planned providers reserve extension points without changing the live kernel
- every provider path or input must stay repo-relative or logical
