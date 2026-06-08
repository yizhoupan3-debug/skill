# Systematic review / Survey Builder lane

吸收 **Survey Builder** 与 ARS **systematic-review** 模式精华；五宿主执行契约相同。

## 何时启用

- 用户要系统综述、meta-analysis 检索策略、PRISMA 流水、纳入排除表。
- 综述为**科研决策**服务（非手稿前门）；成稿若需 IMRaD → handoff `$paper-workbench`。

## 可验收产出

| Artifact | 内容 |
|----------|------|
| `review_protocol.md` | PICO/PECO、检索库、检索式、纳入/排除标准 |
| `search_log.md` | 数据库、日期、命中数、去重后 n |
| `screening_log.tsv` | title, decision, reason, reviewer |
| `prisma_counts.json` | identified / screened / excluded / included |
| `extraction_table.tsv` | 研究特征、结局、质量评价 |
| PRISMA 图 | 用 `$diagramming` implementation-playbook §1b 模板 |

## 阶段（不可跳）

1. **协议**：冻结问题与检索式（探索性检索可先行，但 confirmatory 检索须记录）。
2. **检索**：`academic-sources.md` 多源 fan-out；生物医学优先 PubMed + MeSH 字段（见该 reference）。
3. **筛选**：标题/摘要 → 全文；记录排除原因（至少两级）。
4. **提取**：统一 extraction 表；质量评价工具按领域选择（Cochrane ROB2、NOS 等）并声明。
5. **综合**：定性合成默认；meta-analysis 仅在有预注册终点与统计计划时。
6. **报告**：PRISMA 2020 checklist 对照；缺口列入 `limitations`。

## 硬约束

- 不得把单次 Web 深研冒充系统综述；须有可复现检索式与筛选日志。
- 统计合成 → `$statistical-analysis`；手稿写作 → `$paper-workbench`。
