# 手稿编辑范围门控（精准修改 vs 大面积重构）

在调用 `$paper-writing`、`$paper-reviser` 或在本 front door 内落地改稿前，必须先确定 **`edit_scope`**，避免「只说润色两句」却触发整篇删并、叙事重写或口径升级。

## 两种模式（互斥）

### `surgical`（精准修改 / 外科式）

**默认**：用户要改稿、但未明确授权结构性重构时，**一律按 surgical**（偏安全）。

**硬等级（给实现方）**：在 `surgical` 下**任何**落在 `scope_items` 之外的改动，或**未**用 `change_id + original_excerpt` 绑定就动到的句子，**一律视为越权**，须撤回／拆成下一轮并先扩 `scope_items` 或升格 `refactor`。**不准**用「整体更顺」「顺便统一术语」「通读小修」抵赖——这些词**不是**扩权许可。

允许：

- 在用户**写明或确认的范围内**改句式、衔接、术语一致、局部澄清；
- 在同一 claim ceiling 内做**不改变科学含义**的压缩或拆分句子。

禁止（除非用户把该项写进 scope 清单）：

- 删除 / 合并章节与小节、大段挪到附录、调整全文叙事主线；
- 为「对齐 throughline」而**跨节**改写 abstract / intro / conclusion；
- 升级或收窄 claim、补「更强结论」措辞；
- 整文件或整章替换式重写。
- **整篇回贴式交付**：在 `surgical` 下以「以下是全文/全节润色稿」**替换**用户未授权的全部正文（即使用户贴了整篇源文，也只许交 **hunk / 逐条替换块 / 带锚点的局部块**），除非用户**显式**授权 `refactor` 或声明「允许全文替换」。
- **全局「顺稿」**：未列入 `scope_items` 的节、图注、caption、摘要、关键词、附录条目的**任何**字句改动。
- **静默全局替换**：跨文件或跨全文的术语/拼写/英式美式「统一」、批量改引号/破折号风格，除非 `scope_items` **逐条**写明该替换范围（或单列一条「全文机械替换：X→Y」且仍受改动点数约束）。
- **幽灵一致性**：以「与已改段对齐」为由去改**未**出现在 `scope_items` 里的 abstract / intro / conclusion / related work（**越权 mirror** 的变体）。

**改前必须产出（可极简）**：

```text
edit_scope: surgical
scope_items:
  - <章节或小节标识，或段落锚点 / 行号范围 / 引用的原文片段>
non_goals: <明确写出本轮不动哪些部分>
```

**交付偏好**：对可 diff 的源文件，优先给出 **hunk / 查找替换块**；避免贴回整篇。

### `surgical` 防扩写（实现方强制执行）

下列行为在 **`surgical` 下一律禁止**，即使用户话术含糊地说了「优化一下」「顺一顺语气」也不得当作默许：

- **范围偷扩**：在用户未写入 **`scope_items`** 的表面（段落 / 小节 / 图注 / 定理陈述等）改写、删增、调换语序；
- **顺手整修**：以「读起来更顺」「统一文风」为名，顺带修改 **`scope_items` 紧邻上下段**未被点名的句子；
- **单锚多改**：一个 `scope_item` 只对应**一处**锚定 span；若实际改到多个不相邻段落 / 多个图注 / 跨小节，**必须**拆成多条 `scope_item` 或升为 `refactor`，禁止「一条目包打全场」。
- **静默结构变更**：拆分 / 合并段落、移动句子跨段、加删小标题或 enumerate/itemize，除非用户将该结构操作写进 **`scope_items`**；
- **越权 mirror**：在仅授权单段 / 单节时，去「对齐」abstract、intro、conclusion、caption 等其它表面（见上文禁止项；需 **`refactor`** 或把这些表面逐条写进 **`scope_items`**）；
- **科学语义漂移**：通过换词、加重或减轻语气、改比较级 / 最高级、改因果连接词，使读者对 claim 强度或证据覆盖产生不同理解（若必须动含义 → 先走 reviewer / reviser 决策链，再改稿）。

**锚定三选一（改前必须满足至少一种，否则先向用户要锚点）**：

1. **结构锚**：`§3.2` / `Lemma 2` / `Figure 4 caption` 等稳定定位 + 指明「本小段内哪些句」；
2. **verbatim 前缀锚**：抄写待改段的 **开头约 12 词（中英各自按可读单位）**，必要时再加 **末尾约 6 词**，保证编辑器的查找能唯一定位；
3. **审稿条目锚**：`R1-3b`、`Meta-review #2` 等 + 手稿中对应的被引段落仍须用 (1) 或 (2) 之一钉死。

**改动上限（与条目绑定）**：默认 **一轮 `surgical` 的离散改动点数 ≤ `scope_items` 列出的条目数**。若一条目内需改多句，须在对应条目中明示 `allow_multi_sentence_within_span: yes`（或等价自然语言）；否则视同只改 **该锚定 span 内的最少必要句数**。

**交付格式（与用户对齐「精准」）**：交付改稿时同时给出可追溯清单，例如：

```text
change_id: S1
scope_item_ref: §3.2 paragraph 2
anchor_used: verbatim_prefix + suffix
original_excerpt: <原文摘录，足够唯一定位，可略长>
summary_of_edit: <一句说明改了什么类别：措辞/标点/清晰度，不涉及 claim>
```

可多行 `S2`, `S3`…便于用户逐项接受或驳回；**不要把多处在 narrative 里混成一整段重写而不列清单**。

**默认交付形态（强约束）**：`surgical` 下**优先**只交付 **patch/hunk** 或 **逐条「原文摘录 → 改后」**；若交整段，每段须带 **change_id** 且能指回 `scope_item_ref`。**禁止**用单独一大段「润色后完整 §3」覆盖用户未把 §3 整体写入 `scope_items` 的情况。

若用户只说「这段话润色」却未粘贴或可定位的锚点：**先反问一次**要来 (1)(2)(3) 中之一种，再继续；不要凭记忆对「整页」下笔。

若用户粘贴**整篇**手稿但只要求局部修改：**仍**只改 `scope_items` 内锚定处；**禁止**借「通读」之名改未锚定段落。

### `refactor`（大面积重构）

**仅当**用户明确授权结构性改版，或出现强信号词（见下）时启用。

允许：

- `$paper-reviser` 全合同：删、缩、附录路由、降 claim、按审稿意见批量改多节；
- 多节联动叙事、throughline 重塑（仍受 claim ledger / 证据锚约束）。

**审稿 / R&R 提醒**：即便在 `refactor`，**也不得**把「降 claim / 挪附录 / 加长 limitation」当默认 escape；须先满足 [`claim-evidence-ladder.md`](claim-evidence-ladder.md) 的证据优先顺序与 §「审稿意见 / R&R：逐条关停与逃逸红线」，再动 ceiling。

**改前必须产出**：

```text
edit_scope: refactor
refactor_intent: <例如：按 R1 全面压篇幅 / 重写 related work 叙事 / 合并实验节>
risk_note: <哪些 claim 可能被动到，是否需要先过 reviewer 逻辑门>
```

**交付最低限度（与 `surgical` 对称）**：`refactor` 每一批次交付须带可追溯骨架，避免「整稿重写但说不清动到哪」——至少给出：

```text
sections_touched:
  - <主节 / 小节 stable id，如 §3.2 / Fig.4 caption / Appendix B>
claim_ledger_touch_statement: <none | 摘要：哪些 claim_id / allowed_claim_level 被动到；若仅排版或未改主张句写 none>
```

并与 `$paper-reviser` 批次末的 `claim_ledger_delta` / `evidence_anchor_delta` 叙述一致：凡声称动了主张边界，`claim_ledger_touch_statement` 不得写 `none`。

## 强信号词（启发式，非穷尽）

- 偏向 **refactor**：`大面积`、`重构`、`整篇改版`、`故事线`、`砍到 X 页`、`结构性`、`合并章节`、`全书式润色`、`全稿重写`。
- 偏向 **surgical**：`只改`、`仅此段`、`不要动结构`、`不要改 claim`、`局部`、`两句`、`这一段`、`按划定的范围`。

若**同时出现**两类信号或任务描述含糊：**先问一句**让用户选 `surgical` 或 `refactor`，再动稿；不要猜测成 refactor。

## 机器可读 token（可选）

用户可在消息中**单独一行**写明（便于宿主与复盘）：

```text
edit_scope: surgical
```

或

```text
edit_scope: refactor
```

## 与 `$paper-workbench` 外部模式的关系

| 外部模式     | 默认 `edit_scope` | 说明                                       |
|-------------|-------------------|--------------------------------------------|
| `局部精修`  | `surgical`        | 除非用户明确要重构                         |
| `按意见改稿`| 按条目不扩散      | 默认逐条 surgical；用户说「大改」再升 refactor |
| `整篇判断`  | 审稿侧无 edit_scope | 仅在转入改稿时再选 scope                 |
| `单维度会诊`| 通常 `surgical`   | 维度内允许的范围由用户指定                 |

## 单条审稿意见也可能需要 `refactor`

**不要**用「只有一条 comment」自动等价于 **`surgical`**。若**单条**意见落地时会触发
本门控在 **`surgical`** 下的禁止项（删并章节、附录大挪、全文压篇幅、跨节 throughline
重写、claim 升降级等），必须先与用户确认 **`edit_scope: refactor`**，或把该结构性
动作完整写进 **`scope_items`**（含会波及的节 / 图表 / 结论表述），再执行；禁止在
未授权的情况下为「逐条」而偷偷扩大改写范围。

## 与 `PAPER_GATE_PROTOCOL` 的关系

磁盘门控里的 `lane_scope`（如 `figure:F3-F6`）是 **surgical 的正式化**。**`refactor`**
仍应拆成多个可合并批次，每个批次可对应各自的 `lane_scope`；由 **`$paper-reviser`**
主链串行决策与合并，而不是把 `refactor` 误读成「只能有一个 `lane_scope`」。

## 自检（改稿结束前 10 秒）

仅在 **`surgical`** 下快速核对：

1. 是否有任何改动发生在 **`scope_items` 以外**的表面？若有 → 撤回或升格 `refactor` / 补齐条目。
2. 是否每条改动都能在 **`original_excerpt`** 粒度上被指回原文？若不能 → 补锚或缩小范围。
3. 是否存在「更清晰」但并非用户点名的重写？若有 → 删或移入下一轮并请求扩 scope。
4. 本次交付是否出现**整文件/整节大块回贴**而 `scope_items` 仅是局部？若是 → 改成交付 hunk/patch，或与用户确认升格 `refactor`。
5. 是否做任何**跨段术语统一 / 标点体系统一**而未写入 `scope_items`？若有 → 撤销或单列 scope 项并重算改动点数。
6. **`scope_items` 条数是否与 `change_id` 条数对齐**（或每条目明示 `allow_multi_sentence_within_span`）？若一页里改了十处却只列两条 scope → **越权**，须拆分或升格。
