# 手稿编辑范围门控（精准修改 vs 大面积重构）

在调用 `$paper-writing`、`$paper-reviser` 或在本 front door 内落地改稿前，必须先确定 **`edit_scope`**，避免「只说润色两句」却触发整篇删并、叙事重写或口径升级。

## 两种模式（互斥）

### `surgical`（精准修改 / 外科式）

**默认**：用户要改稿、但未明确授权结构性重构时，**一律按 surgical**（偏安全）。

允许：

- 在用户**写明或确认的范围内**改句式、衔接、术语一致、局部澄清；
- 在同一 claim ceiling 内做**不改变科学含义**的压缩或拆分句子。

禁止（除非用户把该项写进 scope 清单）：

- 删除 / 合并章节与小节、大段挪到附录、调整全文叙事主线；
- 为「对齐 throughline」而**跨节**改写 abstract / intro / conclusion；
- 升级或收窄 claim、补「更强结论」措辞；
- 整文件或整章替换式重写。

**改前必须产出（可极简）**：

```text
edit_scope: surgical
scope_items:
  - <章节或小节标识，或段落锚点 / 行号范围 / 引用的原文片段>
non_goals: <明确写出本轮不动哪些部分>
```

**交付偏好**：对可 diff 的源文件，优先给出 **hunk / 查找替换块**；避免贴回整篇。

### `refactor`（大面积重构）

**仅当**用户明确授权结构性改版，或出现强信号词（见下）时启用。

允许：

- `$paper-reviser` 全合同：删、缩、附录路由、降 claim、按审稿意见批量改多节；
- 多节联动叙事、throughline 重塑（仍受 claim ledger / 证据锚约束）。

**改前必须产出**：

```text
edit_scope: refactor
refactor_intent: <例如：按 R1 全面压篇幅 / 重写 related work 叙事 / 合并实验节>
risk_note: <哪些 claim 可能被动到，是否需要先过 reviewer 逻辑门>
```

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
