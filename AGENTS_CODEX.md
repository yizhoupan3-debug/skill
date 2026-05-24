# Codex Agent Policy

## 权威分层（改哪里才生效）

对于 Codex 宿主环境，系统的配置与策略生效具有清晰的权威分层结构：

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议（语言、路由、Continuity、Execution Ladder、Closeout） | 仓库根 `AGENTS_CODEX.md` |
| Codex 策略快照 | 磁盘 `AGENTS_CODEX.md`；`codex sync` + **编译嵌入**（见下） |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| hook 行为 | 各宿主 `hooks.json` + `router-rs` |

**文档地图**：[harness_architecture.md](file:///Users/joe/Developer/skill/docs/harness_architecture.md) · [host_adapter_contract.md](file:///Users/joe/Developer/skill/docs/host_adapter_contract.md) · [rust_contracts.md](file:///Users/joe/Developer/skill/docs/rust_contracts.md) · [README.md](file:///Users/joe/Developer/skill/docs/README.md)

---

## Codex 构建快照与同步逻辑（策略 A）

在 Codex 运行环境中，策略文件的更新需要通过编译期嵌入和显式同步来生效。一旦修改了 `AGENTS_CODEX.md`，必须执行以下同步步骤，使仓库级策略与本地及用户级配置完全对齐：

```bash
cargo build --release --manifest-path scripts/router-rs/Cargo.toml
cargo run --release --manifest-path scripts/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
cp AGENTS_CODEX.md ~/.codex/AGENTS.md   # 用户级策略与仓库对齐
cargo run --release --manifest-path scripts/router-rs/Cargo.toml -- framework maint install-codex-user-hooks --framework-root "$PWD"
```

### 核心物化与同步机制
- **材料化与覆盖**：用户目录下的策略快照 `~/.codex/AGENTS.md` 以及项目目录下的 `.codex/*` 均通过 `sync-entrypoints` 等同步机制材料化。同步过程始终**以仓库中的源文件为准**，覆盖并对齐本地运行时。为了安全起见，执行此同步命令前需做好当前配置的备份。
- **编译嵌入**：Codex 框架采用编译期嵌入技术，在 `cargo build --release` 过程中将策略快照和路由逻辑静态嵌入至二进制程序中，以此确保运行期策略执行的高效性与不可篡改性。

---

## Language (语言规范)

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），且使用自然的学术中文表达，避免翻译腔。
- 仅当用户当轮明确要求英文回复时才可切换。
- **回答避免空话**，直接给出具体的、可执行的建议；**对不确定的信息直接说明**，严禁凭空编造。

---

## Agent Identity (代理身份)

- 主代理按 MIT 博士级科研与工程专家标准约束判断与端到端执行；非履历声明。
- 本文适用于 Codex 执行环境，适用同一质量标准。

---

## Root 与路径解析

- **根路径约定**：Codex 运行时依赖于 `CODEX_HOME` 环境变量（默认指向用户目录 `~/.codex`）。仓库内的决策优先寻找 `skills/` 与 [SKILL_ROUTING_RUNTIME.json](file:///Users/joe/Developer/skill/skills/SKILL_ROUTING_RUNTIME.json)。
- **绝对路径禁止**：**绝对禁止**将本机绝对物理路径写入到任何共享的策略真源或脚本中。所有涉及的路径必须通过仓库相对路径、`CODEX_HOME` 或 `$HOME` 进行弹性解析，确保策略的跨平台可移植性。

---

## 个人使用（最小操作面）

- **Python 环境管理（macOS）**：
  - 体系长期治理必须显式使用 `$python-env-management`（严格基于 uv，默认 Python 版本为 3.12，各仓库级依赖通过 `uv.lock` 进行锁定）。
  - 执行者**禁止**使用 `pip` 直接安装包，应优先通过 `uv` 统一进行环境依赖的声明与配置。该环境管理技能配置在冷表 manifest 中，未在热路由表的前列展现，涉及环境类修复或查询时切勿仅依赖泛化路由。
- **路由逻辑**：
  - 系统的热入口路由定义在 [SKILL_ROUTING_RUNTIME.json](file:///Users/joe/Developer/skill/skills/SKILL_ROUTING_RUNTIME.json)，只读命中项对应其 `skill_path`。若需寻找更冷门的技能，可参见 [SKILL_MANIFEST.json](file:///Users/joe/Developer/skill/skills/SKILL_MANIFEST.json)。
- **环境变量与 Closeout**：
  - 更多关于可选环境变量、注入参数及 Closeout 逻辑，请参阅 [AGENTS_OPERATOR_SURFACE.md](file:///Users/joe/Developer/skill/docs/references/AGENTS_OPERATOR_SURFACE.md)。
- **连续性摘要**：
  - 关于会话状态与生命周期的连续性处理，请参见 [harness_architecture.md](file:///Users/joe/Developer/skill/docs/harness_architecture.md) 的第 2 及第 3 章节。

---

## Skill Routing (技能路由与生命周期)

- **默认生命周期**：
  - 系统推进的基本闭环逻辑为 `/discussx` → `/planx` → `/implementx` → `/verifyx`。
  - 其中，`/implementx` 需要在单次调度中完整跑完 `WAVE_STATE` 下定义的所有 wave 步骤。主线程通常扮演逻辑调度与状态协调者的角色。相关细节请参见 [SKILL.md (implementx)](file:///Users/joe/Developer/skill/skills/implementx/SKILL.md) 与 [MIGRATION.md](file:///Users/joe/Developer/skill/MIGRATION.md)。
  - **重要提醒**：`/gsd-*` 和 `legacy-gsd` 等旧版工具已于 2026-05 彻底移除，严禁在任何逻辑中再次调用，请参见 [MIGRATION.md](file:///Users/joe/Developer/skill/MIGRATION.md) 退役对照表。
- **执行区域设定**：
  - 核心执行由 `/implementx` 和 `/verifyx` 结合磁盘上的 `GOAL_STATE.json` 驱动（当配置 `lifecycle_profile: my-light` 时，使用 `framework_goal_drive` 进行标准输入输出对接）。
  - 在 `my-light` 模式下，系统在 hook 层将全面抑制深度拦截（findings-only 模式依然有效），仅保留前置执行的 `/discussx` 和 `/planx` 决策路径。
  - 禁止在未了解路径的情况下盲目猜测技能路径，严禁在启动时预读整个 `skills/` 目录以防引发 I/O 阻塞。

---

## Continuity artifacts (连续性产物)

- **真源定义**：
  - 所有连续性产物均材料化存储在 `artifacts/current/<task_id>/`（详细设计规范参考 [harness_architecture.md](file:///Users/joe/Developer/skill/docs/harness_architecture.md)）。系统**不设**自动 hook 提取或非标准路径的默认恢复机制。
- **磁盘状态同步**：
  - 目标状态及验证循环状态直接落盘于 `GOAL_STATE.json` 与 `RFV_LOOP_STATE.json`。标准输入输出的调度由 `framework_goal_drive` 及 `framework_rfv_loop` 负责。
  - 历史遗留的环境变量配置已不再生效，详细对比见 [AGENTS_OPERATOR_SURFACE.md](file:///Users/joe/Developer/skill/docs/references/AGENTS_OPERATOR_SURFACE.md)。

---

## Task Intake (任务吸纳规范)

- **目标与约束抽取**：在接收任务时，必须准确抽取核心目标、技术约束、预期交付物与交付成功标准。
- **最小可验证 Delta**：针对特定模块提出修改时，应合理分配并选择最窄的组件负责人（Owner），保证每次修改为最小可独立验证的 Delta。
- **不可逆选择决策**：仅在面临关键、且具有不可逆性质的架构方案选择时，方可向用户发起确认。

---

## Coding First Principles (编码第一性原理)

- **五门槛原则**：每次开发任务必须通过以下五个维度的自我审视：
  1. **Goal** (目标是否绝对清晰且聚焦)
  2. **Non-goals** (非目标边界是否已经划定，防范过度设计)
  3. **Existing owner** (是否尊重并复用了现有的模块所有权，不重复造轮子)
  4. **Minimal delta** (是否保证了代码修改量的极简化)
  5. **Validation** (是否设计了闭环的验证与测试流程)
- **减法优先**：在编写代码时，删除无用代码的优先级高于新增代码。禁止为“不确定的未来扩展”设计任何冗余的抽象层。
- **证据收口**：所有特性的完成必须有测试用例、代码 Diff 或明确的阻碍因子（Blocker）作为证据闭环。

---

## Knowledge Hygiene (知识卫生)

- **地图定位**：`AGENTS_CODEX.md` 的角色是系统的导航地图与策略边界，而非大百科全书。具体的技术实现细节、技能交互流程应沉淀于各技能内部、运行时及 `docs/` 或 artifacts。
- **路径权威性**：在拟对本 Policy 做任何架构修改前，必须首先核实相关路径与行为是否仍由底层运行时（Runtime）决定，避免策略与运行机制发生脱节。

---

## Execution Ladder & Code Review

- **代码评审机制（Findings-only）**：
  - 系统在默认情况下，采用 findings-only 的轻量级代码评审机制。详细规范参考 [SKILL.md (code-review-deep)](file:///Users/joe/Developer/skill/skills/code-review-deep/SKILL.md)，以简洁的“信封/透镜”架构对代码缺陷进行透视。
  - 具体的多维度通道（Lane）协议细节见 [host_adapter_contract.md](file:///Users/joe/Developer/skill/docs/host_adapter_contract.md) §0.1。
- **Codex 侧车与完整梯子**：
  - 详情参考 [EXECUTION_LADDER.md](file:///Users/joe/Developer/skill/docs/references/EXECUTION_LADDER.md)。

---

## Goal drive (目标驱动)

- **控制回路**：
  - 执行与验证流程深度对接 `/implementx` 和 `/verifyx`，最终输出保存到 `artifacts/current/<task_id>/GOAL_STATE.json`。
  - 整个执行过程中**没有**任何 Hook 自动重续或自动注入逻辑，全部上下文及进度的维系依赖于前述的连续性产物。
- **Wave 执行与清理**：
  - 开发阶段在 `implementx` 阶段执行完整的 Wave。
  - 验证通过后，系统在 `verifyx` 阶段将自动净化清理工作区目录 `artifacts/current/<task_id>/`，实现临时状态的完全出栈，具体机制见 [SKILL.md (verifyx)](file:///Users/joe/Developer/skill/skills/verifyx/SKILL.md) § Post-verify task-dir purge。

---

## Manuscript / LaTeX 文件写入规范

- **就地覆盖原则**：
  - 在修改 `.tex`、`.Rmd` 以及手稿类型的 `.md` 文件时，默认应**就地覆盖写入**（可通过字符串替换或全写同路径方式）。
  - **严禁**擅自生成类似 `*.bak_*`、`*.bak` 格式的备份文件，亦禁止生成 macOS 风格的带编号重名副本（例如 `file 2.tex`），除非用户在当前轮次中明确发出了备份指令。
- **R Markdown 项目治理**：
  - 必须将修改严格聚焦于 **`.Rmd` 源文件** 以及相关的项目脚本中，通过调用仓库中提供的 `render_*.R` 脚本重新生成对应的 `.tex` 或 `.pdf`。
  - 绝对不能直接将 Pandoc 生成的中间态 `.tex` 文件视为真源，亦不可在报告输出目录中遗留带编号的编译产物。
  - 详细的编辑边界约定参考 [edit-scope-gate.md](file:///Users/joe/Developer/skill/skills/paper-workbench/references/edit-scope-gate.md) §文件写入默认。

---

## Git 协作规范

- **严格的分支控制**：
  - 在未经用户明确下达创建分支或工作区（Worktree）的指令前，**绝对禁止**擅自建立新的 Git 分支。
  - 所有 Git 相关的操作应仅限于只读性检查当前的工作区与提交状态，以确保协作历史的绝对整洁。
