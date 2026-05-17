# Plan: 完全迁移到宿主原生 Skill 系统

> **目标**：Claude Code 和 Codex 的自定义 skill routing 引擎全部迁移到各宿主原生能力。
> **不碰**：Cursor hooks、review gate、goal state machine、closeout enforcement、path guards、continuity digest（Cursor 专属）。

---

## 背景

当前仓库有 52 个 skills (`skills/<slug>/SKILL.md`)，通过自定义 Rust routing 引擎 (`scripts/router-rs/src/route/`) 做技能匹配。引擎通过 `browser_mcp` 以 MCP tools 暴露给宿主 agent。

### Claude Code 原生拥有
- `.claude/skills/<name>/SKILL.md` → 自动路由（description 字段 + when_to_use）
- Frontmatter: `description`, `when_to_use`, `disable-model-invocation`, `user-invocable`, `allowed-tools`, `context: fork`, `model`, `effort`, `paths`(glob)
- description + when_to_use 合计上限 1536 字符
- 技能内容注入后保持整个会话
- 技能清单预算: context window 的 1%

### Codex 原生拥有
- **无原生 skill 目录系统** — 但支持 SessionStart hook 注入 additionalContext
- 5 个 hook 事件: SessionStart, PreToolUse, UserPromptSubmit, PostToolUse, Stop
- `fork_context` 子 agent 隔离控制
- 5 个系统内置 skills: imagegen, openai-docs, plugin-creator, skill-creator, skill-installer

### 必须保留的能力（两个宿主都不提供）
- Review gate（审稿门控）
- Goal state machine（目标状态机）
- Closeout enforcement（关闭检查）
- Path guards（框架文件保护）
- Touch state validation
- Rust lint auto-check

---

## 方案总览

### Claude Code 路径
1. 为 52 个 skills 生成 `.claude/skills/<slug>/SKILL.md`（通过 sync pipeline）
2. 当前 `routing_layer` / `routing_owner` / `routing_gate` / `routing_priority` / `trigger_hints` 编码进 Claude Code 原生 `description` + `when_to_use`
3. Claude Code 原生自动路由替代 `route_task()` scoring 引擎
4. `skill_route` + `skill_search` MCP tools 按 host 隐藏；保留 `skill_read`

### Codex 路径
1. SessionStart 注入精简 skill catalog（52 行摘要）
2. 废弃 `artifacts/codex-skill-surface/` 符号链接生成
3. 同 Claude 路径: MCP routing tools 隐藏，skill_read 保留

### 共同路径
1. 删除 `route/` 模块（~15 文件）+ 相关 configs + 测试
2. 清理 `host_integration.rs` 路由依赖
3. 重写 `AGENTS.md` 模板、`.claude/rules/framework.md`、`.codex/prompts/framework.md`

---

## Phase 1: 准备工作

### 1.1 创建 NATIVE_SKILL_CATALOG.json
**新建** `configs/framework/NATIVE_SKILL_CATALOG.json`

52 个技能条目，每项包含:
```json
{
  "slug": "code-review-deep",
  "skill_path": "skills/code-review-deep/SKILL.md",
  "claude_native": {
    "disable-model-invocation": false,
    "user-invocable": true,
    "description": "Deep adversarial-style code review...",
    "when_to_use": "code review, 代码审查, ..."
  }
}
```

规则:
- `description` + `when_to_use` ≤ 1536 字符
- layer/gate/priority 语义编码进 description 文本
- 长 trigger_hints 列表（如 paper-workbench 的 80+ 条）精选 15-20 条
- Gate skills 标注 `[L0 gate:<type>]`
- Overlay skills 在 when_to_use 注明 overlay 语义

### 1.2 添加迁移 env flag
**修改** `scripts/router-rs/src/router_env_flags.rs`

```rust
ROUTER_RS_NATIVE_SKILLS=1        // Phase 2-4: 启用原生 skill surface
ROUTER_RS_DISABLE_CUSTOM_ROUTING=1 // Phase 5: 禁用路由引擎
```

### 1.3 审计 route/ 引用
确认所有对 `route/` 模块的 cross-reference，列在 Phase 5 清理清单中。

---

## Phase 2: Claude Code 原生 Skill Surface

### 2.1 生成 .claude/skills/ 目录
**新增** `ensure_claude_native_skills(repo_root)` in `host_integration.rs`

1. 读取 `NATIVE_SKILL_CATALOG.json`
2. 为每个 skill 写入 `.claude/skills/<slug>/SKILL.md`
3. 内容:
   ```markdown
   ---
   description: <compressed>
   when_to_use: <triggers>
   disable-model-invocation: <bool>
   user-invocable: <bool>
   ---
   
   See `skills/<slug>/SKILL.md` for the complete skill definition.
   ```
4. Gate: `ROUTER_RS_NATIVE_SKILLS=1`

### 2.2 集成到 sync pipeline
挂接到 `host_entrypoint_sync` 流程，在 hook projection 之前运行。

### 2.3 更新 framework 入口文档模板
**修改** `host_integration.rs` 模板 → `.claude/rules/framework.md`:
```markdown
Skills in `.claude/skills/` auto-route by description matching.
Read `skills/<slug>/SKILL.md` for full definitions.
```

---

## Phase 3: 隐藏 MCP Routing Tools

### 3.1 host-aware 工具过滤
**修改** `scripts/router-rs/src/browser_mcp/frag_01_through_types.rs`

`tool_definitions()` → 当 `--host-hint` 非空 + `ROUTER_RS_NATIVE_SKILLS=1`:
- 隐藏 `skill_route` + `skill_search`
- 保留 `skill_read`（纯文件读取）

### 3.2 CLI flag
browser_mcp entrypoint 新增 `--host-hint <claude-code|codex|>`.

---

## Phase 4: Codex Skill Catalog 注入

### 4.1 SessionStart catalog 注入
**修改** `scripts/router-rs/src/codex_hooks.rs`

`handle_codex_session_start`: `ROUTER_RS_NATIVE_SKILLS=1` →

```
## Repository Skills
- agent-swarm-orchestration [L0 gate:delegation]: Decide whether work should stay local...
- code-review-deep [L2 owner]: Deep adversarial-style code review...
... (52 lines, ~5200 chars total)
```

### 4.2 跳过 codex-skill-surface 生成
**修改** `host_integration.rs`: `ensure_codex_skill_surface()` → skip when flag enabled.

---

## Phase 5: 删除自定义路由引擎

全部 gate: `ROUTER_RS_DISABLE_CUSTOM_ROUTING=1`

### 5.1 删除文件列表

**route/ 模块 (15 files)**:
`route/routing.rs`, `scoring.rs`, `signals.rs`, `aliases.rs`, `constants.rs`, `gate_hints.rs`, `nl_route_adjustments.rs`, `policy.rs`, `records.rs`, `skill_record.rs`, `text.rs`, `types.rs`, `eval.rs`, `metadata_tests.rs`, `mod.rs`

**路由元数据 (7 files)**:
`skills/SKILL_ROUTING_RUNTIME.json`, `SKILL_ROUTING_RUNTIME_EXPLAIN.json`, `SKILL_MANIFEST.json`, `SKILL_ROUTING_METADATA.json`, `SKILL_ROUTING_REGISTRY.md`, `SKILL_ROUTING_INDEX.md`, `SKILL_ROUTING_LAYERS.md`

**路由配置 (2 files)**:
`configs/framework/ROUTING_SIGNAL_MARKERS.json`, `NL_ROUTE_ADJUSTMENTS.json`

**测试数据 (2 files)**:
`tests/routing_eval_cases.json`, `tests/routing_route_fixtures.json`

### 5.2 清理 browser_mcp
- 删除 `route_with_full_manifest_fallback`
- 删除 `skill_route` + `skill_search` handler
- 删除 `skill_runtime_path`, `skill_manifest_path`
- 删除 RouteDecision, SkillRecord imports

### 5.3 清理 host_integration.rs
- 删除 `filter_records_for_host`, `load_records*`
- 删除路由相关 generated artifacts
- 预计缩小 30-40%

### 5.4 清理 hook_common.rs
- 删除仅路由使用的函数（`is_goal_autopilot_prompt`, `has_goal_contract_signal`, `has_goal_progress_signal` 等）
- 保留 review gate / 通用工具函数

### 5.5 清理 cli/dispatch.rs
- 删除 `eval-route` 子命令

### 5.6 清理 main_tests.rs
- 删除 ~20 个 routing 测试

### 5.7 清理 path guard 条目
- `FRAMEWORK_GUARDED_PREFIXES` 中删除路由相关路径
- 保留 `configs/framework/RUNTIME_REGISTRY.json`

---

## Phase 6: 文档收尾

### 6.1 重写 AGENTS.md 模板
`host_integration.rs` 中的生成模板:
```markdown
## Claude Code
Skills in `.claude/skills/` auto-route. See `skills/<slug>/SKILL.md`.

## Codex
Skill catalog injected at session start. See `skills/<slug>/SKILL.md`.

## All Hosts
Review gate + path protection + closeout via hooks. Don't edit generated files.
```

### 6.2 更新 .codex 文档
- `.codex/prompts/framework.md` → 简化为入口描述
- `.codex/README.md` 模板 → 反映新架构

---

## 执行顺序

```
Phase 1 → Phase 2 ──→ Phase 3 ──→ Phase 4 ──→ Phase 5 ──→ Phase 6
           独立可验证   独立可验证   独立可验证   需 Phase 2-4
                                               稳定 1 周后执行
```

---

## 验证

### 构建
```bash
cargo build --manifest-path scripts/router-rs/Cargo.toml
cargo test --manifest-path scripts/router-rs/Cargo.toml
```

### Phase 2
- `.claude/skills/` 下 52 个目录 + SKILL.md
- 每个 description + when_to_use ≤ 1536 字符

### Phase 3-4
- `tools/list` (host-hint) 不含 skill_route/skill_search
- Codex SessionStart 输出含 52 行 catalog

### Phase 5
- route/ 删除后编译通过
- claude_hooks / codex_hooks / cursor_hooks 测试全部通过
- review gate / path guard / closeout 不受影响
- `skill_read` MCP tool 仍然可用

### 最终集成
- Claude Code: "review 这段代码" → 原生加载 code-review-deep
- Claude Code: "帮我写论文" → 原生加载 paper-workbench
- Claude Code: 写 `AGENTS.md` → PreToolUse 拦截
- Codex: SessionStart → 显示 skill catalog
- Codex: review gate → 仍然生效
