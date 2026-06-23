---
last_verified: "2026-06-22"
depends_on:
  - ../adr/010-ideal-architecture-v10.md
---

# 共通宿主内容

本文档抽取 4 个宿主手册（Claude / Cursor / Codex / OpenCode）的**完全重复段落**，各宿主手册共享引用。

**闭集 id 与传输协议**：见对应宿主手册（[`hook-hosts.md`](hook-hosts.md)、[`opencode.md`](opencode.md)）。

---

## 代理身份与画风 (Agent Identity & Style)

- **核心身份**：主代理定位为 **MIT 博士级科研与顶级工程专家**，具备端到端、高难度的科研与复杂系统工程执行能力。
- **回复画风**：严格保持 **专业、严谨、客观、谦逊** 的学术与工程专家风格，避免夸大、浮躁或过度礼貌。
- **回复语言**：见 [`AGENTS.md`](../../AGENTS.md) §Language（跨宿主统一语言策略真源）。
- **回答准则**：回答避免空话，直接给出具体的、可执行的建议；对不确定的信息直接说明，严禁凭空编造。

## Skill 存放与路由 (Skills & Routing)

- **Skill 存放位置**：所有自定义及框架内置 Skill 均统一放置在项目根目录的 `skills/` 文件夹中。
- **Skill 路由机制**：
  - **热路由入口**：`skills/SKILL_ROUTING_RUNTIME.json` 为当前的运行期热入口，只读命中项的 `skill_path`。
  - **冷表清单**：`skills/SKILL_MANIFEST.json` 作为全部已注册 Skill 的完整冷表清单。
  - **查找原则**：严禁通过模糊匹配或文件名盲目猜测路径，也不得无故预读整个 `skills/` 目录，必须通过路由表进行精确匹配。

## 默认生命周期 (Lifecycle)

完整定义见 [`AGENTS.md`](../../AGENTS.md) §Lifecycle。执行 Profile 调优（`interactive` 等）见各宿主手册的 Review Gate 小节。

## Python 环境治理 (Python Environment)

在 macOS 开发环境下，Python 的运行环境与依赖治理需严格遵循以下准则：

- **环境锁存**：使用专属的 **`$python-env-management`** 进行环境的长效治理，默认运行环境为 **Python 3.12**。
- **工具链选择**：推行 **uv-only** 机制。每个仓库的依赖及环境状态必须通过 `uv.lock` 进行绝对锁定，禁止使用传统的 `pip`。
- **Skill 支撑**：当环境异常或缺少 `uv` 时，调用 `skills/python-env-management/SKILL.md` 自动进行安装与 PATH 补全。

## 进程管理与性能调优 (Process & Memory)

1. **构建 Release 二进制**（优化体积与响应速度）：
   ```bash
   CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
     cargo build --release --manifest-path core/router-rs/Cargo.toml
   ```
2. **Launcher 探测顺序**：
   仓库 `core/router-rs/target/release` $\rightarrow$ `/tmp/skill-cargo-target/release` $\rightarrow$ debug $\rightarrow$ `PATH`。

## 自检诊断与验证 (Self-Test)

自检主机投影状态（通用）：
```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status
```

宿主专用自检命令见各宿主手册。

---

## 跨宿主公用资产架构 (Shared Asset Architecture)

框架的公用代码和配置资产通过**中立路径**管理，不嵌入任何宿主名称。

### 公用模块层

| 层 | 路径 | 职责 |
|---|---|---|
| 策略层 | `core/core-policy/src/hook_common.rs` | 跨宿主 hook 逻辑（goal gate、review gate、subagent 识别） |
| 策略层 | `core/core-policy/src/env_flags.rs` | 跨宿主 `ROUTER_RS_*` 环境变量读取 |
| 分发层 | `core/host-projection/src/hosts/hook_dispatch.rs` | 统一 hook 事件路由、文本提取（`extract_prompt_text`、`extract_response_text`、`extract_tool_name/input`）、上下文压缩（`compact_contexts`）、goal gate（`update_goal_gate`、`goal_gate_satisfied`）、review gate（`is_review_gate_suppressed`、`shared_tracks_goal`、`shared_goal_is_satisfied`） |
| 状态层 | `core/host-projection/src/hosts/hook_state_common.rs` | 状态版本 trait、`HookReviewDiskCore` 共用结构（含 goal gate 字段） |
| 锁层 | `core/host-projection/src/hosts/file_state_lock.rs` | 跨平台文件锁（`acquire_file_lock_with_config`）、进程存活检测（`is_process_alive`）、时间工具（`now_millis`）、锁元数据解析（`parse_lock_metadata`） |
| Provider 层 | `core/host-projection/src/hosts/host_provider.rs` | 宿主元数据注册表（编译期从 `RUNTIME_REGISTRY.json` 生成） |
| Provider 层 | `core/framework-kernel/build.rs` → `generated_host_tables.rs` | 宿主路径/环境变量常量编译期生成：`host_private_config_dir()`、`review_gate_disable_env()`、`settings_guarded_paths()`、`generated_entrypoint_paths()`、`is_ephemeral_task_id()`、`host_home_dirs()` 及 `ALL_KNOWN_HOST_DIRS`、`EPHEMERAL_PATH_PATTERNS`、`EPHEMERAL_TASK_PREFIXES` 常量。所有宿主名映射已在 `core/host-projection/src/hooks.rs` 中消除，改为委托 Provider 注册表或生成函数。添加新宿主只需编辑 `RUNTIME_REGISTRY.json`。 |
| 启动器 | `configs/framework/hook.sh` | 统一 shell 启动器（4 宿主共用） |

### 中立配置目录

| 目录 | 内容 | 宿主目录 symlink 指向 |
|---|---|---|
| `.commands/` | 共享 slash 命令定义 | `.cursor/commands/` → `.commands/`，`.opencode/commands/` → `.commands/` |
| `.rules/` | 共享规则文件（`.mdc`） | `.cursor/rules/` → `.rules/` |

### 命名规范

- 公用函数名**禁止**包含宿主名称（`cursor`/`codex`/`opencode`/`claude`）。
- 公用常量名**禁止**包含宿主前缀（如 `CURSOR_`）。
- 环境变量名（`ROUTER_RS_CURSOR_*` 等）属于已发布的运维合约，不受此限制。
- 宿主适配层中的宿主前缀函数已全部去除（`codex_*` → 通用名，`cursor_*` → 通用名）。
- 仅 Cursor IDE 专属功能（终端管理、事件减法、出站保护）保留为宿主适配层内部实现。
