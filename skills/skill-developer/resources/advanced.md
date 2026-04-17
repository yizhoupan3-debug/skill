# Advanced Patterns For `skill-developer`

Use this note when the base `SKILL.md` is not enough and the task is specifically about refining an Antigravity skill library.

## Description Upgrade Pattern

1. Start with the domain noun the user would actually say.
2. Add exact trigger verbs such as `debug`, `audit`, `rewrite`, `split`, `merge`, or `validate`.
3. Add runtime nouns like `Antigravity`, `SKILL.md`, `frontmatter`, `activation`, or `workspace corpus`.
4. End with an explicit trigger phrase like `Use PROACTIVELY when...`.

## Split vs. Merge Questions

Ask:
- Do both skills depend on the same runtime behavior?
- Would the same description fire correctly for both?
- Does one variant need different exclusions or validation steps?

If the answer is "no" to any of those, split the skill.

## Review Checklist

- `name` is stable and normalized
- `description` is dense and specific
- `When to use` contains explicit user-facing scenarios
- `Do not use` narrows false positives
- The body stays under the 500-line rule
- All linked resources exist
