---
name: sql-pro
description: |
  Write, optimize, debug, and review SQL for PostgreSQL, MySQL, SQLite, and
  analytical warehouses with focus on schema design, joins, aggregations,
  indexing, EXPLAIN-driven tuning, and OLTP/OLAP query patterns. Use proactively
  when the user asks for 复杂 SQL、慢查询优化、报表分析、数据建模、索引设计, or
  database-side fixes that need SQL-specific engineering judgment.
metadata:
  version: "2.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - sql
    - postgresql
    - mysql
    - indexing
    - query-optimization
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 复杂 SQL
  - 慢查询优化
  - 报表分析
  - 数据建模
  - 索引设计
  - sql
  - postgresql
  - mysql
  - indexing
  - query optimization
---

# sql-pro

This skill owns SQL-first engineering work: query design, schema modeling, index strategy, and EXPLAIN-driven performance tuning.

## When to use

- Writing complex SQL queries or analytics
- Tuning query performance with indexes or plans
- Designing SQL patterns for OLTP/OLAP workloads
- Schema design, normalization/denormalization decisions
- Best for requests like:
  - "这个慢查询怎么优化"
  - "帮我设计索引策略"
  - "写一个复杂的报表 SQL"
  - "数据库 Schema 怎么设计"

## Do not use

- The task is ORM-level code rather than raw SQL → use `$python-pro` or `$node-backend`
- The system is non-SQL or document-only (MongoDB, DynamoDB)
- The task is database migration tooling rather than SQL design → use `$build-tooling`
- The task is full backend service architecture → use `$node-backend` or `$architect-review`

## Task ownership and boundaries

This skill owns:
- SQL query design and optimization
- schema modeling and normalization
- index strategy and EXPLAIN analysis
- OLTP/OLAP pattern trade-offs
- analytical query techniques (window functions, CTEs, aggregations)

This skill does not own:
- ORM configuration or code-level DB integration
- NoSQL/document database design
- database migration tooling
- backend service architecture
- **Dual-Dimension Audit (Pre: Plan/Syntax, Post: Dataset-Result/Performance)** → `$execution-audit-codex` [Overlay]

If the task shifts to adjacent skill territory, route to:
- `$python-pro` (SQLAlchemy, Django ORM)
- `$node-backend` (Prisma, Drizzle, TypeORM)
- `$architect-review` (data architecture decisions)

## Required workflow

1. Confirm the task shape:
   - object: query, schema, index, stored procedure, migration, view
   - action: write, optimize, debug, review, redesign
   - constraints: RDBMS type, data volume, latency targets, access patterns
   - deliverable: SQL code, schema design, optimization plan, or index strategy
2. Identify the target RDBMS and verify syntax compatibility.
3. Inspect schema, statistics, and access paths before optimizing.
4. Validate with EXPLAIN and verify correctness.

## Core workflow

### 1. Intake
- Identify RDBMS type (PostgreSQL, MySQL, SQLite, BigQuery, etc.).
- Understand the data model, table sizes, and access patterns.
- Check existing indexes and constraints before suggesting changes.

### 2. Execution
- Write queries that are readable and performant.
- Use CTEs for readability; inline when performance requires it.
- Design indexes that match actual query patterns, not hypothetical ones.
- Prefer set-based operations over row-by-row processing.
- Use window functions for analytical queries instead of self-joins.

### 3. Validation / recheck
- Run EXPLAIN (ANALYZE) on the optimized query.
- Compare execution plans before and after changes.
- Verify correctness with sample data or edge cases.
- Check for index bloat, sequential scan fallbacks, or plan regressions.

## Capabilities

### Modern Database Systems
- Cloud-native: Amazon Aurora, Google Cloud SQL, Azure SQL Database
- Data warehouses: Snowflake, BigQuery, Redshift, Databricks
- Hybrid OLTP/OLAP: CockroachDB, TiDB, MemSQL
- Time-series: InfluxDB, TimescaleDB
- Modern PostgreSQL features and extensions

### Advanced Query Techniques
- Complex window functions and analytical queries
- Recursive CTEs for hierarchical data
- Advanced JOIN techniques and optimization
- Query plan analysis and execution optimization
- Parallel query processing and partitioning
- JSON/XML data processing

### Performance Tuning
- Comprehensive index strategy design
- Query execution plan analysis
- Partitioning strategies for large tables
- Memory configuration and buffer pool tuning
- I/O optimization and storage considerations

### Data Modeling and Schema Design
- Advanced normalization and denormalization
- Dimensional modeling (star schema, snowflake schema)
- Slowly Changing Dimensions (SCD)
- Data vault modeling
- Event sourcing and CQRS patterns

### Analytics and Business Intelligence
- OLAP cube design and advanced aggregations
- Time-series analysis and forecasting
- Cohort analysis and customer segmentation
- Real-time analytics and streaming data

## Output defaults

Default output should contain:
- RDBMS context and schema assumptions
- query/schema approach
- EXPLAIN results and validation notes

Recommended structure:

````markdown
## SQL Summary
- RDBMS: ...
- Target table(s): ...

## Query / Schema Changes
- ...

## EXPLAIN Analysis
- Before: ...
- After: ...

## Validation / Risks
- Checked: ...
- Data volume considerations: ...
````

## Hard constraints

- Do not write SQL that is syntactically valid only for one RDBMS without flagging.
- Do not suggest indexes without considering write overhead and table size.
- Do not optimize without running or reviewing EXPLAIN output.
- Do not use vendor-specific extensions without noting portability impact.
- Avoid heavy queries on production without safeguards (LIMIT, read replica, etc.).
- Do not denormalize without explicitly stating the trade-off.
- Prefer standard SQL when the RDBMS supports it over proprietary syntax.
- **Superior Quality Audit**: For production or complex analytical SQL, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples

- "Use $sql-pro to optimize this slow query with EXPLAIN analysis."
- "帮我优化这个慢查询，看下执行计划。"
- "设计一个 star schema 的数据仓库模型。"
- "这个 PostgreSQL 索引策略对不对？"
- "强制进行 SQL 深度审计 / 检查执行计划与结果数据准确性。"
- "Use $execution-audit-codex to audit this SQL query for execution-plan idealism."
