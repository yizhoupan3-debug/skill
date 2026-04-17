# Approval Contract

## Goal
- Unify desktop and mobile / IM approval semantics.
- Make approval decisions derive from declarative skill metadata instead of prose-only rules.

## Source of Truth
- `skills/**/SKILL.md` frontmatter
- `skills/SKILL_APPROVAL_POLICY.json`

## Declarative Fields
- `allowed_tools`
- `approval_required_tools`
- `filesystem_scope`
- `network_access`
- `destructive_risk`
- `bridge_behavior`
- `artifact_outputs`

## Runtime Rules
1. Default to skill frontmatter, not ad-hoc prompt inference.
2. `approval_required_tools` always wins over `allowed_tools`.
3. Mobile / IM surfaces may compress wording, but must not change approval meaning.
4. `bridge_behavior: mobile_complete_once` means silent while processing, exactly one completion reply.

## Initial Middleware Behavior
- Runtime reads `skills/SKILL_APPROVAL_POLICY.json`.
- Bridge surfaces map requested actions to the same policy payload.
- Missing policy falls back to conservative approval.
