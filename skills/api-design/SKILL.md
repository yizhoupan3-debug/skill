---
name: api-design
description: |
  Design, review, and refactor API interfaces covering REST, GraphQL, gRPC,
  versioning, error contracts, pagination, rate limiting, and documentation.
  Use proactively when the user asks for API 设计、接口规范、版本策略、错误码设计,
  or OpenAPI/Swagger specification work.
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - api
    - rest
    - graphql
    - grpc
    - openapi
risk: low
source: local
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - API 设计
  - 接口规范
  - 版本策略
  - 错误码设计
  - OpenAPI
  - Swagger specification work
  - api
  - rest
  - graphql
  - grpc

---

# api-design

This skill owns API interface design: resource modeling, endpoint conventions, error contracts, versioning, and documentation standards.

## When to use

- Designing new REST, GraphQL, or gRPC APIs
- Reviewing API consistency, naming, and error handling
- The user wants versioning strategy or pagination design
- The user wants OpenAPI/Swagger specification writing
- Best for requests like:
  - "设计一个 RESTful API 规范"
  - "帮我写 OpenAPI spec"
  - "API 版本策略怎么做"
  - "设计统一的错误码体系"

## Do not use

- The task is backend service implementation (routing, handlers) → `$node-backend`
- The task is database schema design → `$sql-pro`
- The task is system architecture → `$architect-review`

## Task ownership and boundaries

This skill owns:
- API resource modeling and endpoint design
- request/response contract design
- error code and status code conventions
- pagination, filtering, and sorting patterns
- versioning strategy
- rate limiting and throttling policy
- OpenAPI / Swagger specification

This skill does not own:
- backend service implementation code
- **Dual-Dimension Audit (Pre: Schema/Spec, Post: Spec-Accuracy/Client Results)** → runtime verification gate
- database schema design
- authorization implementation
- infrastructure and deployment

## Required workflow

1. Confirm the task shape:
   - object: API, endpoint, resource, error contract, spec document
   - action: design, review, refactor, document, version
   - constraints: protocol (REST/GraphQL/gRPC), consumers, backward compatibility
   - deliverable: API design, specification, or review guidance
2. Identify API consumers and constraints.
3. Design resource model before endpoint details.
4. Define error contracts consistently.
5. Document with OpenAPI or equivalent.

## Core workflow

### 1. Intake
- Identify use case: internal service, public API, BFF, mobile backend.
- Check existing API conventions and patterns.
- Understand consumer requirements and backward compatibility needs.

### 2. Design
- Model resources and their relationships.
- Design endpoints following naming conventions.
- Define consistent error response structure.
- Plan pagination, filtering, and sorting.
- Design authentication and authorization requirements.
- Plan versioning strategy.

### 3. Validation / recheck
- Verify naming consistency across all endpoints.
- Check for breaking changes against existing consumers.
- Validate OpenAPI spec with a linter.
- Review error codes for completeness and consistency.

## Capabilities

### REST Design
- Resource-oriented URL design
- HTTP method semantics (GET, POST, PUT, PATCH, DELETE)
- Status code conventions (2xx, 4xx, 5xx)
- HATEOAS and hypermedia patterns
- Content negotiation and headers

### GraphQL Design
- Schema design and type system
- Query and mutation conventions
- Subscription patterns
- N+1 prevention and DataLoader
- Schema stitching / federation

### gRPC Design
- Protobuf schema design
- Service method patterns (unary, streaming)
- Error model (google.rpc.Status)
- Metadata and interceptors

### API Patterns
- Pagination: cursor-based, offset-based, keyset
- Filtering: query parameters, GraphQL filters
- Rate limiting: token bucket, sliding window
- Idempotency keys and retry safety
- Bulk operations and batch endpoints
- Webhook design and delivery guarantees

### Versioning
- URL versioning (`/v1/`, `/v2/`)
- Header versioning
- Query parameter versioning
- Sunset and deprecation policies
- Backward compatibility rules

### Documentation & Tooling
- OpenAPI 3.1 specification
- JSON Schema for request/response
- API changelog and migration guides
- Mock server generation
- SDK generation from specs

## Output defaults

Recommended structure:

````markdown
## API Design Summary
- Protocol: REST / GraphQL / gRPC
- Audience: internal / public / BFF
- Versioning: ...

## Resource Model
- ...

## Endpoint Design
- ...

## Error Contract
- ...

## OpenAPI Spec (if applicable)
- ...
````

## Hard constraints

- Do not use verbs in REST endpoint paths (use nouns).
- Do not return 200 for errors; use appropriate HTTP status codes.
- Do not design breaking changes without a versioning/migration plan.
- Always include a consistent error response structure.
- Do not expose internal IDs or implementation details in public APIs.
- Always document rate limits and pagination behavior.
- **Superior Quality Audit**: For API contracts, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples

- "Use $api-design to design a RESTful API for a SaaS product."
- "设计一个统一的 API 错误码体系。"
- "帮我写 OpenAPI 3.1 specification。"
- "这个 API 的版本策略怎么做？"
- "强制进行 API 设计深度审计 / 检查 Schema 定义与文档一致性结果。"
- "Use the runtime verification gate to audit this API design for contract idealism."
