# Webhook Security Reference

Use this reference from `security-audit` when reviewing webhook receivers or
callback endpoints for providers such as Stripe, GitHub, Slack, Clerk, and other
SaaS integrations.

Checklist:

- Verify signatures before business logic.
- Preserve the raw body when the provider signs raw payload bytes.
- Use timing-safe comparisons where manual signature checks are required.
- Enforce timestamp tolerance or replay windows when the provider supports them.
- Make side effects idempotent by storing event or delivery IDs.
- Ack quickly and move heavy work to queues or background jobs.

Provider notes:

- Stripe: prefer official SDK verification such as `constructEvent`.
- GitHub: verify `x-hub-signature-256`.
- Slack: verify timestamp plus signature base string.
