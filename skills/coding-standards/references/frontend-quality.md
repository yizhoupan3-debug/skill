# Frontend Code Quality Reference

Use this reference from `coding-standards` when frontend component quality is an
overlay, not the primary owner.

Rules:

- Keep components small enough to scan; extract only when reuse or clarity demands it.
- Prefer explicit props and concrete types over loose bags or `any`.
- Use early returns for loading, empty, and error states.
- Keep event handlers named for intent.
- Avoid speculative hooks, providers, and configuration layers.
- Respect the framework owner (`react`, `nextjs`, `vue`, `svelte`) for stack-specific semantics.
