---
last_verified: "2026-06-07"
depends_on:
  - ../../rfv_loop_harness.md
  - external-research-harness.md
  - reasoning-depth-contract.md
---

# Claim Ledger（结构化 Claim 追踪）

> **status: aspirational** — Claim Ledger 是 RFV 多轮 loop 中结构化 claim 管理的扩展机制；当前 `my-light` profile 下大部分任务未触发此路径。

**Schema 真源**：`configs/framework/CLAIM_LEDGER_SCHEMA.json`（draft-07，`$id: https://local/framework/claim-ledger-v1`）。Rust 校验与生命周期管理待 `core/router-rs/src/rfv_loop/` 集成。

## 1. Claim 生命周期

Claim 在 ledger 中经历以下状态转移：

```
proposed → supported
proposed → contested → supported
proposed → contested → rejected
proposed → contested → withdrawn
proposed → rejected
proposed → withdrawn
supported → contested → supported    (新反面证据出现时重新审视)
supported → contested → rejected
supported → contested → withdrawn
supported → withdrawn
```

| 状态 | 含义 | 进入条件 |
|------|------|----------|
| **proposed** | 新提出，尚未经过验证 | RFV review/external 阶段产出 |
| **supported** | 有可追溯来源和/或 EVIDENCE_INDEX 验证通过 | `verify_commands` PASS 或来源可追溯性确认 |
| **contested** | 存在矛盾扫描发现或反面证据 | `contradiction_sweep.status` 为 `contested` 或 `unresolved` |
| **rejected** | 被反面证据推翻或验证失败 | `verify_commands` FAIL 或矛盾不可消解 |
| **withdrawn** | 主动撤回（不再参与验证链） | 提出者显式撤回 |

## 2. Claim Ledger 结构

顶层对象 `ClaimLedger`：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `schema_version` | `"claim-ledger-v1"` | 是 | 固定版本标识 |
| `task_id` | string | 是 | 关联的 RFV task |
| `claims` | `Claim[]` | 是 | 所有 tracked claims |
| `generated_at` | ISO8601 | 是 | ledger 创建时间 |
| `updated_at` | ISO8601 | 是 | 最近更新时间 |

单条 `Claim` 对象：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `claim_id` | string | 是 | ledger 内唯一标识 |
| `text` | string | 是 | claim 陈述 |
| `status` | enum | 是 | `proposed` / `supported` / `contested` / `rejected` / `withdrawn` |
| `sources` | Source[] | 是 | 可追溯来源（source_type + ref_id + reachable + last_checked） |
| `evidence_ids` | string[] | 否 | 指向 EVIDENCE_INDEX 的行 ID |
| `contradiction_sweep` | object 或 null | 否 | 该 claim 的矛盾扫描记录 |
| `scope` | `claim` / `sub_claim` | 否 | 顶层 claim 或子 claim |
| `parent_section_id` | string 或 null | 否 | 当 `scope=sub_claim` 时的父 claim ID |
| `proposed_in_phase` | string | 是 | 提出阶段（`external` / `review` / `verify` 等） |
| `created_at` | ISO8601 | 是 | 创建时间 |
| `updated_at` | ISO8601 | 是 | 更新时间 |

Source 子对象中 `source_type` 枚举：`paper` / `experiment` / `derivation` / `dataset` / `code`。

## 3. Claim-level RFV 验证模式

Claim 级别的 RFV 验证采用 **单轮 review + verify** 模式，**不嵌套** RFV 子循环：

1. **review**：对单条 claim 执行来源追溯性检查和矛盾扫描
2. **verify**：执行 `verify_commands`（可复跑），将结果写入 `EVIDENCE_INDEX`
3. **update**：根据 verify 结果更新 claim 状态

这种模式避免了无限嵌套的 RFV 循环，同时保证每条 claim 都经过结构化验证。若一条 claim 的验证需要拆分，则通过 `scope=sub_claim` + `parent_section_id` 分解为子 claim，每个子 claim 独立走 review + verify 流程。

### Supervisor 自检

- [ ] 每条 `proposed` 状态的 claim 是否至少有一个可追溯来源？
- [ ] `contested` 状态的 claim 是否有 `contradiction_sweep` 记录？
- [ ] `supported` / `rejected` 的状态转移是否有 `evidence_ids` 支撑？

## 4. 与 EVIDENCE_INDEX 的关系

Claim Ledger 与 `EVIDENCE_INDEX` 通过 `evidence_ids` 字段关联：

- `EVIDENCE_INDEX` 中的行可通过 `scope: claim:<claim_id>` 标记为属于某条 claim
- `claim.evidence_ids` 中的每个 ID 应对应 `EVIDENCE_INDEX` 中一条记录
- Claim 的 `sources` 字段记录**来源信息**（可追溯性），`evidence_ids` 记录**验证记录**（可执行性）——两者互补

## 5. 与 closeout_gate 的集成

在 `closeout_gate` 检查中，Claim Ledger 一致性是可选检查项：

| 检查项 | 规则 |
|--------|------|
| **悬空引用** | `evidence_ids` 中的每个 ID 应在 `EVIDENCE_INDEX` 中存在 |
| **未终结 claim** | closeout 时不应存在 `proposed` 状态的 claim（应已推进到终态） |
| **矛盾未消解** | 所有 `contested` claim 的 `contradiction_sweep.status` 应为 `clean` |
| **来源可达性** | 所有 `sources` 中 `reachable: false` 的条目应有降级说明或 claim 状态为 `rejected` / `withdrawn` |

这些检查在 `my-light` profile 下为 advisory；非 `my-light` 时可由 `completion_gates` / `close_gates` opt-in 硬拦。

## 6. 使用示例

### 基本 Claim Ledger

```json
{
  "schema_version": "claim-ledger-v1",
  "task_id": "analysis-2026-q2-throughput",
  "generated_at": "2026-06-07T10:00:00Z",
  "updated_at": "2026-06-07T14:30:00Z",
  "claims": [
    {
      "claim_id": "cl-001",
      "text": "System throughput is bounded by the I/O subsystem under high concurrency.",
      "status": "supported",
      "sources": [
        {
          "source_type": "paper",
          "ref_id": "https://doi.org/10.1145/1234567",
          "reachable": true,
          "last_checked": "2026-06-07T10:05:00Z"
        },
        {
          "source_type": "experiment",
          "ref_id": "dataset:bench-io-v2.1",
          "reachable": true,
          "last_checked": "2026-06-07T10:05:00Z"
        }
      ],
      "evidence_ids": ["ev-bench-001", "ev-bench-002"],
      "contradiction_sweep": {
        "last_run": "2026-06-07T11:00:00Z",
        "contradictions": [],
        "status": "clean"
      },
      "scope": "claim",
      "parent_section_id": null,
      "proposed_in_phase": "external",
      "created_at": "2026-06-07T10:00:00Z",
      "updated_at": "2026-06-07T14:30:00Z"
    },
    {
      "claim_id": "cl-001a",
      "text": "The I/O bound applies specifically to sequential write paths, not random reads.",
      "status": "contested",
      "sources": [
        {
          "source_type": "paper",
          "ref_id": "arxiv:2601.12345",
          "reachable": true,
          "last_checked": "2026-06-07T12:00:00Z"
        }
      ],
      "evidence_ids": [],
      "contradiction_sweep": {
        "last_run": "2026-06-07T13:00:00Z",
        "contradictions": [
          {
            "description": "Random read benchmarks show CPU-bound behavior above 64 concurrent threads.",
            "source_ref": "https://doi.org/10.1145/9999999"
          }
        ],
        "status": "contested"
      },
      "scope": "sub_claim",
      "parent_section_id": "cl-001",
      "proposed_in_phase": "review",
      "created_at": "2026-06-07T12:00:00Z",
      "updated_at": "2026-06-07T13:00:00Z"
    }
  ]
}
```

### 与 RFV append_round 的配合

在 `append_round` 中，结构化 `external_research`（参见 [external-research-harness.md](external-research-harness.md)）产出 claims；supervisor 可选择性地将这些 claims 提取到 Claim Ledger 进行逐条追踪。两者的关系：

- `external_research.claims` — 外研 lane 的**原始输出**（每轮一份）
- `ClaimLedger.claims` — 跨轮次的**累积状态**（生命周期追踪）

Claim Ledger 不替代 `external_research` 的结构化校验（strict 模式仍由 `validate_external_research_strict` 执行）；它提供的是跨轮次的 claim 状态管理和一致性审计。

---

**See also**: [external-research-harness.md](external-research-harness.md)（结构化 external_research）、[reasoning-depth-contract.md](reasoning-depth-contract.md)（推理深度契约）、[lane-templates.md](lane-templates.md)（RFV lane 模板）。
