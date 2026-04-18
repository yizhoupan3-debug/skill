---
name: observability
description: |
  Make production systems observable through logs, metrics, traces, dashboards, and alerts.
  Use when the task is telemetry design, structured logging, trace propagation, dashboard quality,
  alert noise reduction, or diagnosing whether current logs/metrics/traces are operationally useful.
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - observability
    - logging
    - metrics
    - tracing
    - opentelemetry
    - prometheus
    - grafana
risk: medium
source: local
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - observability
  - logging
  - metrics
  - tracing
  - opentelemetry
  - prometheus
---

# observability

This skill owns **telemetry design and signal quality**: logs, metrics,
traces, dashboards, alerts, and correlation strategy.

## When to use

- The user wants to add or improve logs, metrics, traces, dashboards, or alerts
- The task involves OpenTelemetry, Prometheus, Grafana, structured logging, or trace propagation
- The goal is better production visibility, lower alert noise, or better diagnostic sufficiency

## Do not use

- Sentry-only issue triage → use `$sentry`
- Primary performance remediation → use `$performance-expert`
- Server provisioning with no observability goal → use `$linux-server-ops` or `$docker`
- Error-handling architecture → use `$error-handling-patterns`
- Security-focused log review → use `$security-audit`

## Core workflow

1. Identify the system boundary and the incidents the telemetry must explain.
2. Inspect current logs, metrics, traces, dashboards, and alerts.
3. Improve:
   - signal coverage
   - structured context and correlation IDs
   - trace propagation
   - dashboard usefulness
   - alert precision
4. Validate that a likely incident can actually be traced across signals.

## Hard constraints

- Do not add telemetry without a clear operational question.
- Do not recommend high-cardinality labels casually.
- Do not log sensitive data.
- Distinguish symptom signals from root-cause hypotheses.
