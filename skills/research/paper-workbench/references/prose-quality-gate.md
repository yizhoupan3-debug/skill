# Prose quality gate（写作质量传感器）

**真源**：`$paper-writing` 与 `$paper-workbench` 在**触达正文句子**时的共用门控。  
**全链路**：[`../../paper-workbench/references/prose-chain-contract.md`](../../paper-workbench/references/prose-chain-contract.md)（路由 → 审 → 改 → 写 → 验收）。  
**不替代**：[`research-language-norms.md`](../../paper-workbench/references/research-language-norms.md)（姿态/术语）、[`claim-evidence-ladder.md`](../../paper-workbench/references/claim-evidence-ladder.md)（主张安全）。

## 1. `language_register`（自动推断，禁止等用户声明）

每轮改稿前**必须由模型推断**（用户**无需**也**不应**被要求声明 token）：

| 值 | 场景 | 默认 QC 集 |
| --- | --- | --- |
| **`en_submission`** | 英文投稿（期刊/顶会、英文 rebuttal/cover letter） | §3 EN slop + Gopen + Cadence（[`section-by-section.md`](section-by-section.md)） |
| **`zh_manuscript`** | 中文论文/学位论文正文（含中文摘要、中文引言） | §3 ZH 套话 + 段首句 + 衔接；英文术语首次括注 |
| **`mixed`** | 中文正文 + 英文摘要/关键词/图注混排 | **分 surface**：中文段走 `zh_manuscript`，英文段走 `en_submission`；禁止用英文润色规则「洗」中文段 |

机器 token（可选，单独一行）：`language_register: en_submission` | `zh_manuscript` | `mixed`

**禁止**：在同一轮对 `scope_items` 外的语言混用规则；`mixed` 须在 `scope_items` 里标明各锚点的 register。

## 2. Rewrite ladder（硬门槛）

来源：[`revision-playbook.md`](revision-playbook.md) §Rewrite Ladder。  
**未通过 L1–L4 不得交付「终稿段落」**（仅允许交付提纲，见 §3）。

| 级 | 检查 | 未通过时的动作 |
| --- | --- | --- |
| **L1 Purpose** | 本段/本节要让读者相信什么？一句 | 先写 purpose 句，不改措辞 |
| **L2 Reader path** | 信息顺序是否按读者建立信任，而非作者发现顺序？ | 重排提纲，再写正文 |
| **L3 Claim boundary** | 动词/范围是否与证据一致？ | 校准 claim，禁止「更谦虚」糊弄 |
| **L4 Paragraph job** | 每段：topic 句 + 一条证据/机制 + takeaway？ | 拆段或补 topic 句 |
| **L5 Sentence** | Gopen：主谓近、old→new、句末 emphasis | 逐句修 |
| **L6 Surface** | 语法、拼写、节奏 | 最后做 |

交付须含一行：**`ladder_passed: L1–L4`**（若只做了 L5–L6，须写 **`ladder_blocked: <原因>`** 并只交付 §3 提纲，不得贴长段终稿）。

## 3. 两阶段交付（硬）

凡 **`ladder_passed` 含 L4 之前的工作** 或 **新起草 / 结构重写 / 整节改写**：

1. **Stage A（可 bullet）**：`story_spine` 一句 + 各段 **topic sentence 提纲**（每段 1–2 行）；`refactor` 时再加 section outline（见 `paper-writing` SKILL）。
2. **Stage B（禁止 bullet）**：完整段落正文；摘要/引言/讨论/回复信均适用。

**例外**：用户明确「只改这一句/词」且 `scope_items` 为词级/单句 → 可跳过 Stage A，但仍须 `language_register` + `prose_qc`。

## 4. `prose_qc`（替代「自我感觉良好」）

在 **`tone_audit` 之后、prose 之前** 输出块 **`prose_qc`**（与 [`research-language-norms.md`](../../paper-workbench/references/research-language-norms.md) §3 互补，不重复 (a)–(d) 全文）。

### 4.1 英文 (`en_submission`) — slop 扫描

对 **Stage B 终稿**（或 surgical 改后句）扫描；**命中须改写或列入 `prose_qc_fixes`**：

- 空洞开场：`In recent years`, `With the rapid development of`, `Recently, there has been growing interest`
- 填充副词堆：`very`, `really`, `basically`, `essentially`, `quite`（无度量处）
- 万能动词：`leverage`, `utilize`（可用 `use` 处）, `delve into`, `shed light on`
- 模板强调：`plays a crucial/vital/key role`, `it is worth noting that`, `it is important to note`
- 连接词连发：连续 3 句以 `Furthermore` / `Moreover` / `In addition` 起句
- 自指废话：`In this paper, we` 作段首（摘要/引言首句尤其禁止）
- 假对比：`not only ... but also` 无真实第二维度

### 4.2 中文 (`zh_manuscript`) — 套话扫描

命中须改写：

- 空泛评价：`具有重要意义`, `进行了深入研究`, `取得了显著进展`, `具有广阔的应用前景`
- 翻译腔名词化：`对…进行研究`, `对…进行分析`（可改为直接陈述：「我们测量…」「我们比较…」）
- 堆砌连接：`首先…其次…再次…最后` 贯穿全段而无新信息
- 重复主语链：`本文…本文…本研究…` 同段 ≥3 次 → 合并或换指代
- 英文直译硬造：未在子领域文献出现的「XX性」「XX化」复合指标名（回 [`research-language-norms.md`](../../paper-workbench/references/research-language-norms.md) §1）

### 4.3 共用机械检查

- **段首句测试**：每段首句能否单独回答「本段结论/任务」？`fail` 须标 `paragraph_id`
- **Cadence**：同段连续 ≥3 句相同起手（We/本文/However）→ `fail`
- **终稿无 bullet**：Stage B 不得用列表冒充段落（图注/贡献列表 venue 要求除外）

### 4.4 `prose_qc` 输出形状

```text
language_register: <en_submission|zh_manuscript|mixed>
ladder_passed: L1–L4 | ladder_blocked: <reason>
slop_hits: <count> (list line refs or change_ids if surgical)
paragraph_topic_test: pass | fail [ids]
cadence: pass | fail [ids]
prose_qc_fixes: <none | 1-line summary of rewrites applied>
```

## 5. 与 `tone_audit` 的分工

| 块 | 负责 |
| --- | --- |
| **`tone_audit`** | research-language-norms §3 (a)–(d)：内部/防御/负面口径/but 链 |
| **`prose_qc`** | 本文件：register、ladder、slop、段首句、cadence |

## 6. 用户快捷 token

`writing_mode: ladder-full` — 强制 §2–§3（先提纲后正文）。  
`writing_mode: sentence-only` — 仅当 `scope_items` 为词/单句级。

## 7. 全链路衔接（只读指针）

| 上游 | 本门控消费 |
| --- | --- |
| `$paper-reviewer` language finding | `prose_repair_class` + `writing_handoff` → 本轮回写范围 |
| `$paper-workbench` intake | `language_register` + Claim card |
| `$paper-workbench` inline revision batch | 结构改完后触句仍须 `prose_qc` |
| L2 `PROSE_QC_LOG.md` | 每轮 append `prose_qc` 摘要（可选） |
| L4 `PAPER_PROSE_QUALITY_HOOK` | **默认开**；提醒自动 ladder + register |

契约真源：[`../../paper-workbench/references/prose-chain-contract.md`](../../paper-workbench/references/prose-chain-contract.md)。
