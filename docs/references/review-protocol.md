# Review 通用协议

## 1. 问题

**幻觉率高**：review findings 可能引用不存在的源、捏造证据、错标位置、曲解行为。
**输出冗余**：用户只需 confirmed findings，不需要 rejected/hallucinated 详情。
**碎片化**：各领域独立实现 review 行为，缺少共享的输出约束和幻觉分类标准。

## 2. 设计原则

1. **不强加统一 pipeline**：各领域保持自己的 review 流程，在已有流程中嵌入 factcheck 步骤
2. **不重命名 schema**：code-review-deep 保持 code-specific 字段名（`code_exists`、`line_accurate` 等语义精确），不追求跨领域统一字段名
3. **confirmed-only 是唯一跨领域硬约束**：所有 review 输出只包含经事实核查 + 判断通过的 findings
4. **Factcheck = 执行已有的 Verification Checklist**：各领域已有的 verify commands 就是事实核查，不引入新抽象层
5. **本文件是跨领域协议权威**：幻觉分类标准和 confirmed-only 约束以本文件为准；各 SKILL.md 拥有各自领域的 factcheck 实现细节

## 3. 跨领域硬约束

### 3.1 Confirmed-only 输出

所有 review 类 skill/workflow 的最终用户可见输出**只包含 confirmed findings**。

- confirmed = 事实核查通过（evidence 真实存在且准确）+ 判断通过（是真实问题）
- rejected（判断驳回）和 hallucinated（事实核查拦截）不出现在用户输出中
- 可选统计摘要行：`N confirmed / M rejected / K hallucinated`（一行，不展开）
- 完整的 rejected/hallucinated 数据仍保留在 workflow return 对象中供调试，但不渲染到用户可见层
- **运行时强制**：通过修改 workflow return 对象实现（§4.1），不是仅靠文档声明

### 3.2 幻觉分类标准（hallucination_type）

跨领域通用的幻觉分类枚举。`FACTCHECK_VERDICT_SCHEMA` 的 enum 值与本表一一对应：

| 值 | 含义 | 典型场景 |
|----|------|---------|
| `none` | 事实全部准确 | — |
| `code_not_exist` | 引用的源不存在 | 代码文件/论文/段落/图表不存在 |
| `evidence_fabricated` | 源存在但证据是捏造/复述 | 代码引用非原文、论文 abstract 内容捏造 |
| `wrong_line` | 源存在但位置错误 | 行号偏差、段落编号不对、图表引用错 |
| `behavior_misrepresented` | 证据正确但行为/现象描述有误 | 代码行为描述错误、统计结论与数据不符 |
| `evidence_out_of_context` | 证据真实但与 finding 无关 | 引用了正确代码行但描述的问题不相关 |
| `source_moved` | 源已重命名/移动 | 文件 rename、论文换 DOI、图表重编号 |
| `partial_hallucination` | 部分准确部分幻觉 | 多引用中部分真实部分捏造（与 `evidence_out_of_context` 互斥：前者是事实性问题，后者是关联性问题） |
| `indeterminate` | 无法确认 | 工具不可用、agent 失败、视觉域无截图 |

**互斥规则**：`partial_hallucination` 适用于多引用中部分引用的事实性问题（有些存在有些不存在）；`evidence_out_of_context` 适用于单引用的语义关联性问题（引用存在但与 finding 无关）。两者不同时成立。

**命名说明**：字段名保持 code-specific（`code_not_exist` 而非 `source_not_exist`），因为当前唯一使用 FACTCHECK_VERDICT_SCHEMA 的 workflow 是代码审查。非代码领域通过各自 SKILL.md 的 verify checklist 做事实核查，不共享此 schema。

### 3.3 降级策略

当 factcheck 工具不可用时（无网络/无 scipy/无 GUI/agent 失败/超时）：

| 情况 | 行为 |
|------|------|
| 单个 finding 的 factcheck agent 失败 | 标记 `hallucination_type: "indeterminate"`，**不进入 Verify** |
| 整个 factcheck 阶段失败（pipeline catch） | **不跳过 factcheck**。所有 findings 标记为 `indeterminate`，不进入 Verify，最终输出为空（0 confirmed）。在统计行标注 "⚠️ factcheck 失败，0 findings 输出" |
| 部分 check 无法执行（如无原始数据） | 已执行的 check 正常返回，未执行的标记 N/A，整体 verdict 基于已执行 check |
| 网络不可用（DOI/论文查询） | 标记相关 check 为 `indeterminate`，其余 check 继续执行 |
| 领域专用工具全量不可用（如 latexmk 未安装） | 所有 check 标记 `indeterminate`，该领域的 findings 全部不进入 Verify |

**关键**：factcheck 整体失败时**不降级为"跳过 factcheck 直接进 Verify"**——这会将幻觉 findings 放入 confirmed 输出，违反 §3.1。

### 3.4 各领域 Factcheck 机制

各领域的 factcheck **就是执行本领域已有的 Verification Checklist**，不引入新的抽象层：

| 领域 | Factcheck 机制 | 独立 agent？ | 工具需求 | 确定性？ |
|------|---------------|-------------|---------|---------|
| **代码**（code-review-deep） | 独立 Factcheck agent：逐文件 cat + 逐字比对 evidence | ✅ 已实现 | Read + Bash | ✅ 确定性 |
| **论文**（paper-workbench） | @lane:reviewer 输出 findings 后，按需调用 prose-verification / structure-verification / literature-verification / statistical-verification 校验 | ❌ inline 调用 | Read + Grep | ⚠️ 部分确定性 |
| **结构**（structure-verification） | LaTeX 编译 + \ref/\label 交叉 + 方程编号连续性 | ❌ inline 调用 | Bash（latexmk） | ✅ 确定性 |
| **文献**（literature-verification） | DOI 可达性（curl）+ 引用-claim 对齐（grep UNSUPPORTED） | ❌ inline 调用 | Bash + MCP paperplain | ⚠️ DOI 确定性，claim 对齐 LLM |
| **统计**（statistical-verification） | p 值重算（scipy）+ GRIM test + 效应量 | ❌ 确定性执行 | Bash（Python scipy） | ✅ 确定性 |
| **文字**（prose-verification） | 术语表 grep + claim ledger diff + style guide 比对 | ❌ inline 调用 | Read + Grep | ⚠️ 术语确定性，claim drift LLM |
| **视觉**（visual-review） | CoT 模板（OBSERVE→DESCRIBE→SCAN→ASSESS→JUDGE）+ indeterminate 兜底 | ❌ inline 调用 | 截图 | ❌ LLM 判断 |
| **PR**（gh-address-comments） | comment 原文 fetch + 分类结果校验 | ❌ inline 调用 | gh CLI | ⚠️ fetch 确定性，分类 LLM |
| **形式验证**（formal-verification） | CAS identity / SMT / witness / 量纲 / 步骤依赖图 | ❌ inline 调用 | Bash（CAS 工具） | ✅ 确定性 |
| **可复现性**（reproducibility-verification） | 种子 / 确定性重跑 / lock file / 数据版本 / checkpoint | ❌ inline 调用 | Bash | ✅ 确定性 |
| **引用管理**（citation-management） | BibTeX 完整性 / DOI 映射校验 / 引用元数据核实 | ❌ inline 调用 | Read + Bash | ⚠️ DOI 确定性，元数据 LLM |

**代码领域特殊性**：code-review-deep 的 factcheck 需要独立 agent（逐字比对 evidence 与源码），因为代码 finding 的幻觉率最高（agent 可能引用不存在的代码行）。其他领域的 factcheck inline 调用已有 verify commands。

**确定性分级说明**：
- ✅ 确定性：工具输出可直接判定 PASS/FAIL（如 latexmk 编译、scipy 重算、DOI HTTP 状态码）
- ⚠️ 部分确定性：部分 check 用工具判定，部分依赖 LLM 判断
- ❌ LLM 判断：factcheck 本身依赖 LLM 的观察和推理能力，存在循环依赖风险（LLM 可能在 factcheck 阶段引入新幻觉）。对这些领域，factcheck 是"尽力而为"的幻觉减少层，不保证完全拦截

## 4. 变更清单

### 4.1 Workflow 输出修改（confirmed-only）

| 文件 | 变更 |
|------|------|
| `.claude/workflows/deep-review-template.js` | Synthesize return 对象：`hallucinated` 改为仅返回 `hallucinated_count`；`rejected` 改为仅返回 `rejected_count` |
| `.claude/workflows/deep-review-template.js` | 同上 |

### 4.2 SKILL.md 声明

各领域 SKILL.md 新增 factcheck 声明（声明 + workflow 代码中的调用逻辑共同生效）：

| 文件 | 变更 |
|------|------|
| `skills/code-review-deep/SKILL.md` | 已有 Factcheck gate 小节。新增 confirmed-only 输出约束声明 |
| `skills/paper-workbench/SKILL.md` | @lane:reviewer 中声明：findings 输出前按需调用 verification sub-skills 作为事实核查 |
| `skills/structure-verification/SKILL.md` | 声明：LaTeX 编译和 \ref/\label 交叉即为本领域 factcheck |
| `skills/literature-verification/SKILL.md` | 声明：DOI 可达性和引用-claim 对齐即为本领域 factcheck |
| `skills/statistical-verification/SKILL.md` | 声明：verify commands 执行即为本领域 factcheck |
| `skills/prose-verification/SKILL.md` | 声明：术语表 grep 和 claim ledger diff 即为本领域 factcheck |
| `skills/visual-review/SKILL.md` | 声明：CoT 模板 + indeterminate 兜底即为本领域 factcheck |
| `skills/gh-address-comments/SKILL.md` | 声明：comment 原文 fetch + 分类校验即为本领域 factcheck |
| `skills/formal-verification/SKILL.md` | 声明：CAS/SMT/witness 验证即为本领域 factcheck |
| `skills/reproducibility-verification/SKILL.md` | 声明：种子/确定性重跑/lock file 检查即为本领域 factcheck |
| `skills/citation-management/SKILL.md` | 声明：BibTeX 完整性和 DOI 映射校验即为本领域 factcheck |

### 4.3 hallucination_type 枚举更新

| 文件 | 变更 |
|------|------|
| `.claude/workflows/workflow-helpers.js` | `FACTCHECK_VERDICT_SCHEMA.hallucination_type.enum` 追加 `evidence_out_of_context`、`source_moved`、`indeterminate`（现有 6 值 + 3 = 9 值） |

### 4.4 不改的文件

| 文件 | 原因 |
|------|------|
| `workflow-helpers.js` 的 FACTCHECK_VERDICT_SCHEMA 字段名 | 保持 code-specific（语义精确） |
| AGENTS.md | 不宿主特化 |
| RUNTIME_REGISTRY.json | 已在前轮追加 factcheck lanes |
| Rust 代码 | review engine advisory 模式不变 |

## 5. 优先级

**P0（立即）**：
1. 两个 workflow 文件 confirmed-only 输出
2. hallucination_type 枚举追加 3 个值
3. 修复 factcheck-verifier.md 损坏字符串（已完成）

**P1（紧跟）**：
4. 各 SKILL.md 新增 factcheck 声明（11 个文件，每个 3-5 行）
