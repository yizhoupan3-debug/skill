# Claude Desktop 宿主操作手册

**闭集 id**：`claude-desktop` · **传输**：MCP stdio · **权威**：`RUNTIME_REGISTRY.json` → `host_projections.claude-desktop`

## 能力边界（诚实门控）

Desktop **没有** CLI 级 shell hook，因此：

- **无** PreToolUse / Stop 硬拦截（registry `harness_capability_exceptions`）
- **无** REVIEW_GATE 子代理 hook 面
- 门控可靠 = **MCP 工具工作流** + 短投影文案，不假装 hook 已拦截
- **叙事对齐**：深度 review 用 **spawn-first 配对审稿**（先 spawn 可数 reviewer；窄范围/small_task 不 block）；见 `skills/code-review-deep/SKILL.md`

## 安装

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
  framework host-integration install --to claude-desktop --repo-root "$PWD"
```

## 推荐 MCP 工作流（降 token）

1. 会话开头：`framework_digest`（一次）
2. 路由：`skill_route`
3. 任务：`goal_state_manage operation=start`
4. 验证后：`record_evidence`
5. 收尾：`closeout_gate` → `goal_state_manage operation=complete`

工具全集与限制：项目 `.claude/CLAUDE.md`（应保持 ≤40 行指针）。

## 与 CLI 共享

`artifacts/current/` 与 `claude-code` 共用；切换宿主时 continuity 可延续。

## 自检

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework host-integration status
```
