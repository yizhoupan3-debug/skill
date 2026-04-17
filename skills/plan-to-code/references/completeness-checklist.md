# Completeness Checklist

Use this checklist when translating a plan into "complete" code rather than a narrow patch.

## Core path

- Is there a clear entrypoint from user action or external input to final behavior?
- Are data structures, types, schemas, and contracts updated consistently?
- Are server logic, client logic, and persistence layers wired together where applicable?

## Integration

- Are routes, exports, dependency injection, registries, or module indexes updated?
- Are config, environment variables, permissions, or feature flags added if required?
- Are migrations, fixtures, or seed data included when the feature depends on stored data?

## Product behavior

- Are success, loading, empty, and error states handled?
- Are validation rules implemented on the correct layer?
- Are obvious edge cases covered so the feature does not fail on first real use?

## Quality

- Are tests added or updated at the most valuable layer?
- Did typecheck, lint, build, or focused tests run where feasible?
- Are comments and docs limited to what future maintainers actually need?

## Delivery

- Avoid TODO placeholders unless explicitly requested.
- Avoid mock paths presented as complete implementations.
- Report what was verified and what remains unverified.
