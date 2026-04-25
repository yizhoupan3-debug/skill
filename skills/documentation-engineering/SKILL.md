---
name: documentation-engineering
description: |
  Write, review, and maintain project documentation such as README, API docs,
  ADRs, changelogs, onboarding guides, and docstrings.
  Use when the user asks to 写 README, 补 API 文档, 写 ADR, 生成 changelog,
  写 onboarding guide, 审查文档完整性, or set up documentation-as-code or doc
  generation pipelines. Best for project/code docs, not papers or Word/PDF
  artifact editing.
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 补 API 文档
  - 写 ADR
  - 生成 changelog
  - 写 onboarding guide
  - 审查文档完整性
  - set up documentation-as-code
  - doc generation pipelines
  - documentation
  - readme
  - api docs
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - documentation
    - readme
    - api-docs
    - adr
    - changelog
    - jsdoc
    - docstring
    - onboarding
    - doc-generation
risk: low
source: local
---

- **Dual-Dimension Audit (Pre: Doc-Structure/Logic, Post: Links-Freshness/Completeness Results)** → `$execution-audit` [Overlay]

# documentation-engineering

This skill owns project and code documentation: writing, reviewing, structuring,
and automating developer-facing documentation artifacts.

## When to use

- The user wants to write or improve a README, contributing guide, or onboarding document
- The task involves API documentation (JSDoc, docstrings, rustdoc, TypeDoc, Swagger/OpenAPI docs)
- The user wants to set up or maintain architecture decision records (ADRs)
- The task involves changelog generation, release notes, or version history
- The user wants to audit documentation completeness or freshness
- The task involves doc generation tooling (TypeDoc, Sphinx, rustdoc, Storybook, Docusaurus)
- Best for requests like:
  - "帮我写个 README"
  - "补一下这些函数的 JSDoc/docstring"
  - "帮我写个 ADR 记录这次架构决策"
  - "生成 changelog"
  - "这个项目的文档有哪些缺失"
  - "搭一下 TypeDoc/Sphinx 文档生成"

## Do not use

- The task is academic paper writing → use `$paper-writing`
- The task is `.docx` Word document editing → use `$doc`
- The task is `SKILL.md` or skill documentation → use `$skill-framework-developer` or `$skill-creator`
- The task is code comment style enforcement only → use `$coding-standards`
- The task is API interface design (not its documentation) → use `$api-design`
- The task is PDF generation or manipulation → use `$pdf`

## Task ownership and boundaries

This skill owns:
- README, CONTRIBUTING, CODE_OF_CONDUCT, onboarding guides
- API documentation (JSDoc, TypeDoc, docstrings, rustdoc, Swagger UI)
- Architecture Decision Records (ADRs)
- Changelog and release notes (conventional commits, keep-a-changelog)
- Documentation completeness auditing
- Doc generation pipeline setup (Sphinx, TypeDoc, rustdoc, Docusaurus, Storybook docs)
- Inline code documentation strategy and standards

This skill does not own:
- Academic paper prose
- Office document formatting
- Skill metadata files
- Code style enforcement beyond documentation annotations

## Core workflow

### 1. Assess

- Identify existing documentation and its structure
- Determine the documentation audience (users, contributors, maintainers, API consumers)
- List what's missing: README sections, API docs, ADRs, changelog, setup guide
- Check if doc generation tooling is already configured

### 2. Structure

- Design the documentation information architecture
- Define consistent templates for recurring document types (ADR template, API doc template)
- Establish naming and organization conventions
- Plan cross-references between documents

### 3. Write

- Write each document section with the target audience in mind
- Use concrete examples, code snippets, and diagrams where helpful
- Follow the project's established voice and terminology
- Ensure API docs are synchronized with actual code signatures

### 4. Automate

- Set up or configure doc generation tooling if appropriate
- Add doc generation to CI/CD if the project supports it
- Configure automatic changelog generation from conventional commits if applicable

### 5. Verify

- Check all internal links and cross-references
- Verify code examples compile/run
- Ensure API docs match current code
- Review for completeness against the documentation checklist

## Output defaults

```markdown
## Documentation Summary
- Scope: [what was documented]
- Audience: [who this serves]

## Documents Created / Updated
1. [document] — [status: new / updated / audited]

## Completeness Check
- README: ✅ / ❌ [missing sections]
- API docs: ✅ / ❌
- ADRs: ✅ / ❌
- Changelog: ✅ / ❌
- Onboarding guide: ✅ / ❌

## Follow-up
- ...
```

## Hard constraints

- Always match documentation to actual code behavior, not aspirational behavior
- Keep examples runnable and tested when possible
- Do not duplicate information across documents; cross-reference instead
- Use the project's existing documentation conventions if they exist
- Flag any documentation that contradicts the current codebase
- ADRs should be immutable once accepted; create new ADRs to supersede old ones
- **Superior Quality Audit**: For production-grade documentation, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## Trigger examples
- "强制进行文档工程深度审计 / 检查内容完整性与链接有效性。"
- "Use $execution-audit to audit this README for completeness idealism."
