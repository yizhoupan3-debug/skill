# Cursor 宿主操作手册

**权威能力矩阵**：`configs/framework/RUNTIME_REGISTRY.json` → `host_projections.cursor`  
**接入契约**：`docs/host_adapter_contract.md`  
**官方文档**：[Hooks](https://cursor.com/docs/hooks) · [Rules](https://cursor.com/docs/rules)

## 安装 scope（ADR-003）

| 组件 | Scope | 路径 |
|------|-------|------|
| Framework 叙事（GSD 默认链） | **User only** | `~/.cursor/rules/framework.mdc` |
| Harness hooks | **Project** | `<repo>/.cursor/hooks.json` |
| Review / execution gate 规则 | **Project** | `<repo>/.cursor/rules/*.mdc`（非 framework） |
| Hook state | **Project** | `<repo>/.cursor/hook-state/` |

```bash
export SKILL_FRAMEWORK_ROOT=/path/to/skill   # 或在该仓库内工作
cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
  framework host-integration install --to cursor --scope user
```

**不要**在业务仓库跟踪 `framework.mdc`。成功标准 **不是** 与 Codex `AGENTS.md` 字节级一致。

## 默认工作流

全闭集宿主默认 **GSD**：`/gsd-new-project` → `/gsd-discuss-phase` → `/gsd-plan-phase` → `/gsd-execute-phase` → `/gsd-verify-work` → `/gsd-ship`。  
Pre-execution 三命令 **禁止改产品代码**（`skills/gsd/shared/phase-boundaries.md`）。

## Hook 能力（本仓）

- **Agent 面**：`beforeSubmitPrompt`、`stop`、`subagentStart`/`subagentStop`、`postToolUse` 等 → `router-rs cursor hook`
- **REVIEW_GATE 可清点 lane**（仅）：`general-purpose`、`best-of-n-runner`（registry `review_gate.deep_gate_lanes`）
- **Goal drive**：`/gsd-execute-phase`、`/gsd-verify-work`、`/gsd-ship` — **不是** `/gsd-new-project` 等 pre-exec 命令（`/autopilot` 已退役）
- **Fail-closed**：关键事件经 `cursor-router-rs-hook.sh`；`beforeSubmit` 对 plan 文件无可靠 Plan/Agent 模式信号

## 常用 env

| 变量 | 作用 |
|------|------|
| `ROUTER_RS_GSD_GOAL_CONTINUE_HOOK=0` | 关闭 Stop 续跑注入（兼容 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0`） |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED=1` | 开启 beforeSubmit pre-goal（绑定 `/gsd-execute-phase`） |
| `ROUTER_RS_CURSOR_HOOK_SILENT=1` | 压制非必要 hook 文案 |

## 自检

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint verify-cursor-hooks
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework host-integration status
cargo test --manifest-path scripts/router-rs/Cargo.toml host_integration
```

## Unsupported（勿假看齐）

- 无 Claude Code 专用 reviewer lane 字符串（`review`/`reviewer` 等）作为 REVIEW_GATE 清门依据
- 无 tmux `session_supervisor`（Codex CLI 独有）
- Tab / App lifecycle hooks 未接入 harness
