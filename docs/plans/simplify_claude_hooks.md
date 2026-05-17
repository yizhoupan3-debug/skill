# Plan: 精简 Claude Code 宿主上的重复轮子

> 范围限制：只改 Claude Code 的使用体验，不影响 Cursor/Codex。
> 架构事实：`claude_hooks.rs` 完全独立，不与其他宿主共享。共享的 `hook_common.rs` 和 `review_gate_engine.rs` 不动。

---

## 背景

调研确认 Claude Code 原生提供了若干本仓库在 `claude_hooks.rs` 中重复实现的能力（dangerous bash 检测、automation 教育提示等）。同时 review gate / goal / closeout / continuity 工件等 Claude Code 不提供，必须保留。

---

## 改动清单

### 🔵 Group A：安全删除（Claude-only，原生更好）

#### A1. 删除 dangerous_bash_reason()

**文件**：`scripts/router-rs/src/claude_hooks.rs` ~line 1504–1528

删除函数本身 + `run_pre_tool_use()` 中的调用分支。

理由：Claude Code 原生 permission system 已有 deny rules / auto classifier / circuit breaker（`rm -rf /` 即使 bypassPermissions 也弹窗）/ 只读命令白名单 / compound command 解析。

#### A2. 删除 prompt_mentions_automation()

**文件**：`scripts/router-rs/src/claude_hooks.rs` ~line 1486–1502

删除函数 + `run_user_prompt_submit()` 中的调用分支 + 常量 `AUTOMATION_CONTEXT`（~line 235–236）。

理由：Claude Code 原生有 `ConfigChange` hook + `PermissionRequest` + settings 编辑工具，用户说「from now on」时自动走 settings 路径，无需自检测。

#### A3. 删除 bash_write_target() 及 helpers

**文件**：`scripts/router-rs/src/claude_hooks.rs`

删除 `bash_write_target()`、`split_bash_segments()`、`bash_command_looks_mutating()`、`bash_segment_redirects_to_hint()`（~line 1589–1676）。

保留 `run_pre_tool_use()` 中对 `is_framework_guarded_path` / `is_generated_entrypoint` / `is_host_private_path` 的 `file_path` 检查。

### 🟡 Group B：不改但确认

- **skill 路由**（`AGENTS.md` → `SKILL_ROUTING_RUNTIME.json`）：项目 skills 在 `skills/`（无点前缀），不走 `~/.claude/skills/` 原生路径。迁移需单独项目，本次不碰。
- **continuity digest**：Claude Code 的 SessionStart 目前本来就**没有注册** hook，不存在需要砍的东西。auto memory 已能提供基本的跨会话回忆。
- **PreToolUse 路径守卫**：保留 `is_framework_guarded_path` / `is_generated_entrypoint` / `is_retired_surface` / `is_host_private_path`——这些保护框架文件不被意外修改，原生不提供。

### 🟢 Group C：必须保留（原生的确没有）

| 能力 | 理由 |
|------|------|
| Review Gate | Claude Code 无原生审稿门控 |
| Goal signal 检测 | 无原生 goal 状态机 |
| Closeout 强制 | 无原生 closeout |
| Rust lint auto-check | 无原生 cargo check 触发 |
| Touch state（settings 验证） | 无原生"改了必须验" |
| `payload_looks_like_cursor_hook_stdin` | 防误接保护 |
| Path guard（retired/generated/guarded） | 保护框架文件 |

---

## 改动文件清单

| 文件 | 改动 |
|------|------|
| `scripts/router-rs/src/claude_hooks.rs` | 删除 `dangerous_bash_reason()` + 调用；`prompt_mentions_automation()` + 调用；`AUTOMATION_CONTEXT`；`bash_write_target()` + `split_bash_segments()` + `bash_command_looks_mutating()` + `bash_segment_redirects_to_hint()` |
| 同文件测试区 | 删除 `denies_dangerous_bash`、`silent_for_safe_read_only_bash`、`claude_allows_user_settings_access` 测试；更新其他依赖函数签名的测试 |

---

## 验证

```bash
cargo build --manifest-path scripts/router-rs/Cargo.toml
cargo test --manifest-path scripts/router-rs/Cargo.toml
```

然后在 Claude Code 中实测：`git reset --hard HEAD` 应由原生拦住而非 router-rs；写 `AGENTS.md` 仍应被 PreToolUse 守卫拦住。
