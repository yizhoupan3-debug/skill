# 数理推理强度 harness（STEM）

**语义层真源**：与 [推理深度契约](reasoning-depth-contract.md) 同层（L5）；**运行时**仍只认 **L1 可执行验证 + L2 证据落盘**，不把自然语言「像证明」当作通过标准。

**编排入口**：多轮 RFV 见 [lane-templates.md](lane-templates.md) 中的数理专项 lane；Autopilot 须在 **Goal 契约**里写明 **双轨/脚本级** `validation_commands`。[`rfv_loop_harness.md`](../../rfv_loop_harness.md) 中的 `verify_commands` 与 **`EVIDENCE_INDEX`** 规则同样适用。

---

## A. 可检验中间对象（intermediate witnesses）

把题目拆成若干 **可独立核对** 的小命题；通过标准是 **特例一致、极限相容、无明显矛盾**，不是「读起来顺」。

| 机制 | 要求 |
|------|------|
| **量纲 / 退化极限 / 特例** | 在 handoff 或 `goal` 附件中列出 **Witness 清单**（例如 \(t\to 0\) 标度、对称性、边界情形应满足的阶或常数）。 |
| **区间与误差** | 主结论须标明 **误差阶 \(O(\cdot)\)、显式区间或常数上界**，并标注 **每一步依赖的假设**（哪怕只能定性）。 |
| **双轨对照** | **解析/代数轨** 与 **数值或枚举轨** 并行：数值轨须给 **固定种子、容差、对照协议**（Monte Carlo / brute-force 小范围）；`verify_commands` 中至少一条可复跑脚本。 |

**Review 收窄**：本轮 reviewer 只做 **假设—结论逐项对照** + **与 witness 清单一致性**；不在本轮扩写完整证明散文。

**Counterexample lane**（只读）：专门寻找与 witness 或主结论矛盾的实例；发现矛盾则 **FAIL**，写入 `review_summary` 并驱动 fix。

---

## B. 符号层 verifier（CAS / SMT / 证明助手）

Harness **只认 checker 输出**（exit code + 约定 stdout/stderr），不认「写作风格像定理」。

| 工具类 | 适用 | PASS 条件 |
|--------|------|-----------|
| **SymPy 等 CAS** | 恒等变形、求导、化简 | 脚本以 0 退出且输出与 golden 或自洽检查一致 |
| **Z3 / SMT** | 小范围可行性、不变式 | 输出 `sat`/`unsat` 等与契约一致 |
| **Lean / Coq** | 仅在团队已有模板与 CI 成本可接受时 | `lake build` / `coqc` 无 sorry |

**升级顺序**：CAS → SMT → ITP；任一层给出 **显式反例** 即 **FAIL**，优先记入 `append_round` 与 `EVIDENCE_INDEX`。

**证据落盘两条路径**（与 [`rfv_loop_harness.md`](../../rfv_loop_harness.md) 一致）：宿主 **`PostTool`** 在启发式命中时自动追加一行到 `EVIDENCE_INDEX`（`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` 未关、连续性就绪）；**`framework hook-evidence-append`** 供长尾命令显式记账（非 `cursor_*` 来源时仍走同一验证启发式，含 SymPy / Z3 / Lean / Coq 等 **窄子串**，见 `router-rs` `framework_runtime`）。数理脚本请避免仅写裸 `python` 作为唯一可识别串。

---

## C. 挡「似是而非推导」

| 机制 | 做法 |
|------|------|
| **逐步依赖图** | Fixer 交付 **编号步骤表**：每步 = 结论 + **引用的引理/步骤编号**。Reviewer **只攻击**「本步是否引入未证依赖」。 |
| **反事实探针（数理 fuzz）** | 独立只读 lane 使用 **错误代入 / 错误极限顺序**；主答须 **拒错前提** 或推出矛盾。**盲从** → 本轮记 `probe_failed`，不得标为通过。 |

---

## 与非目标

- 不在 L3/L4 实现自动定理证明；默认仍是 **小 checker + 强对照**。
- 不新增第二套证据 schema；仍用 **`EVIDENCE_INDEX`** + **`append_round`**。
- 长版 Operator 文案见 `configs/framework/HARNESS_OPERATOR_NUDGES.json` 中的 **`math_reasoning_harness_line`**；深度外研检索句为同文件的 **`retrieval_trace_harness_line`**。当宿主续跑启发式命中时，上述行由 `router-rs` 注入 **`RFV_LOOP_CONTINUE`** / **`AUTOPILOT_DRIVE`**（数理行）及 RFV 外研 struct 提示路径（外研两行）；字段留空则该行不出现。关闭注入见 `docs/harness_architecture.md` 开关面（`ROUTER_RS_HARNESS_OPERATOR_NUDGES`、`ROUTER_RS_OPERATOR_INJECT`、`ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT`）。启发式边界：英文 `proof` 仅 ASCII 词边界匹配并排除常见 **proof of concept** / **proof-of-concept** 短语；已去掉裸 **`derive`** 子串以免 Rust/Serde 工程句误触；Autopilot 与 RFV 一样会扫描命令串（`validation_commands` / `verify_commands`）；与 PostTool 数理子串集需与 `framework_runtime` 人工对齐。
- **PoC 与 toolchain**：当文案含 **proof of concept** / **proof-of-concept** 时，仅凭 `theorem`/`lemma` 等宽松英文词**不再**触发数理续跑；仍可由 **中文数理词**、**可执行 toolchain 子串**（SymPy / Z3 / Lean 等，见 `router-rs` `formal_toolchain`）或显式短语 **`formal proof`** / **`mathematical proof`** 触发（例如 **`sympy proof of concept`** 仍会因 SymPy 子串命中 toolchain 而触发）。
