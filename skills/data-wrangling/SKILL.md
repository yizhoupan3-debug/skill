---
name: data-wrangling
description: |
  Clean, transform, validate, and pipeline structured or semi-structured data
  across CSV, JSON, XML, YAML, Parquet, and custom text.
  Use when the user asks to 清洗数据, 转换数据格式, 写 ETL 脚本, 做正则提取, 做 schema
  mapping, 数据去重, 数据校验, parse 非标格式, or build a cleaning pipeline before
  analysis or storage. Best for data wrangling, not SQL optimization,
  ORM/cache/queue runtime issues, or full ML feature engineering.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - data-cleaning
    - etl
    - regex
    - csv
    - json
    - xml
    - schema-mapping
    - data-validation
    - format-conversion
risk: low
source: local
---

# data-wrangling

This skill owns data cleaning, transformation, validation, and format conversion
tasks when the primary object is tabular or semi-structured data and the primary
action is cleaning, reshaping, or piping it through a processing chain.

## When to use

- The user needs to clean, normalize, deduplicate, or validate a dataset
- The task involves parsing, extracting, or converting between CSV, JSON, XML, YAML, Parquet, or custom text formats
- The task involves regex-based extraction or pattern matching on data fields
- The user is building an ETL pipeline, data-loading script, or schema-mapping layer
- The task involves type coercion, missing-value imputation, encoding fixes, or column renaming/reordering
- Best for requests like:
  - "帮我清洗这个 CSV，去重、补缺失值、统一日期格式"
  - "写个脚本把这些 JSON 转成标准化的 CSV"
  - "用正则从日志里提取结构化字段"
  - "做个 ETL pipeline 把多源数据合并成统一 schema"
  - "校验数据是否符合这个 schema 定义"

## Do not use

- The task is SQL query writing, optimization, or index tuning → use `$sql-pro`
- The task is Redis/cache/queue/ORM runtime behavior → use `$datastore-cache-queue`
- The task is ML feature engineering coupled to models (embeddings, tokenization, augmentation) → use `$ai-research` or `$mac-memory-management` when Mac memory/runtime constraints dominate
- The task is Jupyter notebook creation as a container → use `$jupyter-notebook`
- The task is build/bundler/toolchain problems → use `$build-tooling`
- The data is already in a database and the work is purely SQL → use `$sql-pro`
- The main goal is to speed up code via hot-path rewrites or faster-library swaps such as pandas → polars → use `$code-acceleration`

## Task ownership and boundaries

This skill owns:
- Data format parsing and conversion (CSV ↔ JSON ↔ XML ↔ YAML ↔ Parquet ↔ custom)
- Regex-based field extraction and pattern matching
- Schema mapping, column renaming, type coercion, encoding normalization
- Deduplication, missing-value handling, outlier flagging
- Lightweight data validation against schema definitions (JSON Schema, pydantic, Zod, etc.)
- ETL script/pipeline design and implementation

This skill does not own:
- Database query optimization or indexing
- ORM/cache/queue runtime behavior
- ML model training or statistical analysis
- Build toolchain issues
- Full-stack API design
- **Dual-Dimension Audit (Pre: Mapping/Logic, Post: Schema-Match/Row-Count Results)** → `$execution-audit-codex` [Overlay]

## Core workflow

### 1. Intake

- Identify the source format, target format, volume, and quality issues
- Sample a representative slice of the data to inspect structure, types, and anomalies
- Clarify the schema contract: what fields, types, constraints, and invariants the output must satisfy

### 2. Execution

- Parse input data with appropriate tools (pandas, polars, csv, jq, xmltodict, PyYAML, etc.)
- Apply cleaning operations in a deterministic, reproducible order
- Validate intermediate results against schema expectations after each major transformation
- Handle edge cases explicitly: NaN/null semantics, encoding, timezone, locale
- Write idempotent scripts that can re-run safely on updated source data

### 3. Validation

- Verify row counts, null rates, type distributions before and after transformation
- Run schema validation on the final output
- Spot-check a sample of records for correctness
- Document any data dropped, modified, or imputed with rationale

## Output defaults

Finish with a `Data Wrangling Summary` covering: source (format/size/issues), target (format/schema/destination), transformations applied, validation results (row count before→after, null rate, schema PASS/FAIL), edge cases and decisions.

## Cross-references

- `$ai-research` handles model-coupled feature engineering; this skill handles format-level wrangling
- `$sql-pro` handles database queries; this skill handles file-based data transformation
- `$jupyter-notebook` may invoke this skill for data loading and cleaning steps

## Hard constraints

- Never silently drop rows without documenting the reason and count
- Always preserve original data or provide a rollback path
- Prefer explicit type coercion over implicit casting
- Always validate output against the stated schema before declaring done
- If data volume exceeds what can be processed in memory, say so and suggest chunked/streaming approaches
- **Superior Quality Audit**: For production ETL pipelines, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples
- "强制进行数据清洗审计 / 检查 Schema 校验结果与数据丢弃报告。"
- "Use $execution-audit-codex to audit this ETL pipeline for row-count idealism."
