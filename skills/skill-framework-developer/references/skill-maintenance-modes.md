# Skill Maintenance Modes

`skill-framework-developer` owns local skill-system governance. Use these modes
instead of routing to separate maintenance skills.

## single-skill wording pass

Use for one `SKILL.md` when the boundary is already known and the work is wording,
description budget, trigger hints, or progressive disclosure. Keep the top-level
file short and move long examples to `references/`.

## batch wording normalization

Use for many skill files that need consistent frontmatter, trigger specificity,
or repeated boilerplate removal. Do not rewrite content for style alone; every
edit should reduce routing ambiguity or loading cost.

## miss repair

Use after a concrete route miss. Capture the failed prompt, expected owner,
actual owner, smallest safe repair, and regression test. Prefer changing the
incumbent owner before creating a new sibling skill.

## external scout

Use when external skill ecosystems are being benchmarked to improve this local
framework. The output is local framework guidance, not a generic research memo.
