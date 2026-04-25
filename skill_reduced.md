# Skill 系统减法精简 Checklist

## 目标

- [ ] 将当前 skill 系统从“大而全的技能库”收敛为“少入口、窄 owner、强 gate、可解释”的运行系统。
- [ ] 优先删除历史兼容壳、重复入口、低价值专科 skill，而不是继续增加边界说明。
- [ ] 将当前约 `145` 个 skill 压缩到约 `90-100` 个有效入口。
- [ ] 将默认可见面从约 `26` 个入口压缩到约 `12-16` 个。
- [ ] 保留 Rust-owned / Codex-only 的运行时方向，不恢复 Claude/Gemini/Python bridge 兼容面。
- [ ] 保留“一个 front door + 少量内部 mode/reference”的结构，避免多个同类 skill 争抢首轮路由。

## 总原则

- [ ] 能删则删：如果 skill 只是旧兼容、旧命名、旧流程别名，删除优先于合并。
- [ ] 能合则合：如果两个 skill 的触发语、工作流和产物高度相同，只保留一个 owner。
- [ ] 能降级则降级：如果 skill 只是某个 owner 的检查维度，改成 `references/`、mode、checklist section，不保留独立路由入口。
- [ ] 能显式 opt-in 就不要默认加载：低频专家 skill 不进入 default surface。
- [ ] 保留 gate 的理由必须是“对象类型或证据源先于 owner”，不是“我也很重要”。
- [ ] 每个任务最多一个 owner、最多一个 overlay。
- [ ] 不为“未来可能用到”保留独立 skill；未来高频再恢复。
- [ ] 不用健康分当唯一依据；健康但低频、边界重叠、入口噪声大的 skill 也可以合并或降级。
- [ ] 不做全库重写式美化；每轮只处理一个重叠簇，验证后再进入下一簇。

## 当前基线

- [ ] 记录当前 skill 数量：`skills/SKILL_HEALTH_MANIFEST.json` 约 `145`。
- [ ] 记录当前 tier：`core=20`、`optional=123`、`experimental=2`、`deprecated=0`。
- [ ] 记录当前 default activation：约 `26`。
- [ ] 记录当前明显异常：`iterative-optimizer` 使用高但 reroute 高。
- [ ] 记录当前明显异常：`github-investigator` 动态分低且已有 reroute。
- [ ] 记录当前结构问题：skill 维护簇入口过多。
- [ ] 记录当前结构问题：paper/design/research 采用 front door 后仍保留过多同级专家入口。
- [ ] 记录当前结构问题：artifact gate 存在 `spreadsheets` / `xlsx` 双入口。
- [ ] 记录当前结构问题：`autopilot` / `team` 已声明 canonical owner，但仍作为 skill 入口存在。

## 不动范围

- [ ] 不删除 `.system/skill-creator`，因为它是具体 skill package 创建/更新的系统入口。
- [ ] 不删除 `.system/skill-installer`，但默认不进入本地框架治理入口。
- [ ] 不删除 `skill-framework-developer`，它是 skill 框架治理唯一主 owner。
- [ ] 不删除核心证据 gate：`systematic-debugging`、`visual-review`、`playwright`。
- [ ] 不删除核心 source gate：`openai-docs`、`gh-address-comments`、`gh-fix-ci`、`sentry`。
- [ ] 不删除核心 artifact gate：`pdf`、`doc`、`slides`、`spreadsheets`。
- [ ] 不删除主执行入口：`idea-to-plan`、`execution-controller-coding`、`plan-to-code`、`gitx`。
- [ ] 不删除语言专家簇第一轮：`python-pro`、`typescript-pro`、`javascript-pro`、`rust-pro`、`go-pro`、`sql-pro` 先保留为显式 opt-in。
- [ ] 不删除近期工作中的用户变更；任何删除前先确认文件确实属于本轮减法目标。

## P0：历史兼容残留删除

### 目标

- [ ] 确保仓库只保留 Codex-native / Rust-owned skill framework 面。
- [ ] 删除旧宿主兼容壳，减少误读和维护分叉。

### 删除候选

- [ ] 删除 `.claude/commands/refresh.md`。
- [ ] 删除 `.claude/hooks/README.md`。
- [ ] 删除 `.claude/settings.json`。
- [ ] 删除 `.claude/skills`。
- [ ] 删除 `.gemini/memory`。
- [ ] 删除 `.gemini/settings.json`。
- [ ] 删除 `.geminiignore`。
- [ ] 删除 `AGENT.md`。
- [ ] 删除 `CLAUDE.md`。
- [ ] 删除 `GEMINI.md`。
- [ ] 删除 `docs/claude_entrypoint_maintenance.md`。
- [ ] 删除 `openai_proxy/start_anthropic_bridge.sh`。
- [ ] 删除 `rust_tools/anthropic_openai_bridge_rs/`。
- [ ] 删除 `scripts/router-rs/src/claude_hooks.rs`。
- [ ] 删除 `scripts/router-rs/src/framework_mcp.rs`。
- [ ] 删除 `plugins/skill-framework-native/.mcp.json`。

### 验收

- [ ] `rg -n "Claude|Gemini|Anthropic|anthropic|claude|gemini" AGENTS.md configs docs scripts skills plugins rust_tools openai_proxy` 不再出现运行时入口要求。
- [ ] 允许历史说明出现，但不得作为当前运行路径、同步路径、入口路径。
- [ ] `configs/framework/RUNTIME_REGISTRY.json` 不再引用已删除旧宿主入口。
- [ ] `scripts/router-rs` 编译通过。
- [ ] routing 文档只描述 Codex-native / Rust-owned surfaces。

## P0：旧重复 skill 删除

### 目标

- [ ] 删除已经被新 skill 替代的旧 skill 目录。
- [ ] 避免同一产物有两个名字、两个路由、两个 reference 树。

### 删除候选

- [ ] 删除 `skills/.system/imagegen/`。
- [ ] 删除 `skills/imagegen/`。
- [ ] 保留 `skills/image-generated/` 作为唯一生图入口。
- [ ] 删除 `skills/.system/openai-docs/`。
- [ ] 保留 `skills/openai-docs/` 作为唯一 OpenAI 官方文档 gate。
- [ ] 删除 `skills/claude-api/`。
- [ ] 删除 `skills/ppt-html-export/`。
- [ ] 删除 `skills/ppt-markdown/`。
- [ ] 删除 `skills/slides-source-first/`。
- [ ] 保留 `skills/source-slide-formats/` 作为 Markdown / Slidev / Marp / HTML slide source 入口。
- [ ] 删除 `skills/skill-developer/`。
- [ ] 删除 `skills/skill-installer-antigravity/`。
- [ ] 删除 `skills/skill-library-maintenance/`。

### 验收

- [ ] `find skills -mindepth 1 -maxdepth 2 -name SKILL.md | wc -l` 明显下降。
- [ ] `skills/SKILL_MANIFEST.json` 不包含已删除 slug。
- [ ] `skills/SKILL_ROUTING_RUNTIME.json` 不包含已删除 slug。
- [ ] `skills/SKILL_HEALTH_MANIFEST.json` 不包含已删除 slug。
- [ ] `skills/SKILL_SHADOW_MAP.json` 不包含已删除 slug。
- [ ] 搜索旧 slug 不再出现在路由表、loadout、tier、approval policy 中。

## P1：skill 维护簇合并

### 当前问题

- [ ] `skill-framework-developer`、`skill-writer`、`skill-routing-repair`、`writing-skills`、`skill-scout` 都在争抢 skill 维护语义。
- [ ] 用户说“skill 系统精简/合并/删除/优化”时，应该只进 `skill-framework-developer`。
- [ ] 单个 skill 文件的实际编辑可以由 `.system/skill-creator` 承接，不需要 `skill-writer` 独立 owner。

### 目标结构

- [ ] 保留 `skill-framework-developer`：唯一框架治理 owner。
- [ ] 保留 `.system/skill-creator`：唯一具体 skill package 创建/更新执行入口。
- [ ] 保留 `.system/skill-installer`：远程/curated skill 安装入口，默认不参与本地治理。
- [ ] 删除或降级 `skill-writer`。
- [ ] 删除或降级 `writing-skills`。
- [ ] 删除或降级 `skill-routing-repair`。
- [ ] 删除或降级 `skill-scout`。

### 合并方式

- [ ] 将 `skill-writer` 的“单 skill wording / token budget / boundary”合入 `skill-framework-developer` 的一个 mode：`single-skill wording pass`。
- [ ] 将 `writing-skills` 的“批量模板统一”合入 `skill-framework-developer` 的一个 mode：`batch wording normalization`。
- [ ] 将 `skill-routing-repair` 的“post-task miss repair”合入 `skill-framework-developer` 的一个 mode：`miss repair`。
- [ ] 将 `skill-scout` 的“外部 skill 生态对标”合入 `skill-framework-developer` 的一个 mode：`external scout`。
- [ ] 将长说明移入 `skills/skill-framework-developer/references/`。
- [ ] 在 `skill-framework-developer/SKILL.md` 只保留模式选择表和最小流程。

### 删除顺序

- [ ] 先把四个 skill 的高价值规则摘入 `skill-framework-developer/references/skill-maintenance-modes.md`。
- [ ] 再从 routing runtime / index / layers 移除四个独立入口。
- [ ] 最后删除四个目录或将目录移到 archive。

### 验收

- [ ] 用户说“skill 不好用”“路由没触发”“写一个 skill”“批量规范 skill”“外部调研优化 skill”都路由到 `skill-framework-developer` 或 `.system/skill-creator`。
- [ ] `skills/SKILL_ROUTING_LAYERS.md` 的易混淆边界删除旧五分法。
- [ ] `skills/SKILL_ROUTING_INDEX.md` 只保留 `skill-framework-developer` 作为 skill 库/路由框架入口。
- [ ] `skills/SKILL_LOADOUTS.json` 的 `framework_loadout` 不再列出旧维护 skill。
- [ ] `skill-framework-developer` 顶层不超过约 `120` 行。

## P1：checklist 簇合并

### 当前问题

- [ ] `checklist-writting` 拼写错误形成永久债务。
- [ ] `checklist-writting` 和 `checklist-normalizer` 都是“计划到执行清单”的形状工作。
- [ ] `checklist-fixer` 是执行队列，语义不同，可以保留。

### 目标结构

- [ ] 新建或重命名为 `checklist-planner`。
- [ ] `checklist-planner` 覆盖从目标生成 checklist 与整理已有 checklist。
- [ ] `checklist-fixer` 保留为“按 checklist 执行”的 owner。
- [ ] 删除 `checklist-writting`。
- [ ] 删除或合并 `checklist-normalizer`。

### 合并内容

- [ ] 从 `checklist-writting` 保留：版本化文件输出规则、agent 数量建议、先 plan 后执行边界。
- [ ] 从 `checklist-normalizer` 保留：串行写在一点、并行拆开、验收/约束/停止条件、更新规则。
- [ ] 从两个 skill 删除重复的 subagent / local-supervisor 大段说明，只保留一句转交 `subagent-delegation`。
- [ ] 将详细 checklist 模板放入 `checklist-planner/references/checklist-template.md`。

### 路由规则

- [ ] 用户只有目标、还没有清单：`checklist-planner`。
- [ ] 用户已有混乱清单、要整理：`checklist-planner`。
- [ ] 用户说“按 checklist 执行”“先做 1-3”“从 P0 开始”：`checklist-fixer`。
- [ ] 用户只是直接实现 spec：`plan-to-code`。

### 验收

- [ ] 搜索 `checklist-writting` 只在迁移说明或历史记录中出现。
- [ ] `SKILL_ROUTING_RUNTIME.json` 不再有 `checklist-writting`。
- [ ] `SKILL_ROUTING_LAYERS.md` 的易混淆边界改成 `checklist-planner` vs `checklist-fixer`。
- [ ] checklist 相关入口从 `3` 个降到 `2` 个。

## P1：execution alias 收口

### 当前问题

- [ ] `autopilot` 和 `team` 自己声明 canonical owner 是 `execution-controller-coding`。
- [ ] 它们实际更像用户显式命令 alias，而不是独立 skill。
- [ ] `subagent-delegation` 已经承担 local / subagent / team 的运行时判断。

### 目标结构

- [ ] 保留 `execution-controller-coding` 作为复杂执行唯一主 owner。
- [ ] 保留 `subagent-delegation` 作为是否拆 sidecar / team 的 gate。
- [ ] 将 `autopilot` 降成 `execution-controller-coding` 的 alias mode。
- [ ] 将 `team` 降成 `subagent-delegation` 或 `execution-controller-coding` 的 alias mode。
- [ ] 删除独立 `autopilot/SKILL.md` 和 `team/SKILL.md`，或保留极短 stub 且不进入 runtime skills。

### 合并内容

- [ ] `autopilot` 的 Expansion -> Planning -> Execution -> QA -> Validation -> Cleanup 变成 `execution-controller-coding/references/autopilot-mode.md`。
- [ ] `team` 的 scoping -> delegation -> execution -> integration -> qa -> cleanup 变成 `subagent-delegation/references/team-mode.md`。
- [ ] `execution-controller-coding/SKILL.md` 增加显式入口：`$autopilot` / “一路执行到底”。
- [ ] `subagent-delegation/SKILL.md` 增加显式入口：`$team` / “多 agent 执行”。

### 验收

- [ ] 用户说 `$autopilot` 时不产生独立 owner 竞争。
- [ ] 用户说 `$team` 时先由 `subagent-delegation` 判断是否真的需要 team orchestration。
- [ ] `SKILL_TIERS.json` 不再把 `autopilot`、`team` 当 optional skill。
- [ ] `RUNTIME_REGISTRY.json` 仍可保留 alias 状态机，但不要求独立 skill 目录。

## P1：spreadsheet / xlsx 合并

### 当前问题

- [ ] `spreadsheets` 和 `xlsx` 都是 L3 artifact gate。
- [ ] 两者都 required，会扩大首轮 gate 判断面。
- [ ] `spreadsheets` 是通用入口，`xlsx` 是实现路径，应该下沉。

### 目标结构

- [ ] 保留 `primary-runtime/spreadsheets` 作为唯一 spreadsheet artifact gate。
- [ ] 删除或降级 `xlsx` 为 `spreadsheets/references/xlsx-rust-workflow.md`。
- [ ] 将 Rust OOXML、LibreOffice render、formula/style audit 路径移动到 references。
- [ ] 保留触发语 `xlsx`、`Excel`、`workbook structure audit`，但命中 `spreadsheets`。

### 验收

- [ ] artifact gate 中 spreadsheet 只有一个 required gate。
- [ ] 用户说 `.xlsx`、Excel、公式、格式、打印布局，命中 `spreadsheets`。
- [ ] `xlsx` 不再作为独立 slug 出现在 runtime。
- [ ] `spreadsheets` 顶层文档不超过约 `120` 行，细节在 references。

## P1：paper 簇降级

### 当前问题

- [ ] 已有 `paper-workbench` 作为 front door，但仍有太多 paper 同级 owner。
- [ ] `paper-reviewer`、`paper-reviser` 是真实二级 lane，可以保留。
- [ ] `paper-logic`、`paper-notation-audit`、`paper-length-tuner`、`paper-visuals` 多数是检查维度，不一定需要独立路由入口。

### 目标结构

- [ ] 保留 `paper-workbench`：唯一 manuscript-level front door。
- [ ] 保留 `paper-reviewer`：review-only lane。
- [ ] 保留 `paper-reviser`：known findings / reviewer comments 后的 edit lane。
- [ ] 保留 `paper-writing`：bounded prose drafting/polish lane。
- [ ] 降级 `paper-logic` 为 `paper-reviewer` / `paper-reviser` 的 `logic mode`。
- [ ] 降级 `paper-notation-audit` 为 `paper-reviewer` 的 `notation sweep` reference。
- [ ] 降级 `paper-length-tuner` 为 `paper-reviser` / `paper-writing` 的 `length budget mode`。
- [ ] 降级 `paper-visuals` 为 `paper-reviewer` / `paper-reviser` 的 `figure-table mode`。

### 合并内容

- [ ] 新建 `paper-workbench/references/paper-lanes.md`。
- [ ] 新建 `paper-reviewer/references/review-dimensions.md`。
- [ ] 新建 `paper-reviser/references/revision-modes.md`。
- [ ] 将 paper 专科 skill 的触发语保留到 front door 的 routing table。
- [ ] 将专科工作流压成 mode，不再作为 top-level skill。

### 路由规则

- [ ] 整篇论文、能不能投、投稿前把关：`paper-workbench`。
- [ ] 明确只审不改：`paper-reviewer`。
- [ ] 明确按 reviewer comments 改：`paper-reviser`。
- [ ] 明确只改表达且 claim 已定：`paper-writing`。
- [ ] 明确只做文献梳理：`literature-synthesis`。
- [ ] 代码生成科研图：`scientific-figure-plotting`。

### 验收

- [ ] paper 独立 slug 从约 `8` 个降到约 `4` 个。
- [ ] 用户仍可说“检查符号”“砍到 X 页”“只看图表”，但路由到 paper front door 或 reviewer/reviser mode。
- [ ] `SKILL_ROUTING_LAYERS.md` 不再列一长串 paper owner 竞争。
- [ ] `paper-workbench` 顶层只负责选择 lane，不复制各 lane 细节。

## P1：design 簇降级

### 当前问题

- [ ] `design-agent`、`frontend-design`、`design-md`、`design-output-auditor`、`design-prompt-enhancer`、`design-workflow-protocol` 入口过细。
- [ ] 多个 skill 都处理 DESIGN.md / prompt / screenshot / verdict 的链路。
- [ ] `visual-review` 已经是截图/渲染 evidence gate，不需要 design audit 再抢证据入口。

### 目标结构

- [ ] 保留 `design-agent`：named-product reference grounding gate。
- [ ] 保留 `frontend-design`：直接 UI redesign / visual direction owner。
- [ ] 保留 `visual-review`：截图/渲染证据 gate。
- [ ] 合并 `design-md`、`design-prompt-enhancer`、`design-output-auditor`、`design-workflow-protocol` 为 `design-workflow`，或全部降为 `frontend-design/references/`。
- [ ] 保留 `motion-design` 为显式高端动效实现 owner，但不进入 default surface。
- [ ] 保留 `infographic` 为产物类型明确的 HTML infographic owner。

### 合并内容

- [ ] `design-md` 变成 `design-workflow` 的 `capture DESIGN.md` mode。
- [ ] `design-prompt-enhancer` 变成 `design-workflow` 的 `prompt generation` mode。
- [ ] `design-output-auditor` 变成 `design-workflow` 的 `acceptance verdict` mode。
- [ ] `design-workflow-protocol` 变成 `design-workflow` 的主文档或 reference。
- [ ] `frontend-design` 只链接 `design-workflow`，不复制工作流协议。

### 路由规则

- [ ] 用户说“像 Linear / Stripe / Apple”：`design-agent`。
- [ ] 用户说“直接改 UI / 做高级感”：`frontend-design`。
- [ ] 用户说“先沉淀 DESIGN.md / 设计 prompt / 设计验收 / 设计闭环”：`design-workflow`。
- [ ] 用户给截图要看问题：`visual-review`。
- [ ] 用户明确要 Framer Motion / GSAP / micro-interactions：`motion-design`。

### 验收

- [ ] design 独立入口从约 `6` 个降到约 `3-4` 个。
- [ ] `SKILL_ROUTING_LAYERS.md` 的 design 易混淆边界减少到一张短表。
- [ ] default loadout 不包含 design 专科。
- [ ] `frontend-design` 不再承担 prompt enhancer / audit protocol 的细节。

## P2：research 簇收口

### 当前问题

- [ ] `research-workbench` 已经是非 manuscript research front door。
- [ ] `information-retrieval`、`github-investigator`、`skill-scout`、`literature-synthesis`、`brainstorm-research`、`autoresearch`、`ai-research`、`research-engineer` 有相邻触发。
- [ ] `github-investigator` 健康信号偏弱，有 reroute。

### 目标结构

- [ ] 保留 `research-workbench`：科研项目 front door。
- [ ] 保留 `literature-synthesis`：学术文献检索/综合 owner。
- [ ] 保留 `information-retrieval`：通用 pre-action research owner。
- [ ] 将 `github-investigator` 降为 `information-retrieval` 的 GitHub deep-dive mode。
- [ ] 将 `skill-scout` 并入 `skill-framework-developer`。
- [ ] 保留 `brainstorm-research` 仅显式 opt-in，不做 preferred session_start。
- [ ] 保留 `autoresearch` 仅显式 opt-in，不做 preferred session_start。
- [ ] 保留 `ai-research`、`research-engineer` 为窄专家，但从 default surface 排除。

### 路由规则

- [ ] 一般“调研/对比/查最新”：`information-retrieval`。
- [ ] GitHub repo/issue/PR/time line 深挖：`information-retrieval` 的 GitHub mode。
- [ ] 科研项目下一步：`research-workbench`。
- [ ] 找论文、related work、novelty：`literature-synthesis`。
- [ ] AI/ML training/eval/inference 实现：`ai-research`。
- [ ] 算法/方法是否站得住：`research-engineer`。
- [ ] 多假设自主实验循环：显式 opt-in `autoresearch`。

### 验收

- [ ] `github-investigator` 不再作为独立 runtime slug。
- [ ] `research-workbench` 不吞 manuscript paper work；仍转 `paper-workbench`。
- [ ] `information-retrieval` 顶层增加 GitHub mode，但不膨胀成长文档。
- [ ] research default loadout 不含 `brainstorm-research` 和 `autoresearch`。

## P2：security 簇收口

### 当前问题

- [ ] `security-audit`、`security-threat-model`、`webhook-security`、`auth-implementation` 有明显相邻区域。
- [ ] `webhook-security` 是 provider-specific implementation slice，不一定需要独立 skill。

### 目标结构

- [ ] 保留 `security-audit`：实现级安全审计 overlay。
- [ ] 保留 `security-threat-model`：系统级威胁建模 owner。
- [ ] 保留 `auth-implementation`：登录/鉴权/授权实现 owner。
- [ ] 降级 `webhook-security` 为 `security-audit/references/webhook-security.md` 或 `auth-implementation/references/webhook-callbacks.md`。

### 路由规则

- [ ] 实现登录、权限、session/JWT/OAuth：`auth-implementation`。
- [ ] 查漏洞、注入、SSRF、secret、鉴权缺陷：`security-audit`。
- [ ] 资产、边界、攻击路径、abuse case：`security-threat-model`。
- [ ] Stripe/GitHub/Slack webhook：按任务性质进入 `auth-implementation` 或 `security-audit` 的 webhook reference。

### 验收

- [ ] `webhook-security` 不再作为独立 slug。
- [ ] `security-audit` 的触发语包含 webhook review，但不吞 generic auth implementation。
- [ ] `auth-implementation` 的 Do not use 保持安全审计边界。

## P2：frontend / web 簇降噪

### 当前问题

- [ ] `react`、`nextjs`、`vercel-react-best-practices`、`frontend-code-quality`、`frontend-design`、`frontend-debugging`、`css-pro`、`tailwind-pro`、`web-platform-basics` 有多层覆盖。
- [ ] overlay 过多会导致主线程判断成本上升。

### 目标结构

- [ ] 保留 `frontend-debugging`：前端 runtime bug owner。
- [ ] 保留 `frontend-design`：视觉改版 owner。
- [ ] 保留 `react`、`nextjs`、`vue`、`svelte`：框架语义 owner。
- [ ] 将 `vercel-react-best-practices` 降为 `nextjs/references/vercel-best-practices.md`。
- [ ] 将 `frontend-code-quality` 合入 `coding-standards` 或作为 `react/nextjs` reference。
- [ ] `css-pro` 和 `tailwind-pro` 暂保留，但不进入 default surface。
- [ ] `web-platform-basics` 暂保留为底层 browser/API owner，但不进入 default surface。

### 验收

- [ ] overlays 中不再同时出现 `frontend-code-quality` 和 `vercel-react-best-practices`。
- [ ] React/Next 请求优先窄 owner，不被 generic frontend overlay 抢走。
- [ ] default loadout 不包含 frontend 专科 overlay。

## P2：artifact / presentation 簇整理

### 当前问题

- [ ] `slides` 是 gate，`ppt-pptx`、`ppt-beamer`、`source-slide-formats` 是后续 source owner，结构合理。
- [ ] 旧 slide source skill 已删除后，需要确保所有触发语迁移完整。

### 目标结构

- [ ] 保留 `slides`：generic PPT / presentation artifact gate。
- [ ] 保留 `ppt-pptx`：source-first native PPTX owner。
- [ ] 保留 `ppt-beamer`：LaTeX Beamer owner。
- [ ] 保留 `source-slide-formats`：Markdown / Slidev / Marp / HTML owner。
- [ ] 不再恢复 `ppt-html-export`、`ppt-markdown`、`slides-source-first`。

### 验收

- [ ] Generic “做个 PPT” 先命中 `slides`。
- [ ] 明确 Markdown/Slidev/Marp/HTML 命中 `source-slide-formats`。
- [ ] 明确 Beamer 命中 `ppt-beamer`。
- [ ] 明确 `deck.plan.json` / Rust PPTX 命中 `ppt-pptx`。
- [ ] 旧 slide slug 不出现在 routing runtime。

## P2：low-frequency specialist 降到 explicit opt-in

### 目标

- [ ] 不急着删除低频专家，但从默认面拿掉。
- [ ] 只有用户明确说出领域词，才触发这些 skill。

### 候选

- [ ] `algo-trading`。
- [ ] `financial-data-fetching`。
- [ ] `mac-memory-management`。
- [ ] `youtube-summarizer`。
- [ ] `email-template`。
- [ ] `copywriting`。
- [ ] `assignment-compliance`。
- [ ] `tao-ci`。
- [ ] `sustech-mailer`。
- [ ] `chrome-extension-dev`。
- [ ] `jupyter-notebook`。
- [ ] `latex-compile-acceleration`。
- [ ] `scientific-figure-plotting`。
- [ ] `infographic`。
- [ ] `accessibility-auditor`。

### 验收

- [ ] 这些 skill 不出现在 default loadout。
- [ ] 这些 skill 的 `session_start` 不为 `preferred`，除非有极强理由。
- [ ] 触发语保留具体领域词，不使用 generic “research”“review”“optimize”。

## P2：overlay 体系减法

### 当前问题

- [ ] overlay 太多会制造隐性组合爆炸。
- [ ] `iterative-optimizer` 使用多但 reroute 多，不适合继续独立 overlay。

### 目标结构

- [ ] 保留 `execution-audit`：强验收 overlay。
- [ ] 保留 `code-review`：代码审查 findings overlay。
- [ ] 保留 `security-audit`：安全审计 overlay。
- [ ] 保留 `coding-standards`：跨栈代码规范 overlay。
- [ ] 保留 `anti-laziness`，但只作为行为约束，不抢 owner。
- [ ] 删除或合并 `iterative-optimizer`。
- [ ] 删除或合并 `frontend-code-quality`。
- [ ] 删除或合并 `vercel-react-best-practices`。
- [ ] 保留 `i18n-l10n` 为显式 overlay，不默认启用。
- [ ] 保留 `tdd-workflow` 为显式 overlay，不默认启用。
- [ ] 保留 `error-handling-patterns` 为显式 overlay，不默认启用。

### 合并方式

- [ ] `iterative-optimizer` 的多轮收敛规则合入 `execution-audit/references/iteration-loop.md`。
- [ ] `frontend-code-quality` 的文件长度/early return/RORO 规则合入 `coding-standards` 或 frontend references。
- [ ] `vercel-react-best-practices` 合入 `nextjs` references。

### 验收

- [ ] overlay 列表不超过 `8` 个。
- [ ] default overlay 不超过 `1-2` 个。
- [ ] 用户要求“优化 N 轮 / review-fix-rescore”时仍有路径，但不独立抢 owner。

## P3：语言 / 平台专家保守保留

### 目标

- [ ] 第一轮不大删语言专家，因为它们是低频显式 opt-in，路由噪声低。
- [ ] 只做 default surface 清理和触发语去泛化。

### 保留

- [ ] `python-pro`。
- [ ] `typescript-pro`。
- [ ] `javascript-pro`。
- [ ] `rust-pro`。
- [ ] `go-pro`。
- [ ] `sql-pro`。
- [ ] `node-backend`。
- [ ] `nextjs`。
- [ ] `react`。
- [ ] `vue`。
- [ ] `svelte`。
- [ ] `docker`。
- [ ] `linux-server-ops`。
- [ ] `mcp-builder`。
- [ ] `cloudflare-deploy`。
- [ ] `github-actions-authoring`。

### 清理规则

- [ ] 删除泛化触发词，如单独的 `research`、`review`、`optimize`。
- [ ] 保留具体技术词，如 `FastAPI`、`Tokio`、`App Router`、`Composition API`。
- [ ] 不让语言专家抢 build/debug/source gate。

### 验收

- [ ] 语言专家不进入 default loadout，除非用户经常明确使用。
- [ ] `build-tooling` vs language expert 边界仍清楚。
- [ ] `systematic-debugging` 仍先于语言专家处理未知根因。

## P3：文档和写作簇整理

### 当前问题

- [ ] `documentation-engineering`、`copywriting`、`humanizer`、`paper-writing`、`prompt-engineer`、`email-template` 都与写作有关，但对象不同。

### 目标结构

- [ ] 保留 `documentation-engineering`：项目文档。
- [ ] 保留 `paper-writing`：学术论文 prose。
- [ ] 保留 `humanizer`：自然化润色。
- [ ] 保留 `copywriting`：商业转化文案，但 explicit opt-in。
- [ ] 保留 `prompt-engineer`：非设计 prompt。
- [ ] 保留 `email-template`：HTML email artifact。
- [ ] 不让任何写作 skill 触发 skill authoring；skill authoring 只归 `skill-framework-developer` / `.system/skill-creator`。

### 验收

- [ ] `writing-skills` 删除或合并后，不再和普通 writing 混淆。
- [ ] “写 README” 命中 `documentation-engineering`。
- [ ] “润色这段话” 命中 `humanizer`。
- [ ] “论文润色” 命中 `paper-writing`。
- [ ] “广告/落地页文案” 命中 `copywriting`。

## P3：API / backend 簇整理

### 目标

- [ ] 保留各自清晰边界，不做大合并。
- [ ] 清理泛化触发词，减少和 debugging/build-tooling 抢入口。

### 保留

- [ ] `api-design`：API contract / OpenAPI / endpoint design。
- [ ] `api-integration-debugging`：服务边界请求失败。
- [ ] `api-load-tester`：负载测试。
- [ ] `backend-runtime-debugging`：后端运行时异常。
- [ ] `datastore-cache-queue`：store/cache/queue/worker correctness。
- [ ] `observability`：logs/metrics/traces/dashboard/alerts。
- [ ] `env-config-management`：env/config/secrets/feature flags。
- [ ] `error-handling-patterns`：显式 overlay。

### 验收

- [ ] 未知错误仍先进 `systematic-debugging`。
- [ ] API 设计和 API 调试不互抢。
- [ ] load test 不被 performance expert 抢走。

## 路由文件更新 Checklist

- [ ] 更新 `skills/SKILL_ROUTING_INDEX.md`。
- [ ] 更新 `skills/SKILL_ROUTING_LAYERS.md`。
- [ ] 更新 `skills/SKILL_ROUTING_REGISTRY.md`。
- [ ] 更新 `skills/SKILL_ROUTING_RUNTIME.json`。
- [ ] 更新 `skills/SKILL_MANIFEST.json`。
- [ ] 更新 `skills/SKILL_SOURCE_MANIFEST.json`。
- [ ] 更新 `skills/SKILL_HEALTH_MANIFEST.json`。
- [ ] 更新 `skills/SKILL_TIERS.json`。
- [ ] 更新 `skills/SKILL_LOADOUTS.json`。
- [ ] 更新 `skills/SKILL_APPROVAL_POLICY.json`。
- [ ] 更新 `skills/SKILL_SHADOW_MAP.json`。
- [ ] 更新 `configs/framework/RUNTIME_REGISTRY.json`。
- [ ] 更新 `configs/framework/FRAMEWORK_SURFACE_POLICY.json`。
- [ ] 更新相关 tests fixtures。

## 生成/同步命令 Checklist

- [ ] 运行 skill compiler apply：

```bash
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json \
  --apply
```

- [ ] 运行 router-rs runtime snapshot：

```bash
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml \
  --framework-runtime-snapshot-json \
  --repo-root /Users/joe/Documents/skill
```

- [ ] 运行 routing eval tests。
- [ ] 运行 policy contract tests。
- [ ] 运行 host integration tests。
- [ ] 运行 rust CLI tools tests。
- [ ] 如果生成物漂移，先查生成源，不手改 generated surface。

## 测试用例更新 Checklist

### skill 维护路由

- [ ] “新一轮核查，用减法原理，看看 skill 系统怎么精简” -> `skill-framework-developer`。
- [ ] “这次为什么没触发，顺手修一下 skill” -> `skill-framework-developer` 的 miss repair mode。
- [ ] “帮我写一个新的 Codex skill” -> `.system/skill-creator` 或 `skill-framework-developer` 决策后交给 creator。
- [ ] “批量规范所有 SKILL.md” -> `skill-framework-developer` 的 batch mode。
- [ ] “调研外部 skill 生态” -> `skill-framework-developer` 的 external scout mode。

### checklist 路由

- [ ] “先给我一个 checklist” -> `checklist-planner`。
- [ ] “把这个 checklist 串行写一点，并行拆开” -> `checklist-planner`。
- [ ] “按 checklist 执行 1-3” -> `checklist-fixer`。
- [ ] “好，就按这个做” 且上一轮是 checklist -> `checklist-fixer`。

### execution alias

- [ ] “$autopilot 一路执行到底” -> `execution-controller-coding`。
- [ ] “$team 多 agent 执行” -> `subagent-delegation` gate 后进入 controller。
- [ ] “需要并行 sidecar” -> `subagent-delegation`。
- [ ] “普通单文件修复” -> 不进 controller。

### artifact

- [ ] “帮我改这个 xlsx 公式和格式” -> `spreadsheets`。
- [ ] “做个 PPT” -> `slides`。
- [ ] “用 Markdown 做 slides” -> `source-slide-formats`。
- [ ] “做 Beamer slides” -> `ppt-beamer`。
- [ ] “检查 PDF 排版” -> `pdf`。
- [ ] “改这个 docx 表格版式” -> `doc`。

### paper

- [ ] “帮我看这篇 paper 能不能投” -> `paper-workbench`。
- [ ] “只做严审，不改稿” -> `paper-reviewer`。
- [ ] “按 reviewer comments 改” -> `paper-reviser`。
- [ ] “只润色 abstract，不改 claim” -> `paper-writing`。
- [ ] “检查符号是否统一” -> paper notation mode，不是独立 slug。
- [ ] “砍到 8 页” -> paper length mode，不是独立 slug。

### design

- [ ] “像 Linear 一样，先找参考源” -> `design-agent`。
- [ ] “直接把 UI 做高级感” -> `frontend-design`。
- [ ] “先沉淀 DESIGN.md” -> `design-workflow`。
- [ ] “优化 UI 生成提示词” -> `design-workflow`。
- [ ] “按 DESIGN.md 做设计验收” -> `design-workflow` 或 `visual-review` 先取证。
- [ ] “看这张截图哪里不对” -> `visual-review`。

### research

- [ ] “调研一下最新做法” -> `information-retrieval`。
- [ ] “拆解这个 GitHub 仓库 issue/PR 演进” -> `information-retrieval` GitHub mode。
- [ ] “这个科研方向下一步怎么做” -> `research-workbench`。
- [ ] “找 20 篇相关论文” -> `literature-synthesis`。
- [ ] “这个算法复杂度站得住吗” -> `research-engineer`。
- [ ] “跑多假设自主实验循环” -> explicit `autoresearch`。

## 删除执行安全 Checklist

- [ ] 删除前先 `rg` 旧 slug 的所有引用。
- [ ] 删除前确认该 slug 没有被 runtime registry 当作 alias entrypoint。
- [ ] 删除前确认该目录没有唯一脚本或资产；如果有，迁移到保留 owner 的 `references/`、`scripts/`、`assets/`。
- [ ] 删除前确认 tests fixture 已更新。
- [ ] 删除后运行生成/同步。
- [ ] 删除后运行最小 routing eval。
- [ ] 删除后检查 `git status --short`，确认没有意外删除用户无关文件。

## 合并执行安全 Checklist

- [ ] 先定义保留 owner。
- [ ] 再列出被合并 skill 的高价值规则。
- [ ] 只迁移规则，不迁移整篇冗余 prose。
- [ ] 顶层 `SKILL.md` 保持短，细节放 references。
- [ ] 将被合并 skill 的触发语迁入保留 owner。
- [ ] 将被合并 skill 的 Do not use 边界转成保留 owner 的 mode selection。
- [ ] 更新 routing docs。
- [ ] 更新 manifest / health / tiers / loadouts。
- [ ] 删除旧目录。
- [ ] 跑验证。

## 降级执行安全 Checklist

- [ ] 判断该 skill 是否只是检查维度、流程 mode、工具路径或 reference。
- [ ] 如果是，移到上级 owner 的 `references/`。
- [ ] 如果用户仍会自然说出该触发语，把触发语留在上级 owner。
- [ ] 如果该 skill 有工具命令，把命令保留在 reference quick path。
- [ ] 删除其独立 routing metadata。
- [ ] 添加测试，确保旧触发语仍命中上级 owner。

## 默认面收缩 Checklist

- [ ] `default_surface_loadout.owners` 控制在 `5-7` 个。
- [ ] `default_surface_loadout.overlays` 控制在 `1-2` 个。
- [ ] 从 default 排除 research / audit / design / paper / low-frequency 专科。
- [ ] default owners 建议只保留：
- [ ] `plan-to-code`。
- [ ] `gitx`。
- [ ] `shell-cli`。
- [ ] `execution-controller-coding` 仅在明确复杂执行时，不常驻普通任务。
- [ ] `python-pro` / `typescript-pro` 视使用习惯保留或移出 default。
- [ ] default overlays 建议只保留 `anti-laziness`，或连它也改成行为规则而非 overlay。

## 成功标准

- [ ] skill 总数下降到约 `90-100`。
- [ ] required session_start skill 数量下降。
- [ ] default activation 下降到约 `12-16`。
- [ ] L0 skill 数量明显下降。
- [ ] skill 维护入口从 `5+` 个降到 `2` 个。
- [ ] checklist 入口从 `3` 个降到 `2` 个。
- [ ] spreadsheet artifact gate 从 `2` 个降到 `1` 个。
- [ ] paper 独立入口从约 `8` 个降到约 `4` 个。
- [ ] design 独立入口从约 `6` 个降到约 `3-4` 个。
- [ ] overlay 数量不超过 `8`。
- [ ] `iterative-optimizer` 不再独立抢路由。
- [ ] `github-investigator` 不再独立抢路由。
- [ ] 所有旧 slug 无 runtime 引用。
- [ ] routing eval 无新增失败。
- [ ] sync / compiler 无生成物漂移。

## 建议执行顺序

- [ ] 第 1 轮：清理历史兼容残留和旧重复 skill。
- [ ] 第 2 轮：合并 skill 维护簇。
- [ ] 第 3 轮：合并 checklist 簇。
- [ ] 第 4 轮：收口 `autopilot` / `team` alias。
- [ ] 第 5 轮：合并 `spreadsheets` / `xlsx`。
- [ ] 第 6 轮：降级 paper 专科。
- [ ] 第 7 轮：降级 design 专科。
- [ ] 第 8 轮：research/security/frontend overlay 降噪。
- [ ] 第 9 轮：收缩 default loadout。
- [ ] 第 10 轮：跑全量 routing eval / contract tests / sync audit。

## 每轮完成模板

- [ ] 本轮处理簇：
- [ ] 删除 slug：
- [ ] 合并 slug：
- [ ] 降级 slug：
- [ ] 保留 owner：
- [ ] 迁移 references：
- [ ] 更新 routing 文件：
- [ ] 更新 tests：
- [ ] 验证命令：
- [ ] 剩余风险：
- [ ] 下一轮入口：
