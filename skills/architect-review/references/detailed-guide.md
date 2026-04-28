# architect-review — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- system architecture evaluation and ADRs
- module boundaries and service decomposition
- scalability and resilience analysis
- technology selection tradeoffs
- architectural drift detection

This skill does not own:
- local code style enforcement → `$coding-standards`
- security implementation details → `$security-audit`
- specific framework implementation → use framework skills
- database query optimization → `$sql-pro`

If the task shifts to adjacent skill territory, route to:
- `$coding-standards` for code-level conventions
- `$security-audit` for implementation security
- `$performance-expert` for runtime performance
- `$node-backend` for backend service implementation

## Required workflow

1. Gather system context, goals, and constraints.
2. Evaluate architectural decisions and identify risks.
3. Propose improvements with tradeoff analysis and next steps.
4. Document decisions and follow up with verification.

## Core workflow

### 1. Intake
- Understand the system's current state and target state.
- Identify the architectural scope (new design, migration, refactor, review).
- Check existing ADRs, diagrams, and documentation.

### 2. Evaluation
- Assess pattern conformance (Clean Architecture, DDD, event-driven, etc.).
- Identify architectural violations and anti-patterns.
- Evaluate quality attributes: reliability, scalability, maintainability, testability.
- Check module boundaries and coupling.

### 3. Recommendation
- Propose improvements with concrete refactoring suggestions.
- Include tradeoff analysis for each recommendation.
- Consider scalability impact for future growth.
- Document decisions in ADR format when appropriate.

## Capabilities

### Architecture Patterns
- Clean Architecture and Hexagonal Architecture
- Microservices with proper service boundary design
- Event-driven architecture (EDA) with Event Sourcing and CQRS
- Domain-Driven Design (DDD) with bounded contexts
- Serverless patterns and FaaS design
- API-first design (GraphQL, REST, gRPC)

### Distributed Systems
- Service Mesh (Istio, Linkerd, Consul Connect)
- Event streaming (Kafka, Pulsar, NATS)
- Distributed data patterns: Saga, Outbox, Event Sourcing
- Resilience patterns: circuit breaker, bulkhead, timeout
- Distributed tracing and observability

### Design Principles
- SOLID principles
- Repository, Unit of Work, Specification patterns
- Anti-Corruption Layer and Adapter patterns
- Dependency injection and IoC

### Cloud Native
- Kubernetes and container orchestration
- Infrastructure as Code (Terraform, Pulumi)
- GitOps and CI/CD pipeline architecture
- Multi-cloud and hybrid cloud strategies

### Data Architecture
- Polyglot persistence (SQL + NoSQL)
- Data lake, data warehouse, data mesh
- Database-per-service isolation
- Eventual consistency and distributed transactions

## Output defaults

Default output should contain:
- architecture context and current state
- evaluation findings with impact ratings
- recommendations with tradeoffs

For framework-compatible output, include:
- `finding_id`
- `category`
- `severity_native`
- `impact`
- `recommended_next_step`
- `verification_method`

Recommended structure:

````markdown
## Architecture Review Summary
- Scope: ...
- Impact: High / Medium / Low

## Findings
1. `AR-001`: [Short title of finding]
   - **Category**: [e.g., boundaries, scalability, resilience]
   - **Severity**: [High/Medium/Low]
   - **Impact**: [What happens if not fixed]
   - **Recommendation**: [Concrete next step]
   - **Verification**: [How to prove it's fixed]

## Recommendations
- ...
- Tradeoff: ...

## ADR (if applicable)
- Decision: ...
- Context: ...
- Consequences: ...
````

## Hard constraints

- Do not approve high-risk changes without a verification plan.
- Document assumptions and dependencies to prevent regression.
- Do not propose over-engineered solutions without justification.
- Always include tradeoff analysis for each recommendation.
- Prefer evolutionary architecture over big-bang rewrites.
- Do not skip quality attribute evaluation.

## Integration notes

In PLANNING mode:
- Highlight architectural risks and alternatives in implementation_plan.md
- Attach tradeoff analysis to design decisions
- Use Mermaid diagrams (via `$diagramming`) to visualize architecture

In EXECUTION mode:
- Verify new code follows established architectural conventions
- Check that new dependencies align with architectural direction
- Watch for architecture drift

## Trigger examples

- "Use $architect-review to review this microservice boundary design."
- "审查这个微服务设计的限界上下文边界。"
- "评估引入 Event Sourcing 的架构影响。"
- "帮我写一个关于这次技术选型的 ADR。"
