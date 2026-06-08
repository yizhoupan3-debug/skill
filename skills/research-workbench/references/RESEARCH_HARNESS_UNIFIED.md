# 科研 Harness 五宿主统一入口地图

**闭集宿主（2026-06）**：`codex` · `claude-code` · `cursor` · `antigravity` · `opencode` — 真源 `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`。

**原则**：五宿主共享同一套 **NL 热路由** 与 skill 契约；差异仅在 hook/MCP/子代理表面（见 `docs/host_adapter_contract.md`），**不**另建第二套科研 skill 拓扑。

## 统一前门（按对象选一条）

| 用户对象 | 前门 skill | 五宿主热路由 |
|----------|------------|--------------|
| 手稿 / 投稿 / 审稿 / 改稿 | `$paper-workbench` | ✅ 全宿主 |
| 实验设计 / 方法核查 / 非手稿科研 | `$research-workbench` | ✅ 全宿主 |
| 深度 Web 多源调研报告 | `$deep-research` | ✅ 全宿主（显式或 NL） |
| 引用 / BibTeX / DOI 卫生 | `$citation-management` | ✅ 全宿主 |
| 统计检验 / 效应量 / 功效 | `$statistical-analysis` | ✅ 全宿主 |
| 可复现 / 预注册 / 环境-seed | `$experiment-reproducibility` | ✅ 全宿主 |
| 严格推导 / 证明 | `$math-derivation` | ✅ 全宿主 |
| 科研出图代码 | `$scientific-figure-plotting` | ✅ 全宿主 |

手稿栈细节：`skills/paper-workbench/references/RESEARCH_PAPER_STACK.md`。

## research-workbench 专科 lane（二次吸收精华）

在 `research-workbench` 内部分类，**不**新增并列用户入口：

| Lane | 吸收的外部精华 | Reference |
|------|----------------|-----------|
| `experiment_design` | Academic Research / Repro Pack | 本 skill 正文 + `experiment-reproducibility` |
| `external_research` | Paper RAG（检索层）、Academic sources | [`academic-sources.md`](academic-sources.md) |
| `ref_corpus_qa` | Paper RAG（本地 FTS + 锚点问答） | [`ref-corpus-qa.md`](ref-corpus-qa.md) · CLI `ref-corpus` |
| `systematic_review` | Survey Builder / PRISMA | [`systematic-review-workflow.md`](systematic-review-workflow.md) |
| `grant_proposal` | Grant Writer | [`grant-proposal-workflow.md`](grant-proposal-workflow.md) |
| `math_verification` | Stats Sanity（方法层） | `$math-derivation` handoff |
| `code_verification` | Repro Pack | `$code-review-deep` / tests |
| `paper_handoff` | PaperSpine 手稿化 | `$paper-workbench` + prose chain |

## 确定性引文 gate（五宿主共用）

1. **Skill 层**：`$citation-management` + paperplain MCP（项目 `.mcp.json`；Codex 另写 `.codex/config.toml` `[mcp_servers.paperplain]`；Cursor/Antigravity/OpenCode 用户 MCP 投影）。
2. **CLI 层**（可选 fail-closed）：

```bash
cargo run -p citation_tool_rs -- audit --bib path/to/refs.bib --fail-on blocking
cargo run -p citation_tool_rs -- claim-lint --input manuscript.md --fail-on-findings
```

3. **手稿 gate**：`PAPER_GATE_PROTOCOL` G5 `citation_verify` → `$citation-management`。

## Data Availability / FAIR（手稿侧）

Nature-data 类精华并入 paper-workbench reference，五宿主同路径：[`../../paper-workbench/references/data-availability-fair.md`](../../paper-workbench/references/data-availability-fair.md)。

## 宿主差异（预期，非缺口）

| 能力 | cursor | claude-code | codex | antigravity | opencode |
|------|:------:|:-----------:|:-----:|:-----------:|:--------:|
| Paper prose L4 hook | ✅ | ✅ | ✅ | ❌ | ❌ |
| REVIEW_GATE multiset | ✅ | 部分 | 部分 | MCP | MCP |
| Native workflow (`deep-research.js`) | supervisor | ✅ `.claude/workflows/` | — | — | — |
| session_supervisor | ❌ | ❌ | ✅ | ❌ | ❌ |

Cursor 无 native workflow 时：读 `.claude/workflows/*.js` 的 `meta.phases`，按 [`../../agent-swarm-orchestration/references/workflow-supervisor-protocol.md`](../../agent-swarm-orchestration/references/workflow-supervisor-protocol.md) 手调度。

## Loadout

显式启用科研面：`research_loadout` → `skills/SKILL_LOADOUTS.json`（与 `FRAMEWORK_SURFACE_POLICY.json` 对齐）。

## 连续性

长周期科研工件：`artifacts/current/<task_id>/` + `paper_story/` + `paper_ref/`（见 `RESEARCH_PAPER_STACK.md` §科研纪录）。
