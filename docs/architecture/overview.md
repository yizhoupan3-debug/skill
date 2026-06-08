---
last_verified: "2026-06-02"
depends_on:
  - ../../AGENTS.md
  - ../../README.md
---

# 架构总览

本文档是仓库入门级架构地图。目标读者：首次接触本仓库的开发者或 AI agent，需要在 10 分钟内理解「这个仓库是什么、各组件怎么连接、修改哪里才安全」。

详细契约、环境变量表、宿主差异见 `docs/` 子文档。

## 1. 仓库定位

本仓库是一套面向 AI coding agent（Codex、Cursor、Claude Code、Antigravity）的 skill 路由与执行治理框架。核心目标：

- **Skill 路由**：给定用户意图，从 `skills/` 中选出最窄匹配的 skill，注入对应 prompt。
- **执行治理**：通过 hook 和 Rust 控制平面，约束 agent 在 review、closeout、goal drive 等关键节点的行为。
- **多宿主适配**：同一套 skill 和策略，通过宿主投影层适配不同 AI coding 平台。

## 2. 默认生命周期

```
/discussx  ->  /planx  ->  /implementx  ->  /verifyx
(讨论)        (规划)       (执行全部 wave)    (验收并清理)
```

`implementx` 一口气执行 `WAVE_STATE.json` 中所有 wave；`verifyx` 验收后清理 `artifacts/current/<task_id>/`。

## 3. 源码地图

```
.
+-- AGENTS.md                          # 跨宿主策略真源
+-- AGENTS_{CURSOR,CODEX,CLAUDE,ANTIGRAVITY}.md  # 宿主差异
+-- Cargo.toml                         # workspace 根
+-- Justfile                           # 开发命令
+-- cli/
|   +-- opencode/                      # OpenCode MCP 宿主
+-- core/
|   +-- antigravity/                   # 状态管理纯库（task_state、state_manager、rfv_loop）
|   +-- router-rs/                     # 核心控制平面二进制（hook、路由、门控、CLI）
|   +-- autoresearch-rs/               # 自动研究引擎
|   +-- evolution-rs/                  # 进化审计
+-- configs/
|   +-- framework/                     # 框架配置（registry、narrative、hook 脚本）
|   +-- codex/                         # Codex 特定配置
+-- skills/
|   +-- SKILL_ROUTING_RUNTIME.json     # 热路由真源
|   +-- SKILL_MANIFEST.json            # 冷热 manifest
|   +-- <skill-name>/SKILL.md          # 各 skill 定义
+-- rust_tools/                        # 独立 Rust 工具 crate（citation、image、pptx 等）
+-- scripts/
|   +-- cursor-bootstrap-framework.sh  # 跨仓库接入脚本
|   +-- ci/                            # CI 脚本
+-- tests/                             # 集成测试（policy、host、routing eval）
+-- docs/                              # 契约文档
|   +-- architecture/                  # 架构文档（本文档所在位置）
|   +-- harness_architecture/          # 五层模型详细设计（已拆分）
|   +-- rust_contracts/                # Rust 实现契约（已拆分）
|   +-- hosts/                         # 各宿主接入手册
|   +-- references/                    # 扩展参考
+-- artifacts/                         # 运行产物（不入版本库）
+-- .cursor/                           # Cursor 工作区配置
+-- .claude/                           # Claude Code 配置
+-- .codex/                            # Codex 配置
+-- .github/workflows/                 # CI 流水线
```

## 4. 子文档导航

| 主题 | 文档 |
|------|------|
| 组件详解（skill 体系、router-rs、antigravity、configs） | [components.md](components.md) |
| 数据流（用户请求、skill 路由、goal drive、证据流） | [data-flow.md](data-flow.md) |
| 安全模型（测试、CI、schema drift、生成物 drift） | [security.md](security.md) |
| 宿主集成（宿主列表、hook 差异、shell launcher、跨仓库接入） | [host-integration.md](host-integration.md) |
