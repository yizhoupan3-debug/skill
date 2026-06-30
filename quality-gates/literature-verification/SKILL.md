---

description: 'Literature verification capability: DOI reachability, citation-claim alignment, contradiction sweep, closest work identification, coverage scoring.'
metadata:
  platforms:
  - supported
  tags:
  - literature
  - citation
  - verification
  - research
  version: '1.0.0'
name: literature-verification
scene: research
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: n/a
source: local
trigger_hints:
- $literature-verification
- DOI验证
- citation verification
- contradiction sweep
- literature check
- 引用审查
- 文献验证
- literature-verification
---
# Literature Verification

无状态能力 skill：对文献引用进行端到端可靠性验证。不独立编排会话，仅作为前门 skill 的内联验证步骤。

## When to Use

- 前门 skill 需要验证引用 claim 是否被文献支持
- 需要检查 DOI 可达性与元数据完整性
- 需要扫描文献集中的未解决矛盾
- 需要识别 closest prior work 并评估覆盖度

## Do not use

- 数学推导验证（→ `$formal-verification`）
- 文稿语言质量（→ `$prose-verification`）
- 统计结果审计（→ `$statistical-verification`）
- 文献综述撰写（→ `$paper-workbench`）

## Hard constraints

- DOI 可达性检查失败（HTTP 4xx/5xx）必须标记为 FAIL，不得以 "DOI 迁移" 为由豁免
- 单条 claim 无任何引用支持为 P0 blocker——无源声称不得出现在论文主体
- Contradiction sweep 发现未解决矛盾时，必须在报告中列出双方文献，不得选择性忽略
- 覆盖度评分 < 70 时必须警告用户存在显著文献缺口
- paperplain MCP 不可用时，降级为 DOI curl 检查，不得假装完成完整验证

## Input / Output

| 输入 | 输出 |
|------|------|
| 文献列表（BibTeX / DOI / 标题） | 每条引用的验证状态（PASS / FAIL / WARN） |
| claim ledger 或论文主张列表 | claim-文献对齐矩阵 |
| 覆盖范围关键词 | 覆盖度评分（0-100）及缺口清单 |

## Verification Checklist

Rust 实现：`research_harness::verification::literature`（通过 MCP tool 或直接调用）

MCP tool: `research_verification_literature` → `verification_tool_dispatch`（`check` 参数：`doi` / `claim_coverage`）

```
# DOI 可达性检查：
research_harness::verification::literature::verify_doi_reachable(doi).await

# Claim 覆盖率计算：
research_harness::verification::literature::verify_claim_coverage(claims, references)
```

| # | 检查名 | PASS 条件 |
|---|--------|-----------|
| 1 | DOI 可达性 | 每条 DOI HTTP 200 或 3xx |
| 2 | 引用-claim 对齐 | 0 条 UNSUPPORTED |
| 3 | Contradiction sweep | 0 条未解决矛盾 |
| 4 | Closest work 识别 | ≥ 3 行优先级表 |

## References

- citation-management skill：[`../../skills/citation-management/SKILL.md`](../../skills/citation-management/SKILL.md)（引用元数据验证与格式化）
- paperplain MCP：`mcp__paperplain__fetch_paper` / `mcp__paperplain__find_paper_by_title` / `mcp__paperplain__search_research`
- claim-evidence 阶梯：[`../../skills/research/paper-workbench/references/claim-evidence-ladder.md`](../../skills/research/paper-workbench/references/claim-evidence-ladder.md)

## Integration Contract

### Trigger

| Caller | When | Blocking | Call mode |
|--------|------|----------|-----------|
| `$research` (discovery lane) | literature survey lane completes, before synthesis | No (advisory — coverage gap warns but does not block) | Inline |
| `paper-workbench` | submission gate: reference integrity check | Yes (FAIL blocks submission readiness) | Inline |

### Input

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `references` | `Vec<BibTeX\|DOI\|Title>` | yes | Literature entries to verify |
| `claim_ledger` | `Vec<Claim>` | no | Claim list for alignment matrix (omit = skip check #2) |
| `coverage_keywords` | `Vec<String>` | no | Keywords for coverage scoring (omit = skip check #4) |

### Output

```json
{
  "status": "PASS" | "FAIL" | "WARN",
  "checks": [
    { "name": "doi_reachability", "status": "PASS" | "FAIL" | "WARN", "detail": "..." },
    { "name": "claim_alignment", "status": "PASS" | "SKIP" | "FAIL", "detail": "..." },
    { "name": "contradiction_sweep", "status": "PASS" | "FAIL", "detail": "..." },
    { "name": "closest_work", "status": "PASS" | "FAIL", "detail": "..." }
  ],
  "blockers": ["DOI 10.xxxx/yyyy returned 404"],
  "metrics": { "coverage_score": 85 }
}
```

### Failure propagation

- **PASS**: caller continues normally.
- **WARN**: caller continues with annotation in evidence map.
- **FAIL** (blocking caller): caller MUST NOT advance to next stage; blocker list is returned to user or upstream orchestrator.
