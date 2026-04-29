# Runtime 系统修复执行清单

## 目标

把当前 runtime 系统从“多处漂移、测试失真、健康状态虚高”的状态，收敛为一个可验证、可维护、跨 host 行为一致的运行时系统。

本清单按优先级组织。建议先完成 P0，再进入 P1 和 P2。每一项都包含执行动作、验收标准和涉及区域。

## P0：修复运行时加载 contract

### 1. 修复 `spreadsheets` slug/path 漂移

- [ ] 确认当前设计选择：是否允许嵌套技能作为正式 runtime 技能。
- [ ] 如果不允许嵌套技能：将 `skills/primary-runtime/spreadsheets/SKILL.md` 移动或重构为 `skills/spreadsheets/SKILL.md`。
- [ ] 如果允许嵌套技能：修改 manifest/runtime schema，加入实际技能路径字段，例如 `skill_path` 或 `source_path`。
- [ ] 同步更新 root `CLAUDE.md` 和 `.claude/CLAUDE.md` 中的加载规则，不再假设所有技能都在 `skills/<name>/SKILL.md`。
- [ ] 更新 router/host 加载逻辑，使其按 manifest/runtime 中声明的路径读取技能。
- [ ] 增加测试：runtime 和 manifest 中每个 slug 都必须能解析到一个实际存在的 `SKILL.md`。

验收标准：`spreadsheets` 被 runtime 命中后，所有 host 都能按同一规则找到并加载正确的 `SKILL.md`。

涉及区域：`skills/SKILL_ROUTING_RUNTIME.json`、`skills/SKILL_MANIFEST.json`、`skills/primary-runtime/spreadsheets/SKILL.md`、`scripts/skill-compiler-rs/src/main.rs`、host entrypoint policy。

### 2. 修复 compiler 的路径信息丢失问题

- [ ] 审查 `discover_skill_dirs()` 的 slug 生成逻辑。
- [ ] 保留 slug 的同时，记录相对 `skills/` 根目录的真实路径。
- [ ] 在 `SKILL_MANIFEST.json` 中加入真实路径字段。
- [ ] 在 `SKILL_ROUTING_RUNTIME.json` 中加入 host 加载所需的最小路径字段。
- [ ] 增加 schema version 或 migration 说明，避免旧 consumer 误读新字段。
- [ ] 增加 compiler 测试：嵌套技能不会丢失路径信息。

验收标准：任意深度发现的技能都不会只剩 basename slug；runtime consumer 可以不猜路径。

涉及区域：`scripts/skill-compiler-rs/src/main.rs`、manifest/runtime schema、相关测试。

## P0：修复测试和 CI 的可信度

### 3. 更新陈旧的 `policy_contracts.rs`

- [ ] 删除或改写“根目录 `CLAUDE.md` 不应存在”的旧断言。
- [ ] 将 supported hosts 期望更新为当前设计：`codex-cli`、`codex-app`、`claude-code-cli`、`claude-desktop`。
- [ ] 更新与 `.claude/CLAUDE.md`、`CLAUDE.md`、`.codex/host_entrypoints_sync_manifest.json` 相关的断言。
- [ ] 重新确认 hot runtime 必须包含哪些技能。
- [ ] 如果 `subagent-delegation`、`skill-creator`、`skill-installer` 已不属于 hot runtime，删除对应断言。
- [ ] 如果这些技能应属于 hot runtime，修复 compiler selection 逻辑，而不是只改测试。

验收标准：测试表达当前真实设计，而不是旧 Codex-only 设计。

涉及区域：`tests/policy_contracts.rs`、`configs/framework/RUNTIME_REGISTRY.json`、`.codex/host_entrypoints_sync_manifest.json`、host entrypoints。

### 4. 把 router-rs 核心测试加入 CI

- [ ] 在 `.github/workflows/skill-ci.yml` 中加入 router-rs crate 测试。
- [ ] 至少运行：`cargo test --manifest-path scripts/router-rs/Cargo.toml`。
- [ ] 如果测试过慢，拆分为 route eval、stdio protocol、host hooks、storage/trace 等 jobs。
- [ ] 确保 routing eval cases 在 CI 中稳定执行。
- [ ] 确保 CI 失败时能明确指出是 route regression、schema drift 还是 generated artifact drift。

验收标准：router-rs 的核心路由逻辑变更无法绕过 CI。

涉及区域：`.github/workflows/skill-ci.yml`、`scripts/router-rs/Cargo.toml`、`scripts/router-rs/src/route.rs`、routing eval tests。

### 5. 清理 routing eval fixtures 中的失效 skill slug

- [ ] 扫描 `tests/routing_eval_cases.json` 中所有 `expected_owner`、`expected_overlay`、`forbidden_owners`。
- [ ] 与当前 `skills/SKILL_MANIFEST.json` 做一致性校验。
- [ ] 对不存在的 slug 做分类：恢复、替换、删除、或标记 retired。
- [ ] 清理明显陈旧的 owner，例如 `execution-controller-coding`、`checklist-planner`、`checklist-fixer`、`skill-creator`、`skill-installer`。
- [ ] 保留仍有业务价值的中文 runtime audit case，并确保其 owner 为 `skill-framework-developer`。
- [ ] 增加测试：eval fixtures 中引用的 slug 必须存在于 manifest，除非显式标记 retired。

验收标准：routing eval 只验证当前真实技能系统，不再引用幽灵技能。

涉及区域：`tests/routing_eval_cases.json`、`skills/SKILL_MANIFEST.json`、router-rs route tests。

## P1：让健康检查反映真实 runtime 状态

### 6. 改造 `SKILL_HEALTH_MANIFEST.json` 的生成逻辑

- [ ] 不再默认所有技能 `100.0` 健康。
- [ ] 增加 slug/path 可加载性检查。
- [ ] 增加 frontmatter `name` 与 manifest slug 一致性检查。
- [ ] 增加 runtime hot set 与 full manifest 一致性检查。
- [ ] 增加 required gate 是否可加载检查。
- [ ] 增加 routing metadata 合法枚举检查：`routing_layer`、`routing_owner`、`routing_gate`、`routing_priority`。
- [ ] 增加 trigger hints 基本质量检查，避免空触发面进入 hot runtime。
- [ ] 将检查失败映射为明确 health degradation，而不是仍然 Healthy。

验收标准：类似 `spreadsheets` 这种路径漂移会直接导致 health manifest 报错或降级。

涉及区域：`skills/SKILL_HEALTH_MANIFEST.json`、`scripts/skill-compiler-rs/src/main.rs`、health manifest tests。

### 7. 增加 compiler 级 contract validator

- [ ] 在 skill compiler 中加入 `validate_runtime_contract()` 或等价步骤。
- [ ] 校验 manifest 中每个 skill 都有实际 `SKILL.md`。
- [ ] 校验 runtime hot skill 是 manifest skill 的子集。
- [ ] 校验 runtime skill 所需加载字段完整。
- [ ] 校验 generated host policy 与 runtime schema 一致。
- [ ] 在 `--apply` 前后都执行校验，避免写出无效产物。

验收标准：compiler 不能生成自相矛盾的 runtime/manifest。

涉及区域：`scripts/skill-compiler-rs/src/main.rs`、generated artifacts、CI。

## P1：减少 router 中的硬编码漂移

### 8. 盘点 `route.rs` 中硬编码 skill slug

- [ ] 从 `scripts/router-rs/src/route.rs` 提取所有字符串形式的 skill slug。
- [ ] 与 `skills/SKILL_MANIFEST.json` 做差异对比。
- [ ] 为确实需要保留但不在 manifest 中的 slug 建立 allowlist，并写明原因。
- [ ] 删除已 retired 的 slug 规则。
- [ ] 将能声明化的规则迁移到 manifest/registry metadata。

验收标准：router 源码不再静默引用大量不存在的技能。

涉及区域：`scripts/router-rs/src/route.rs`、`skills/SKILL_MANIFEST.json`、routing tests。

### 9. 将特殊路由规则迁移为声明式 metadata

- [ ] 设计 metadata 字段表达 artifact gate、overlay-only、negative triggers、fallback role、routing contexts。
- [ ] 从 `route.rs` 中迁移与具体技能绑定的 boost/suppress 逻辑。
- [ ] 保留通用 scoring 算法，减少 slug-specific 分支。
- [ ] 更新 compiler，使 metadata 从 `SKILL.md` frontmatter 或 source manifest 进入 manifest/runtime。
- [ ] 增加 regression tests，确保迁移前后关键 case 行为一致。

验收标准：新增、删除或重命名技能主要通过 manifest/registry 完成，不需要到 router 源码里改硬编码。

涉及区域：`scripts/router-rs/src/route.rs`、`scripts/skill-compiler-rs/src/main.rs`、skill frontmatter schema。

### 10. 修复 fallback owner 过度偏向 `plan-to-code`

- [ ] 审查 `fallback_owner()` 当前逻辑。
- [ ] 增加任务 intent 分类：讨论、规划、审计、执行、验证。
- [ ] 对 audit/review/diagnose/problem-finding 类请求避免 fallback 到 `plan-to-code`。
- [ ] 为中文请求增加覆盖：`核查`、`审查`、`有什么问题`、`哪里错了`、`诊断`。
- [ ] 增加 eval：runtime audit 请求必须命中 `skill-framework-developer`，且 forbidden owner 包含 `plan-to-code`。

验收标准：审计型任务在 scoring 不确定时不会默认进入代码实现 owner。

涉及区域：`scripts/router-rs/src/route.rs`、`tests/routing_eval_cases.json`、router-rs tests。

## P1：统一 generated host surface

### 11. 把 host entrypoint 文本从 Rust hardcode 迁移到模板

- [ ] 新建或确认 host entrypoint template 目录。
- [ ] 将 root `CLAUDE.md` 内容迁移为模板源。
- [ ] 将 `.claude/CLAUDE.md` 内容迁移为模板源。
- [ ] 将 `AGENTS.md` 也纳入同一模板机制。
- [ ] 修改 `codex_hooks.rs`，从模板读取并渲染，而不是硬编码长 policy 字符串。
- [ ] 增加测试：生成文件与模板渲染结果一致。

验收标准：修改 host policy 不需要编辑 Rust 源码字符串。

涉及区域：`scripts/router-rs/src/codex_hooks.rs`、`CLAUDE.md`、`.claude/CLAUDE.md`、`AGENTS.md`、configs/templates。

### 12. 建立统一 generated artifact manifest

- [ ] 定义一个机器可读 manifest，列出所有 generated/protected 文件。
- [ ] 纳入 `AGENTS.md`、`CLAUDE.md`、`.claude/CLAUDE.md`、`.codex/host_entrypoints_sync_manifest.json`、`.claude/settings.json`。
- [ ] 纳入所有 `skills/SKILL_*.json` 和 `skills/SKILL_ROUTING_*.md`。
- [ ] 修改 CI drift check，从该 manifest 读取文件列表，而不是在 workflow 里手写。
- [ ] 修改 hooks/protection 逻辑，也引用同一 manifest。
- [ ] 更新文档，明确哪些文件可手写、哪些必须 regenerate。

验收标准：generated/protected 文件边界只有一个来源。

涉及区域：`.github/workflows/skill-ci.yml`、`scripts/router-rs/src/codex_hooks.rs`、compiler output、host sync manifest。

## P2：提升 runtime 可观测性和可维护性

### 13. 生成 hot runtime selection explanation

- [ ] 新增产物，例如 `skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`。
- [ ] 记录每个 hot skill 入选原因。
- [ ] 记录 preferred/required 但未入选技能的排除原因。
- [ ] 记录 hot set 与 full manifest 的关系。
- [ ] 在 CI 中检查 explain 文件与 runtime 文件同步。

验收标准：可以解释为什么当前 hot runtime 是 17 个技能，而不是凭经验猜测。

涉及区域：`scripts/skill-compiler-rs/src/main.rs`、`skills/SKILL_ROUTING_RUNTIME.json`、new explain artifact。

### 14. 改善 runtime schema 的演进安全

- [ ] 评估数组 rows schema 是否继续适合 host runtime。
- [ ] 至少为加载必需字段增加明确字段名或严格 schema 校验。
- [ ] 加入 `schema_version` 和 consumer compatibility 说明。
- [ ] 如果继续使用数组 schema，为字段顺序增加测试锁定。
- [ ] 对新增字段如 `skill_path`、`selection_reason`、`aliases` 设计兼容策略。

验收标准：runtime schema 扩展不会靠数组位置猜测，host consumer 不会静默误读。

涉及区域：`skills/SKILL_ROUTING_RUNTIME.json`、compiler、router loader、host policy。

### 15. 加强中文路由测试

- [ ] 建立中文 first-turn intent 测试集。
- [ ] 覆盖：核查、审查、修复、实现、规划、总结、生成文档、验证、同步、清理、迁移。
- [ ] 确保中文 audit/review 请求不会落入 `plan-to-code` fallback。
- [ ] 确保中文 artifact 请求正确进入 doc/pdf/slides/spreadsheets gates。
- [ ] 将中文 eval 纳入 router-rs CI。

验收标准：中文用户请求不是靠偶然 trigger 命中，而是有系统化回归保护。

涉及区域：`tests/routing_eval_cases.json`、`scripts/router-rs/src/route.rs`、router-rs tests。

## P2：修复本地 runtime 可执行性问题

### 16. 改造 `run_router_rs.sh` 的平台检查

- [ ] 在选择已有 binary 后检查 OS/arch 是否匹配当前平台。
- [ ] 如果 binary 是不兼容格式，例如 macOS arm64 binary 在 Linux 环境中，跳过或报明确错误。
- [ ] 如果 cargo 不存在，输出清晰提示，而不是失败得不透明。
- [ ] 避免依赖残留 `target/` 目录中的旧 binary。
- [ ] CI 中使用 clean build 验证 launcher 行为。

验收标准：launcher 不会把“存在且可执行”的错误平台 binary 当作可用 runtime。

涉及区域：`scripts/router-rs/run_router_rs.sh`、CI、local hook setup。

## P3：清理系统边界和历史遗留

### 17. 明确 `.system` 技能的生命周期状态

- [ ] 盘点 `.system/plugin-creator`、`.system/skill-creator`、`.system/skill-installer`。
- [ ] 判断它们是 internal-only、retired，还是应该重新进入 manifest。
- [ ] 如果 internal-only，确保 routing eval 不再把它们作为 expected owner。
- [ ] 如果仍可路由，加入 manifest 并标记 visibility。
- [ ] 删除 route.rs 中与实际状态不一致的历史规则。

验收标准：`.system` 技能不会同时处于“测试期望存在、manifest 不存在、源码仍硬编码”的灰区。

涉及区域：`.system` skills、`skills/SKILL_MANIFEST.json`、`tests/routing_eval_cases.json`、`scripts/router-rs/src/route.rs`。

### 18. 修正 CI drift check 文件列表

- [ ] 审查 `.github/workflows/skill-ci.yml` 中 drift check 的文件列表。
- [ ] 移除已废弃或不再 relevant 的条目。
- [ ] 加入 Claude 相关 generated files。
- [ ] 改为从统一 generated artifact manifest 读取列表。
- [ ] 确保 drift check 覆盖所有 host surfaces。

验收标准：任何 generated host entrypoint 或 runtime artifact 漂移都会被 CI 捕获。

涉及区域：`.github/workflows/skill-ci.yml`、generated artifact manifest、host sync logic。

## 推荐执行顺序

1. 修复 `spreadsheets` slug/path 漂移。
2. 修改 compiler，保留真实 skill path。
3. 增加 runtime/manifest 可加载性 validator。
4. 更新 `policy_contracts.rs`，使其匹配当前四 host 设计。
5. 将 router-rs crate tests 加入 CI。
6. 清理 routing eval fixtures 中不存在的 skill slug。
7. 改造 health manifest，使路径漂移等 contract 错误能被发现。
8. 盘点并清理 `route.rs` 中不存在的硬编码 slug。
9. 修复 fallback owner 对 `plan-to-code` 的过度偏向。
10. 把 host entrypoint hardcode 迁移到模板。
11. 建立统一 generated artifact manifest。
12. 增加 hot runtime selection explanation。
13. 加强中文路由 eval。
14. 改造 runtime launcher 的平台检查。
15. 明确 `.system` 技能边界。

## 最小可交付版本

如果只做一轮最小修复，建议至少完成以下项目：

- [ ] `spreadsheets` 可以按 runtime contract 正确加载。
- [ ] compiler 会拒绝生成不可加载的 manifest/runtime。
- [ ] `policy_contracts.rs` 不再与当前四 host 设计冲突。
- [ ] CI 会运行 router-rs crate tests。
- [ ] routing eval 不再引用不存在的 expected owner。
- [ ] health manifest 不再对路径漂移显示全绿。

完成这六项后，runtime 系统才算从“明显不可信”恢复到“基本可验证”。
