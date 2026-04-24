# Webhook Callback Implementation

Use this reference from `auth-implementation` when webhook work is primarily an
integration implementation task rather than an exploit-focused audit.

Implementation rules:

- Treat provider callbacks as machine-to-machine trust boundaries, not user auth.
- Put verification before parsing-dependent business logic.
- Keep event processing idempotent.
- Model retries and duplicate deliveries explicitly.
- Keep provider secrets server-only and out of client bundles/logs.

For exploit-focused review of an existing handler, route to
`security-audit` and its webhook security reference.
