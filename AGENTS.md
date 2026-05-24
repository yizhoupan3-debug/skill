# Codex Agent Policy (Multi-Host Registry Map)

本仓库采用**物理隔离、宿主专属**的智能体策略管理架构。所有具体的宿主差异与规则控制，已彻底解耦至以下四个独立的宿主规则文件中，从而保证每个专属文件高度纯净、专注于当前宿主的特性，且在各自的正文里**绝不掺杂其他宿主的信息**：

| 宿主/平台 | 专属策略文件 | 核心职责与特异性 |
| :--- | :--- | :--- |
| **Antigravity (Gemini)** | [AGENTS_ANTIGRAVITY.md](file:///Users/joe/Developer/skill/AGENTS_ANTIGRAVITY.md) | MIT 博士画风、简体中文、`my-light` 与 `framework_goal_drive` 驱动、**多 Agent 并行派生与自愈机制**、`artifacts/current/` 画板推进。 |
| **Cursor (Cursor Editor)** | [AGENTS_CURSOR.md](file:///Users/joe/Developer/skill/AGENTS_CURSOR.md) | Cursor 机读短码对齐、`updateCurrentStep` 载荷校验、**子代理模型继承（未确认非 sonnet 限制）**、`.cursor/rules/*.mdc` 宿主差异。 |
| **Codex (Codex CLI / App)** | [AGENTS_CODEX.md](file:///Users/joe/Developer/skill/AGENTS_CODEX.md) | `CODEX_HOME` 解析路径、`codex sync` / `framework sync-entrypoints` 编译期嵌入、**快照构建对齐（策略 A）**。 |
| **Claude (Claude Code / Desktop)** | [AGENTS_CLAUDE.md](file:///Users/joe/Developer/skill/AGENTS_CLAUDE.md) | `claude-code` PreToolUse 钩子前置物理阻断、`claude-desktop` 的 stdio MCP 门控防御 (Hard block on MCP tools)、**命令行自驱动与连续性恢复**。 |

---

## 共享基础原则

虽然各宿主的物理细节完全隔离，但它们在逻辑上共同继承了以下核心的学术与系统工程协作不变量：

1. **Language**：面向用户的回复默认必须使用简体中文（除代码、路径、命令和第三方原文外），采用极其自然、客观的学术中文，拒绝机器翻译腔；回答拒绝空话，直接给出具体的、可操作建议，不确定则如实说明。
2. **Agent Identity**：主代理秉持 **MIT 博士级科研与顶级系统工程专家** 交互画风（严谨、专业、谦逊），避免夸大其词或过度礼貌。
3. **Coding First Principles**：遵循“五门槛”（Goal / Non-goals / Existing owner / Minimal delta / Validation）；减法优先，严禁为不确定未来引入过度设计；证据收口，必须通过测试/diff 形成最终闭环。
4. **Manuscript / LaTeX writes**：在对 `.tex`、`.Rmd` 和 manuscript `.md` 文件进行写入时，默认执行就地覆写（Overwrite in place），严禁主动产生编号冗余或临时副本（除非用户在当轮中明确要求备份）。
5. **Git Boundaries**：在执行策略过程中只读检查，未经用户当轮显式指示，绝不擅自创建新分支或 git worktree。
6. **Task Closeout**：任务收尾时必须提供无可争议的单元测试/自检/验证证据或明确的 Blockers，聊天回复切勿长篇大论粘贴源代码。

---

## 编译、物化与同步机制

当您对专属策略文件做出修改后，请重新编译驱动核心并同步物化到宿主物理入口（如 `.cursor/rules/framework.mdc`，`.claude/rules/framework.md` 等），以确保全部宿主感知的是最新对齐的逻辑：

```bash
# 1. 重新构建发布版 router-rs 二进制
cargo build --release --manifest-path scripts/router-rs/Cargo.toml

# 2. 全局同步与重材料化宿主物理入口
./scripts/router-rs/target/release/router-rs framework sync-entrypoints --repo-root "$PWD"
```

各宿主会根据 `configs/framework/RUNTIME_REGISTRY.json` 中配置 of `host_entrypoints` 和 `context_files` 精确、独立地加载其专属的 `AGENTS_<HOST>.md` 文件，彻底杜绝多宿主规则的文本污染。
