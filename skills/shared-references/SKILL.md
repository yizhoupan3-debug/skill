---
allowed_tools:
- Read
- Bash
- WebFetch
description: Shared reference library — cross-skill lookup tables, glossary, domain constants, and reference data. Not a standalone skill; consumed by other skills via `skills/shared-references/*`.
metadata:
  platforms:
  - supported
---

# shared-references

Cross-cutting reference data used by multiple skills: domain constants,
standard glossaries, lookup tables, and citation formatting rules.

Not designed for direct invocation. Other skills reference entries via
relative paths under this directory.
