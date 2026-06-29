# Cross-Skill Domain References

This directory indexes domain-calibration and cross-domain-transfer references
that span multiple skills. Each entry points to the actual file in its owning skill.

## Domain Adaptation

| Reference | Owner skill | Description |
|-----------|-------------|-------------|
| [cross-domain-transfer.md](../../skills/good-story/references/cross-domain-transfer.md) | good-story | Portable story grammar + domain calibration protocol for 6+ fields: ecology, remote sensing, AI4Science, social science, biomedical research, earth system science |
| [domain-adapters.md](../../skills/good-question/references/domain-adapters.md) | good-question | Field-specific evidence norms and common failure modes: ecology, remote sensing, machine learning, social science, biomedicine |

## Consuming Skills

These references are designed to be loaded on demand by any research skill that
needs domain-specific calibration. Typical consumers:

- `paper-workbench`: cross-field manuscript review needs knowledge of evidence norms
- `research-execution`: experiment design needs domain-specific methodology standards
- `research-discovery`: literature survey may need domain-specific gap evaluation

## Adding a New Domain Reference

1. Write the reference in its owning skill's `references/` directory
2. Add an entry to this index
3. Update the owning skill's SKILL.md "Reference Loading" section
