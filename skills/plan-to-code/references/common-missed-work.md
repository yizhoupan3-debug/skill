# Common Missed Work

Use this checklist before concluding that an implementation is "complete". It focuses on the work that is often skipped even when the core feature appears done.

## Registration and wiring

- New page or screen exists, but route registration, navigation entry, or menu link is missing.
- New backend handler exists, but router, controller map, DI container, or export barrel is not updated.
- New job or worker exists, but scheduler, queue binding, trigger point, or retry configuration is not connected.
- New component or utility exists, but no real caller reaches it.

## Data and contracts

- Frontend field changed, but API contract, backend validation, type definitions, or schema did not change with it.
- Database model changed, but migration, seed data, backfill, or read/write queries were not updated consistently.
- Response shape changed, but downstream callers, serializers, or client adapters still assume the old shape.
- Optional fields, nullability, defaults, and legacy data compatibility were not checked.

## Product states

- Happy path works, but loading, empty, error, disabled, or success-feedback states are missing.
- Form submits once, but duplicate submit protection, retry behavior, and validation feedback are missing.
- A list renders data, but pagination, filtering, sorting, or no-results behavior now breaks.
- A destructive action works, but confirmation, undo path, or failure recovery is missing when expected by the product.

## Permissions and safety

- Admin path works, but normal user, read-only user, or unauthenticated behavior is undefined.
- UI hides an action, but backend authorization does not enforce the same rule.
- New config or secret is required, but missing-config behavior is not handled cleanly.
- Feature flag gating is added in one layer only.

## Integration and compatibility

- UI is connected to mocks or static data while the feature is presented as complete.
- API works in isolation, but the real client flow does not call it correctly.
- Existing entrypoints, old consumers, or adjacent flows regress because shared logic changed.
- Mobile layout, SSR, caching, i18n, or background refresh behavior was affected but not rechecked.

## Observability and ops

- Error path exists, but logs, surfaced messages, or debugging breadcrumbs are too weak to support real use.
- Migration or job behavior changed, but rollout, retry, or idempotency concerns were ignored.
- New external dependency was introduced without timeout, fallback, or failure handling.

## Tests and delivery

- Core code changed, but the highest-value test layer was not updated.
- Build or typecheck was skipped even though contracts or imports changed.
- Documentation, sample env, or operator notes are missing when the change introduces setup requirements.
- The final delivery note says "done" without saying what was verified and what remains unverified.

## Red flags

Treat the implementation as incomplete if any of these are true:

- There is no real user path from entrypoint to outcome.
- The feature only works with hardcoded or mock data unless the repository explicitly expects that.
- The code compiles in isolation but is not registered into the running system.
- Only the happy path exists and the first obvious failure mode is unhandled.
