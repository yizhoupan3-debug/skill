# Verification Matrix

Use this matrix to choose the minimum credible verification set after implementation. Run the highest-signal checks that match the repository and changed layers.

## Baseline

Always prefer checks that exercise the changed path directly. If time or tooling is limited, run fewer high-signal checks rather than many weak ones.

Always report:

- what ran
- what passed or failed
- what was not run

## By repository type

### Frontend application

Run as applicable:

- lint for changed packages or the app
- typecheck
- production build
- focused component, page, or integration tests
- manual check of loading, error, and empty states if the UI changed

Prioritize:

- build and typecheck when routing, bundling, or app wiring changed
- focused tests when business logic moved into hooks, stores, or components

### Backend service or API

Run as applicable:

- lint
- typecheck or static analysis
- unit tests for touched services, handlers, or utilities
- integration or API tests for changed endpoints
- local request smoke test when a route, auth rule, or serialization changed

Prioritize:

- endpoint-level verification when request/response contracts changed
- integration tests when persistence, queues, or external services are involved

### Full-stack application

Run as applicable:

- frontend build and typecheck
- backend typecheck and tests
- integration path covering the changed user flow end to end
- smoke test for route-to-database or route-to-client wiring

Prioritize:

- at least one end-to-end or equivalent manual path when both UI and server changed

### Library or shared package

Run as applicable:

- lint
- typecheck
- unit tests
- consumer package build or test if downstream contracts changed

Prioritize:

- contract tests or representative usage tests when exported APIs changed

### CLI or automation tool

Run as applicable:

- lint
- typecheck
- unit tests
- one or more real command invocations against sample input
- help or usage output check if flags changed

Prioritize:

- sample execution over unit-only confidence when I/O behavior changed

## By changed layer

### Schema or migration changes

Add as applicable:

- migration generation or application check
- schema validation
- tests covering read and write paths affected by the schema

### Config or environment changes

Add as applicable:

- startup or boot smoke test
- failure-path check when required config is missing
- docs or sample env update review

### Auth, permissions, or roles

Add as applicable:

- allowed-role check
- denied-role check
- regression check for adjacent protected routes or actions

### Async jobs, queues, or schedulers

Add as applicable:

- job enqueue trigger check
- worker execution test or dry run
- idempotency or retry-path check when relevant

### UI state changes

Add as applicable:

- loading state
- empty state
- error state
- success feedback

## Fallback rule

If the full ideal matrix is too expensive, run one direct behavior check plus one static confidence check:

- direct behavior check: focused test, sample invocation, endpoint smoke test, or manual flow
- static confidence check: typecheck, build, or lint

Do not claim the feature is fully verified if neither was run.
