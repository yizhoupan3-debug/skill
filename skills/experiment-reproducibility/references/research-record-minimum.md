# 科研纪录最低清单（Research record minimum）

与 [`../SKILL.md`](../SKILL.md) 的环境/种子/数据层配合使用：满足下列最低项后，才适合声称「可复核」或写入手稿/补充材料的方法与可复现段落。

## 预注册与分析计划

- [ ] **预注册或等价**：分析计划、主终点/主对比、样本规则在数据采集前或锁定分析前可查（注册库、OSF、机构预注册或领域内等价）。
- [ ] **主分析 vs 探索性**：明确 **confirmatory（主假设/主终点）** 与 **exploratory（探索、机制、子群）**；探索性结果不得默示为同一多重比较族内的确证性结论。
- [ ] **方案偏离**：任何偏离预注册或锁定方案之处（样本、预处理、模型、剔除规则）有**带日期的记录**与对结论敏感性的说明。

## 数据与材料标识

- [ ] **数据 DOI / 版本**：公开数据写 DOI、版本号或快照标识；私有数据写访问条件与伦理批件引用，避免「数据可得 upon request」作为唯一关停句。
- [ ] **阴性结果**：与主分析同等登记（含中止实验、无效尝试），避免文件抽屉。

## 软件与环境 pin

- [ ] **语言与包版本**：解释器/编译器版本 + 依赖 lockfile 或等价 pin（含关键非 Python 依赖）。
- [ ] **随机性与确定性**：种子、已知非确定来源（GPU、浮点、多进程）已记录；若不可完全复现，写明波动量级与是否影响结论。
- [ ] **分析脚本与配置**：可指向 commit、标签或归档 bundle，与正文图表一一对应。

## 与手稿栈的衔接

- 主张—证据与 R&R 关停件：`paper-workbench` → [`claim-evidence-ladder.md`](../../paper-workbench/references/claim-evidence-ladder.md)。
- 仓库连续性：`docs/harness_architecture.md`、`artifacts/current/` 下 `SESSION_SUMMARY` / `NEXT_ACTIONS` / `EVIDENCE_INDEX`。
