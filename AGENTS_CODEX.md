# Codex Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。本文仅 Codex 宿主差异与编译嵌入。

## 权威分层（改哪里才生效）

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议 | 仓库根 [`AGENTS.md`](AGENTS.md) |
| Codex 宿主差异 | **`AGENTS_CODEX.md`**（本文件） |
| Codex 策略快照 | 编译 embed（`AGENTS.md` + 本文件）；项目 sync 材料化 **`AGENTS_CODEX.md`** + `.codex/*` |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| hook 行为 | `.codex/hooks.json` + `router-rs` |

**文档地图**：[`docs/harness_architecture.md`](docs/harness_architecture.md) · [`docs/host_adapter_contract.md`](docs/host_adapter_contract.md) · [`docs/hosts/codex-cli.md`](docs/hosts/codex-cli.md)

## Codex 构建快照与同步逻辑（策略 A）

`router-rs` 编译期嵌入 **`AGENTS.md` + 本文件**（见 `build_codex_agent_policy`）。改跨宿主策略 → 改 [`AGENTS.md`](AGENTS.md)；改 Codex 差异 → 改本文件；二者均需 rebuild + sync。

```bash
cargo build --release --manifest-path core/router-rs/Cargo.toml
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint install-codex-user-hooks --framework-root "$PWD"
```

### 核心物化与同步机制

- **材料化**：`framework sync-entrypoints` 材料化 **`AGENTS_CODEX.md`**、**`.codex/README.md`** 与项目 `.codex/*`（见 `.codex/host_entrypoints_sync_manifest.json`）；**不** overwrite 仓库根 [`AGENTS.md`](AGENTS.md)（跨宿主内核，人工维护）。勿仅复制本文件到 `~/.codex/AGENTS.md`（会丢失跨宿主内核）。
- **编译嵌入**：`cargo build --release` 将 `AGENTS.md` + `AGENTS_CODEX.md` 静态嵌入二进制；hook 运行期不读盘。用户级 Codex 策略对齐见宿主文档；勿单独 cp 宿主 delta。

## Root

- Codex：`CODEX_HOME`（默认 `~/.codex`）；仓库内优先 `skills/` 与 `skills/SKILL_ROUTING_RUNTIME.json`。

## Language

- 跨宿主语言规范见 [`AGENTS.md`](AGENTS.md) § Language；Codex 宿主强制继承，不得豁免。

## Knowledge Hygiene

- 本文件是 Codex 地图；跨宿主正文在 [`AGENTS.md`](AGENTS.md)。

## 多代理编排 (Multi-Agent Orchestration)

Codex 宿主**鼓励积极使用多代理**以提升并行执行效率与 review 质量。与 Cursor 的 hook 级 `subagentStart`/`subagentStop` 门控不同，Codex 端的多代理行为由**文档契约与 agent 自觉**驱动，不依赖 hook 生命周期事件。

### 默认行为

- **`/implementx` 执行区**：`WAVE_STATE.json` 中 `execution_mode=parallel` 时，主线程**应主动 spawn 子代理**并行执行各 lane，主线程仅担任 scheduler。
- **深度 review**：spawn-first 配对审稿（`fork_context=false` 只读 reviewer）在非 my-light 时默认启用；my-light 下仍可按需 spawn。
- **≥2 独立子问题时默认并行**；通常 3–5 个 `fork_context=false` lane。

### 子代理契约

| 约束 | 说明 |
|------|------|
| 写入 disjoint | 各 lane 仅写 `scope_paths` 内文件，不修改共享 continuity artifact |
| 输出路径 | `artifacts/current/<task_id>/lane-notes/<lane_id>.md`，max 15 行 |
| `fork_context` | 深度 reviewer 必须显式 `fork_context: false`；`ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` 默认可推断 |
| 主线程可见内容 | coordinator visible content ≤35% of turn |

### 与 Cursor / Claude 的差异

- **Cursor**：有 `subagentStart`/`subagentStop` hook 事件 + 专用 gate 文件（`execution-subagent-gate.mdc`、`review-subagent-gate.mdc`）+ 模型继承规则。
- **Claude Code**：原生 `Task` 工具 + `PreToolUse` 硬阻断。
- **Codex**：无 hook 级子代理事件；多代理行为由本节文档契约 + `implementx` skill 契约约束。agent 应**同等积极**地使用并行 lane，不因缺少 hook 门控而退缩为主线程串行。
