# Skill System Subtraction Execution Plan

日期：2026-04-25
范围：基于 `skills/SKILL_ROUTING_RUNTIME.json`、`skills/SKILL_TIERS.json`、`skills/SKILL_LOADOUTS.json` 与 `configs/framework/FRAMEWORK_SURFACE_POLICY.json` 的编译索引，推进 skill 合并、删除、下沉 runtime 的执行计划。

## 1. 执行目标

把 skill 系统从“很多入口都像 owner”继续压缩成：

```text
讨论 -> 规划 -> 执行 -> 验证
```

默认面只保留必要 gate 和最窄 owner 选择，不再把 controller、command、质量姿态、runtime 状态机包装成常驻 skill。

当前证据面：

- 总 skill 数：`125`
- default/core：`16`
- explicit opt-in：`109`
- 默认 loadout 的 owners/overlays：空
- 当前 runtime checklist 已使用“讨论/规划/执行/验证”

目标形态：

- default/core 从 `16` 压到 `12` 左右，只保留 source / artifact / evidence / delegation gate。
- command 和 runtime protocol 不再作为普通 skill 参与 owner 竞争。
- front-door workbench 保留，phase-only specialist lane 尽量下沉为 references 或内部模式。
- 所有删除或合并先经过 shadow/reroute 测试，不直接硬删。

## 2. 第一性原理判据

一个条目只有同时满足这些条件，才应该继续作为 skill：

1. 有明确任务对象，例如 paper、React、Docker、Sentry、PDF。
2. 有明确动作和交付物，例如 review、revise、deploy、render、audit。
3. 有领域专属约束，不只是通用“认真执行/持续推进/验证”。
4. 能选择最窄 owner，且不会被 runtime 四步协议替代。
5. 有可回归验证的触发边界。

应该下沉 runtime 的情况：

- 只是执行协议、completion pressure、验证要求、continuity、resume、trace、refresh。
- 只是 command alias 或宿主入口。
- 只是 supervisor/state machine，并不拥有领域对象。
- 只是“别偷懒/严格落实/推进到底”这类质量姿态。

应该合并的情况：

- 多个 skill 共享同一对象，只是阶段不同。
- front-door 已经存在，specialist lane 只是内部 mode。
- 用户不应该被迫在多个 sibling skill 之间先做选择。

应该删除或 deprecated 的情况：

- 没有独立 runtime 行为，只是旧入口兼容壳。
- 已有 front-door 或 runtime command 完整覆盖。
- 触发词主要是 alias，而不是领域任务。
- 保留它会扩大 default/core 面或制造 owner 竞争。

## 3. P0: 先压 default/core surface

当前 core 为：

```text
design-agent
doc
execution-controller-app
gh-address-comments
gh-fix-ci
idea-to-plan
openai-docs
pdf
playwright
sentry
skill-framework-developer
slides
spreadsheets
subagent-delegation
systematic-debugging
visual-review
```

建议保留为 default/core 的硬 gate：

| 类别 | 保留 |
|---|---|
| source gates | `openai-docs`, `gh-address-comments`, `gh-fix-ci`, `sentry` |
| artifact gates | `doc`, `pdf`, `slides`, `spreadsheets` |
| evidence gates | `playwright`, `visual-review`, `systematic-debugging` |
| delegation gate | `subagent-delegation` |

建议从 default/core 移出但暂不删除：

| Skill | 动作 |
|---|---|
| `execution-controller-app` | 改为 explicit app orchestration command/loadout，不再 `session_start: required` / P0 core。 |
| `idea-to-plan` | 改为 explicit planning owner 或 host plan-mode adapter，不再 L-1 core required。 |
| `skill-framework-developer` | 保持高精度 routing 命中，但不作为默认 core surface。 |
| `design-agent` | 改为 design evidence/source gate 的 explicit specialist，不默认挂载。 |

执行步骤：

1. 修改 tier 生成逻辑，不手改 generated JSON。
2. 把 core 判据改成“真正 gate + required first-turn”，排除 owner/controller/framework-owner。
3. 重新生成 `skills/SKILL_TIERS.json` 和 `configs/framework/FRAMEWORK_SURFACE_POLICY.json`。
4. 确认 `activation_counts.default` 从 `16` 降到约 `12`。

验证命令：

```bash
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json \
  --apply

jq '.summary.tier_counts, .summary.activation_counts, .tiers.core' skills/SKILL_TIERS.json
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
cargo test --test policy_contracts --quiet
```

## 4. P1: 下沉 runtime / command，不再作为普通 skill

这些条目优先转成 runtime command、framework command 或 internal mode：

| Candidate | 当前问题 | 目标形态 |
|---|---|---|
| `execution-controller-coding` | supervisor / continuity / multi-lane 集成是 runtime 状态机，不是领域 owner。 | 下沉为 `router framework/session-supervisor` 能力；保留只读 compatibility route 或 hidden internal skill 一个周期。 |
| `execution-controller-app` | APP 总控同时像 controller、delegation gate、quality gatekeeper，default P0 面过大。 | 转成 explicit `app-orchestration` loadout 或 framework command；普通 APP 单层任务路由给 frontend/backend/test owner。 |
| `deepinterview` | 本质是 clarification/review command mode，且 canonical owner 指向 `code-review`。 | 下沉为 explicit command mode；review 结论交给 `code-review` / `architect-review` / `execution-audit`。 |
| `refresh` | 生成下一轮执行提示是 runtime/host action，不是任务 owner。 | 转成 `router framework refresh` 或 Codex command，不进入 skill owner 竞争。 |
| `anti-laziness` | 质量姿态 overlay，不能替代 owner。 | 保持 overlay 或进一步下沉为 verification policy；只在 explicit quality-risk signal 时出现。 |
| `execution-audit` | 严格验收是 iteration/verification policy，容易变成第二中枢。 | 保持 explicit audit overlay；不要 default/core，不要抢 owner。 |

执行步骤：

1. 先给每个 candidate 加 routing regression，证明普通任务不会选它。
2. 把 command-like trigger 迁移到 `configs/framework/RUNTIME_REGISTRY.json` 或 router command help。
3. 将 skill frontmatter 降级为 `session_start: n/a`、optional 或 hidden/deprecated。
4. 维护一轮 `SKILL_SHADOW_MAP.json` reroute。
5. 第二轮再删除 skill 目录或移动为 `references/`。

必加 regression：

```text
推进到底，别停，并给验证证据 -> 不选 execution-controller-coding
启动 deepinterview 严格采访 -> command mode 或 code-review lane，不作为普通 owner 抢占
生成下一轮执行提示 -> router framework refresh，不选 refresh skill
APP 单独改 UI -> frontend-design，不选 execution-controller-app
APP 全栈总控显式请求 -> explicit app orchestration path
```

## 5. P1: 合并 sibling lane，保留 front-door

### 5.1 Paper group

当前结构：

```text
paper-workbench
paper-reviewer
paper-reviser
paper-writing
literature-synthesis
citation-management
```

建议：

- 保留 `paper-workbench` 作为 manuscript front-door。
- `paper-reviewer` 和 `paper-reviser` 改为 internal lane references 或 hidden specialist，一个 shadow 周期后再决定是否删除。
- 保留 `paper-writing`，因为 bounded prose rewrite 有独立对象和交付物。
- 保留 `literature-synthesis` 和 `citation-management`，因为它们跨 paper/research，且有独立 source/citation 约束。

判断理由：

- 用户不应该先选择 reviewer vs reviser。
- `paper-workbench` 已经能做 mode selection。
- reviewer/reviser 是同一对象 paper 的阶段拆分，适合作为 front-door 内部 lane。

### 5.2 Research group

当前结构：

```text
research-workbench
brainstorm-research
autoresearch
literature-synthesis
research-engineer
experiment-reproducibility
statistical-analysis
scientific-figure-plotting
ai-research
```

建议：

- 保留 `research-workbench` 作为 non-manuscript front-door。
- 暂时保留 `autoresearch`，因为它有 Rust controller、state discipline 和 experiment loop，是真实 runtime 行为。
- `brainstorm-research` 作为 early ideation lane 暂时保留，但降低 first-turn 优先级，避免所有 research 泛词触发它。
- 保留 `literature-synthesis`、`statistical-analysis`、`scientific-figure-plotting`，它们有清晰 artifact/evidence 约束。
- `research-engineer` 与 `ai-research` 需要后续看触发重叠，暂不删。

### 5.3 Design/frontend group

当前高重叠区域：

```text
design-agent
design-workflow
frontend-design
motion-design
css-pro
tailwind-pro
visual-review
accessibility-auditor
performance-expert
```

建议：

- `frontend-design` 保留为 UI design owner。
- `visual-review` 保留为 evidence gate。
- `accessibility-auditor` 和 `performance-expert` 保留为 specialist owner。
- `design-agent` 从 core 移出，改成 named-reference/source-grounding gate，只在“像 Linear / Stripe / verified tokens”等信号出现。
- `design-workflow` 可并入 `frontend-design` references，或只保留为 DESIGN.md artifact lane。
- `motion-design` 保留 specialist，因为 motion implementation 有独立约束。
- `css-pro` 与 `tailwind-pro` 暂不合并，分别服务 CSS architecture 和 Tailwind config，但不 default。

### 5.4 Presentation group

当前结构：

```text
slides
ppt-pptx
ppt-beamer
source-slide-formats
visual-review
pdf
```

建议：

- 保留 `slides` 作为 artifact gate。
- 保留 `ppt-pptx`、`ppt-beamer`、`source-slide-formats`，因为输出格式和工具链不同。
- 不新增 presentation-workbench，避免又多一个 front-door。
- 把共同流程下沉到 `skills/primary-runtime/slides/references/` 或 `skills/SKILL_FRAMEWORK_PROTOCOLS.md`，不要复制到每个 slide skill。

## 6. P2: 删除 / deprecated 候选

第一轮不要直接删除 domain specialist，只处理明显 command/runtime 壳。

| Candidate | 建议状态 | 删除前置条件 |
|---|---|---|
| `refresh` | deprecated -> command-only | `router framework refresh` 覆盖所有触发，routing eval 无直接 owner 依赖。 |
| `deepinterview` | deprecated -> command-only / review mode | `$deepinterview` command stub 正常，review lane tests 覆盖。 |
| `execution-controller-coding` | hidden/internal -> runtime-only | supervisor/continuity tests 全部走 router/framework runtime，不再需要 skill route。 |
| `execution-controller-app` | explicit command/loadout -> maybe hidden | APP orchestration 有独立 command 或 loadout，普通 APP route 不再需要它。 |
| `paper-reviewer` | hidden lane | `paper-workbench` 能覆盖 whole-paper review、single-dimension review、external calibration tests。 |
| `paper-reviser` | hidden lane | `paper-workbench` 能覆盖 reviewer-comments、known findings、rebuttal/edit tests。 |

不要删除：

- source/artifact/evidence gate。
- 有真实文件格式、部署平台、语言运行时、学术统计、引用真实性等硬约束的 specialist。
- 用户特定但明确的 owner，例如 `sustech-mailer`、`tao-ci`，它们应该 explicit opt-in，而不是 default。

## 7. 实施顺序

### Batch A: Core surface shrink

目标：default/core 从 16 降到硬 gate 集合。

改动面：

- skill tier 生成逻辑
- `configs/framework/FRAMEWORK_SURFACE_POLICY.json`
- generated `skills/SKILL_TIERS.json`
- policy tests

验收：

- `execution-controller-app`、`idea-to-plan`、`skill-framework-developer`、`design-agent` 不在 `.tiers.core`。
- 对应 route query 仍能命中正确 owner。

### Batch B: Command/runtime downshift

目标：refresh、deepinterview、execution controllers 不再作为普通 owner 竞争。

改动面：

- `configs/framework/RUNTIME_REGISTRY.json`
- router command/help/migration guidance
- routing eval cases
- affected skill frontmatter 或 deprecation stubs

验收：

- command explicit entrypoint 可用。
-普通任务不选 controller/refresh/deepinterview。
- compatibility error 有迁移指引，不 fall through 到 clap unexpected argument。

### Batch C: Paper lane merge shadow

目标：`paper-workbench` 成为唯一 manuscript front-door。

改动面：

- `paper-workbench` triggers / lane map
- routing eval cases
- `SKILL_SHADOW_MAP.json`
- reviewer/reviser deprecation note

验收：

- `帮我审这篇论文能不能投` -> `paper-workbench`
- `根据 reviewer comments 改到能投` -> `paper-workbench`
- `$paper-reviewer` / `$paper-reviser` 显式入口仍可临时命中或给迁移指引

### Batch D: Design lane cleanup

目标：去掉 design-agent 的 default/core 地位，并减少 design-workflow/front-end design 重叠。

改动面：

- `design-agent` frontmatter
- `frontend-design` references
- routing eval cases

验收：

- `像 Linear 一样，先核查品牌 token` -> `design-agent`
- `把这个页面改高级` -> `frontend-design`
- `看截图找 UI 问题` -> `visual-review`

## 8. 验证矩阵

每个 batch 都要跑：

```bash
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json \
  --apply

cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
cargo test --test policy_contracts --quiet
```

重点手工 route probes：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '减法原理和第一性原理深度review我的skill系统，看看是不是明确的行为驱动了，讨论，规划，执行，验证！'
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '按方案实现这个仓库修复，直接做代码，推进到底，别停，并给我验证证据'
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '帮我审这篇论文能不能投，先严审再决定怎么改'
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '根据 reviewer comments 把这篇论文改到能投'
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '帮我推进这个科研方向，判断下一步该搜文献还是做实验'
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '生成下一轮执行提示并复制'
```

## 9. 停止条件

停止当前 batch 并回滚该 batch 的条件：

- source/artifact/evidence gate 被普通 owner 抢走。
- route context 中 completion pressure 又改变 selected owner。
- default/core 降低后高频任务首次路由失败。
- command-only 迁移导致显式入口不可用。
- paper/research/design front-door 合并后无法命中明确 named lane。

## 10. 当前建议的下一步

优先执行 Batch A。

原因：

- 风险最低，只改变 default/core 表面积，不删除 skill。
- 与当前 `FRAMEWORK_SURFACE_POLICY` 的 kernel 原则一致。
- 能立刻减少“默认系统像多个中枢”的问题。
- 给后续 command/runtime downshift 和 paper lane merge 提供更干净的基线。

Batch A 完成后，再执行 Batch B。不要先删 `paper-reviewer`、`paper-reviser` 或 research/design specialist；这些需要 route shadow period 和真实 query regression。
