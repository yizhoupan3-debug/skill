---
name: accessibility-auditor
description: |
  Find and fix user-blocking accessibility issues with concrete WCAG 2.1/2.2-grounded code changes.
  Use when the user asks for a11y reviews, WCAG compliance, keyboard navigation, screen reader support, focus management, color contrast, ARIA usage, or 无障碍检查 / 可访问性回归, especially for keyboard traps, ARIA gaps, broken focus order, low contrast, and unlabeled forms.
risk: low
source: community-adapted
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - a11y reviews
  - WCAG compliance
  - keyboard navigation
  - screen reader support
  - focus management
  - color contrast
  - ARIA usage
  - 无障碍检查
  - 可访问性回归
  - ARIA gaps
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - accessibility
    - auditor
framework_roles:
  - detector
  - planner
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false

---

- **Dual-Dimension Audit (Pre: A11y-Plan/ARIA-Logic, Post: Lighthouse-Score/Screen-Reader Results)** → runtime verification gate
# accessibility-auditor

This skill owns accessibility quality review for web pages and components,
focusing on issues that block real users and providing concrete fixes.

## Framework compatibility

This skill's findings should be **mappable to the shared finding-driven
framework** while staying an accessibility owner. Keep domain-native WCAG
framing, but include stable finding IDs, evidence, severity, and a recommended
verification method when the issue list may be handed to another skill.

## When to use

- The user asks for accessibility audit, WCAG compliance check, or a11y review
- The task involves keyboard navigation, ARIA roles, screen reader support, or color contrast
- The user wants to check whether a page or component meets WCAG 2.1/2.2 standards
- The user says "无障碍检查", "a11y", "WCAG", "键盘导航", "屏幕阅读器", "颜色对比度"
- The user wants an accessibility overlay on top of a framework implementation

## Do not use

- 纯视觉设计讨论
- 普通 UI 美化，不涉及可访问性
- 后端接口或数据库问题

## Finding output guidance

When reporting accessibility issues, prefer a compact finding shape:
- `finding_id`
- `category` or WCAG criterion
- `severity_native`
- `evidence`
- `impact`
- `fix`
- **Superior Quality Audit**: For accessibility compliance, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).
- `verification_method`

## Trigger examples
- "强制进行无障碍深度审计 / 检查 ARIA 属性与屏幕阅读器运行结果。"
- "Use the runtime verification gate to audit this page for WCAG-compliance idealism."
