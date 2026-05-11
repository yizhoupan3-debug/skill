---
name: harness-portable-core-claude-code
overview: 深度审查宿主与共享核心边界；以数据驱动收口 Codex/Cursor 硬编码；新增 Claude Code 为闭集第三宿主（L4 薄壳 + L3 复用），默认零影响既有 Codex/Cursor 安装与 hook 行为。
todos:
  - id: decouple-audit-doc
    content: "撰写宿主解耦审计 @ docs/plans/harness_host_decoupling_audit.md；Done: 文档含 (1)已 portable 面：L2 artifacts、framework_* CLI、hook_common、RUNTIME_REGISTRY 闭集元数据；(2)仍 duopoly 耦合点：`host_integration::canonical_tool_name` 仅 codex/cursor、`--to` 分支、projection_manifest 字面 host id、`cursor_hooks`/`codex_hooks` 分文件；(3) Claude Code 接入不改变上述默认路径的声明；Verify: test -f docs/plans/harness_host_decoupling_audit.md && rg -n 'canonical_tool_name|portable|duopoly|Claude' docs/plans/harness_host_decoupling_audit.md"
    status: pending
  - id: event-map-claude-code
    content: "产出 Claude Code ↔ 现有 codex/cursor hook 事件对照与缺口表 @ docs/plans/harness_host_decoupling_audit.md 新节或 docs/plans/claude_code_hook_event_map.md；Done: 表覆盖 SessionStart/Stop/PreToolUse/PostToolUse/UserPromptSubmit/Subagent* 与 router-rs 拟承载语义（续跑/证据/review），并标注官方 stdin/stdout 契约链接 https://docs.anthropic.com/en/docs/claude-code/hooks；Verify: rg -n 'SessionStart|PostToolUse|Subagent|hooks' docs/plans/claude_code_hook_event_map.md docs/plans/harness_host_decoupling_audit.md 至少其一存在"
    status: pending
  - id: registry-claude-code
    content: "注册 Claude Code 闭集宿主 @ configs/framework/RUNTIME_REGISTRY.json, tests/common/mod.rs；Done: host_targets.supported 含新 id（建议 `claude-code`）、metadata 含 install_tool（与官方 CLI 名一致，通常为 `claude`）与 host_entrypoints（至少 `AGENTS.md`，若用规则则对齐数组形状）；tests 内嵌 registry 与真源一致；Verify: cargo test --manifest-path scripts/router-rs/Cargo.toml framework_host_targets 或全量 router-rs 测绿"
    status: pending
  - id: rust-claude-hooks
    content: "实现 Claude Code hook 入口 @ scripts/router-rs/src/claude_hooks.rs（新）, scripts/router-rs/src/main.rs, scripts/router-rs/src/cli/dispatch_body.txt, scripts/router-rs/src/cli/dispatch.rs；Done: `router-rs claude hook <Event> --repo-root …` 与 codex/cursor 一样 stdin JSON→Rust→stdout JSON；首期可只实现与连续性/证据/续跑重叠的事件子集，其余 Event 透传或 no-op 并文档列出 roadmap；Verify: cargo test --manifest-path scripts/router-rs/Cargo.toml 且含新模块单测或 smoke"
    status: pending
  - id: host-integration-claude
    content: "扩展 framework install/status/remove 支持 `--to claude`（或 registry 驱动别名）@ scripts/router-rs/src/host_integration.rs；Done: 仅当用户显式选择该 target 时写入 `.claude/settings.json`（及/或项目 `.claude/`）中的 command hooks，指向 `router-rs claude hook …`；默认 `codex`/`cursor`/`all` 行为与生成物路径与 Round1 前一致（回归：`framework_maint`/`verify_*_hooks` 路径）；Verify: cargo test --manifest-path scripts/router-rs/Cargo.toml tests/host_integration.rs 或等价集成测"
    status: pending
  - id: canonical-tool-datadriven
    content: "将 `canonical_tool_name` / 用户可见 supported tools 列表改为以 RUNTIME_REGISTRY 的 install_tool 为主真源（失败信息同表）@ scripts/router-rs/src/host_integration.rs, scripts/router-rs/src/framework_host_targets.rs；Done: 新增 `claude` 后不再在 match 里手工追加第三个 arm 才能用 `--to`；Codex/Cursor 行为不变；Verify: cargo test --manifest-path scripts/router-rs/Cargo.toml"
    status: pending
  - id: docs-host-adapter
    content: "更新宿主契约文档 @ docs/host_adapter_contract.md, docs/harness_architecture.md §6；Done: 表格式映射增加 claude_hooks.rs 与 `.claude/settings.json`；第三宿主 PoC defer 语句改为「claude-code 已纳入闭集」或保留 defer 其它宿主；Verify: rg -n 'claude|claude-code' docs/host_adapter_contract.md docs/harness_architecture.md"
    status: pending
  - id: improvement-backlog-link
    content: "若已存在 harness 提升 backlog 则增加一节「Claude Code 宿主」链接本 plan；否则在 docs/plans/harness_improvement_backlog.md 写最小指针 @ docs/plans/harness_improvement_backlog.md；Done: backlog 或本 plan 其一可被 `rg harness_portable_core_claude_code` 交叉引用；Verify: rg 'harness_portable_core_claude_code|claude-code' docs/plans/harness_improvement_backlog.md .cursor/plans/harness_portable_core_claude_code.plan.md"
    status: pending
  - id: gitx-closeout
    content: "对照本 plan 与 docs/plans 变更做计划 vs 实际并 Git 收口 @ .cursor/plans/harness_portable_core_claude_code.plan.md, docs/plans/；Done: todos 逐项勾选或 defer；工作区仅有意变更；Verify: 在宿主执行 /gitx plan（与 /gitx 同契约，见 skills/gitx/SKILL.md）并联跑 git status -sb"
    status: pending
isProject: false
---

# 共享核心与 Codex/Cursor 解耦 — 深度审查 + Claude Code 宿主

## 结论（对抗式但基于代码）

**L2/L3 在叙事与多数实现上已与具体 IDE 解耦**（单一 `artifacts/current`、同一套 `framework_*` / `closeout` / `task_state`）；**L4 安装与 CLI 分发仍显著「双宿主形状」**：`host_integration.rs` 的 `canonical_tool_name` 仅识别 `codex`/`cursor`，`framework host-integration install` 的 `--to` 分支与投影路径大量手写；`codex_hooks.rs` 与 `cursor_hooks.rs` 为平行模块而非插件 ABI——这在工程上可接受，但**不等于**「任意第三宿主零成本插入」。要做到「新宿主完美享受全部能力」且「旧宿主零感知」，必须：**新增平行 L4 薄壳 + 复用 L3**，并对「工具名解析」等横切点做 **registry 数据驱动**，避免每加一个宿主就改一串 `match`。

## 零影响约束（硬）

- **默认路径**：未执行 `install --to claude`（或 `all` 扩展定义）时，**不**创建/修改 `.claude/settings.json` 中与 framework 相关的 hooks。
- **`all` 语义**：若将 `all` 扩展为含 Claude Code，必须在计划实现中明确是 **opt-in**（例如 `--to all` 仍仅 codex+cursor，另设 `--to all-with-claude`）或 **版本化迁移说明**；推荐首版：**`all` 不变**，Claude 仅 ` --to claude`，避免 CI/脚本静默行为变化。
- **共享 Rust 逻辑**：证据流、续跑合并、closeout、RFV 等 **禁止**为 Claude 复制第二套算法；与 [`docs/harness_architecture.md`](../../docs/harness_architecture.md) L3 规则一致。

## Claude Code 接入（你已选：CLI / hooks 链）

官方契约：**命令型 hook 为 stdin JSON → stdout JSON**；配置在 `~/.claude/settings.json` 与项目 `.claude/settings.json`（见 [Hooks reference](https://docs.anthropic.com/en/docs/claude-code/hooks)）。环境变量示例使用 `CLAUDE_PROJECT_DIR`。

### 能力对齐策略（「全部能力」）

| 能力域 | 落点 | Claude Code 侧做法 |
|--------|------|---------------------|
| 连续性 / snapshot / digest | 现有 L3 | `SessionStart` / `Stop` 等对齐 `codex_hooks` 已有路径 |
| PostTool 证据 | 现有 L3 | `PostToolUse`（及 Failure/Batch 若需）映射到同一 `EVIDENCE_INDEX` 启发式 |
| Autopilot / RFV 续跑 | 现有 L3 | `Stop` / `beforeSubmit` 等价事件（Claude 文档中的 `UserPromptSubmit` 等）按对照表合并 |
| Review / subagent 门控 | Cursor 特化 today | **Phase 2**：若 Claude Code 的 `SubagentStart`/`SubagentStop` 提供足够字段，复用 `review_gate` 核心纯函数 + 新 stdin 适配层；首期可文档标明「parity 未达 Cursor 级」 |
| Framework 规则投影 | `.mdc` / Codex prompts | Claude 使用 `CLAUDE.md` / `.claude/rules/*.md`（官方 `InstructionsLoaded`）；需新增 **render_claude_framework_entrypoint** 或等价，内容仍指向 `AGENTS.md` + `SKILL_ROUTING_RUNTIME.json` |

### 与现有 Round 2 计划关系

- [`harness_host_round2.plan.md`](harness_host_round2.plan.md) 的 P0（registry 单真源）与本 plan 的 **registry-claude-code**、**canonical-tool-datadriven** 可同 PR 链或顺序依赖；避免双轨回归。

## 风险与缓解

- **Hook JSON schema 漂移**：Claude Code 与 Codex/Cursor 字段名不同 → 在 `claude_hooks.rs` 做 **归一化层** 再调用现有 `hook_common` / `framework_runtime`，禁止三处复制正则。
- **`--settings` merge 行为**（上游已知 issue）：文档写明「团队 hooks 与本地覆盖」策略，避免用户以为关闭 hook 实际仍合并执行。

## 验证命令（非末条 todo 共用）

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml
```

若触及根包契约：`cargo test`（仓库根）。
