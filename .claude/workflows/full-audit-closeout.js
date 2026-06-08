import { agent, parallel, pipeline, phase, log } from "workflow"
export const meta = {
  name: 'full-audit-closeout',
  description: '11维度全面审计：commit审查→harness→文档→死代码→过期设计→测试覆盖→宿主特化→运维文档→对抗测试→验证→修复规划，200+ agent',
  phases: [
    { title: 'Commit审查', detail: '本周30个commit逐一审查' },
    { title: 'Harness审计', detail: '核心运行时代码质量扫描' },
    { title: '文档更新', detail: '全量文档准确性和完整性' },
    { title: '死代码清理', detail: '未引用代码和废弃文件' },
    { title: '过期设计', detail: '设计文档与实现脱节检查' },
    { title: '测试覆盖', detail: '关键路径测试缺口分析' },
    { title: '宿主特化', detail: '共享资产中的宿主耦合' },
    { title: '运维文档', detail: '多端同步和新宿主接入' },
    { title: '对抗测试', detail: '性能瓶颈和安全漏洞' },
    { title: '验证', detail: '独立验证每个发现的真实性' },
    { title: '修复规划', detail: '分批修复方案和commit计划' },
  ],
}

// ============================================================
// 每维度: 7子项 × 3阶段(review→verify→plan) = 21 agents
// 11维度 = 231 agents + 1 synthesis = 232 total
// ============================================================

const PIPELINE = [
  // ── DIM 0: 本周Commit审查 ──
  {
    dim: '本周Commit审查',
    items: [
      { name: 'commit_e09cdbd4', prompt: '审查 commit e09cdbd4: fix(harness): 审查发现5项修复—total语义、缓存去重、死代码、边界处理。检查代码变更的正确性、是否有遗漏修复、commit message是否准确。读 git diff e09cdbd4^..e09cdbd4' },
      { name: 'commit_6b3fbcfe', prompt: '审查 commit 6b3fbcfe: refactor(skills): 多agent深度审计—7 slug清理+配置四文件对齐+归档引用修复。检查 slug 清理是否彻底、配置文件是否真的对齐、归档引用是否还有断裂。读 git diff 6b3fbcfe^..6b3fbcfe' },
      { name: 'commit_8c702219', prompt: '审查 commit 8c702219: refactor: 长期运维全面修复—hook共享库+术语表+Rust拆分+类型安全。检查 hook 共享库设计是否合理、术语表是否完整、Rust 模块拆分边界是否清晰。读 git diff 8c702219^..8c702219' },
      { name: 'commits_hooks_lifecycle', prompt: '审查本周 hooks 和 lifecycle 相关 commits: aa6f7b49(test hooks), d91f13a6(refactor hooks lifecycle profile), f4a77758/4e334409/47a703e4(test fixes)。检查 advisory 模式测试是否充分、lifecycle profile 简化是否引入回归。读 git log --oneline --since=2026-05-27 -- hooks/ core/router-rs/src/hooks/' },
      { name: 'commits_skills_routing', prompt: '审查本周 skills 路由相关 commits: 2e30e993(路由配置清理), e838993e(安全隐患), cfd0efc2(三阶段深度审计), ffa525c0(plan-mode无宿主化)。检查路由 hints 去重是否完整、安全隐患是否彻底清除。读 git diff 2e30e993^..ffa525c0' },
      { name: 'commits_harness_features', prompt: '审查本周 harness 功能增强: 0591167d(缓存命中率优化), bedc6c26(新agent定义), 3745e3e7(Antigravity launcher), 0316447a(Cursor/Antigravity宿主对齐)。检查缓存优化效果、新agent定义完整性、宿主对齐一致性。读相关 diff' },
      { name: 'commit_overall_cohesion', prompt: '整体审查本周所有commits的演进逻辑是否连贯：是否有反复修改同一文件的commit（说明之前修复不彻底）、是否有遗漏的中间状态、跨commit的依赖是否正确。运行 git log --stat --since=2026-05-27 并分析文件修改频率和模式' },
    ],
  },

  // ── DIM 1: Harness 核心代码审计 ──
  {
    dim: 'Harness核心代码',
    items: [
      { name: 'rust_router_core', prompt: '审计 core/router-rs/src/ 中的路由核心逻辑。检查：路由决策的正确性、边界条件处理、错误传播链、是否有 unwrap/expect 在非测试代码中。读 core/router-rs/src/ 下的关键模块' },
      { name: 'rust_state_management', prompt: '审计 core/router-rs/src/ 中的状态管理代码（session、task、evidence相关）。检查：状态一致性保证、并发安全性、磁盘I/O错误处理、CAS操作正确性。关注 state.rs, session 相关模块' },
      { name: 'rust_mcp_tools', prompt: '审计 core/router-rs/src/ 中的 MCP tool 实现。检查：输入验证、返回值安全性、错误消息质量、是否有信息泄露风险。关注 mcp/ 或 tool 相关模块' },
      { name: 'rust_hook_integration', prompt: '审计 core/router-rs/src/ 中的 hook 集成代码。检查：hook 调用顺序保证、advisory vs hard-block 模式实现、超时处理、hook 输出解析安全性。关注 hook 相关模块' },
      { name: 'rust_host_adapters', prompt: '审计 core/router-rs/src/ 中的宿主适配器代码。检查：各宿主（Claude/Cursor/Codex/Antigravity/OpenCode）的差异处理是否正确、共享逻辑是否真的共享、宿主特化逻辑是否隔离在适配层。关注 host_integration 或 adapter 模块' },
      { name: 'rust_cache_layer', prompt: '审计 core/router-rs/src/ 中的缓存层。检查：缓存一致性、过期策略、缓存穿透防护、L1/L2分层是否合理、命中率优化的0591167d是否引入新问题。关注 cache 相关模块' },
      { name: 'rust_cli_and_config', prompt: '审计 core/router-rs/src/ 的 CLI 入口和配置加载。检查：配置验证、默认值合理性、环境变量覆盖安全性、help 文档准确性。关注 main.rs, cli.rs, config 相关文件' },
    ],
  },

  // ── DIM 2: 文档更新 ──
  {
    dim: '文档更新',
    items: [
      { name: 'doc_agents_md', prompt: '全面审查 AGENTS.md（主文档）和所有 AGENTS_*.md 宿主文档。检查：(1)内容是否反映本周的所有重构变更 (2)宿主文档间的共享内容是否一致 (3)宿主特有内容是否准确 (4)术语使用是否统一 (5)是否有过期的引用。读 AGENTS.md, AGENTS_CLAUDE.md, AGENTS_CURSOR.md, AGENTS_CODEX.md, AGENTS_ANTIGRAVITY.md, AGENTS_OPENCODE.md' },
      { name: 'doc_docs_directory', prompt: '全面审查 docs/ 目录下的所有文档。检查：(1)ARCHITECTURE.md 是否反映当前架构 (2)runtime_unified_spec.md 是否与代码一致 (3)harness_policy_map.md 是否最新 (4)host_adapter_contract.md 是否准确 (5)框架命名文档是否与代码匹配。读 docs/ 下主要文件' },
      { name: 'doc_skills_docs', prompt: '审查 skills/ 目录下的文档文件：CONTRIBUTING.md, SKILL_MAINTENANCE_GUIDE.md, SKILL_FRAMEWORK_PROTOCOLS.md, SKILL_ROUTING_LAYERS.md, PAPER_GATE_PROTOCOL*.md。检查内容准确性、是否有过期指导、与实际实现是否一致' },
      { name: 'doc_readme_files', prompt: '检查仓库中所有 README.md 文件（根目录、子目录）。检查：(1)是否过期 (2)安装/使用说明是否正确 (3)是否有断链 (4)是否反映最新架构' },
      { name: 'doc_changelog_and_migration', prompt: '检查是否有 changelog、migration guide、或 breaking changes 文档。如果本周有 breaking change 但没有文档记录，这是个问题。检查 docs/ 下是否有相关记录' },
      { name: 'doc_inline_docs', prompt: '审查核心 Rust 代码中的文档注释。检查：公共 API 是否有 doc comments、doc comments 是否准确（不与实现矛盾）、是否有过期的 TODO/FIXME。读 core/router-rs/src/ 下带 #[doc] 或 /// 的文件' },
      { name: 'doc_plan_and_design', prompt: '审查 docs/plans/ 和 docs/architecture/ 下的计划和设计文档。检查：(1)已完成的计划是否标注完成状态 (2)设计决策是否反映在代码中 (3)是否有过期的 ADR' },
    ],
  },

  // ── DIM 3: 死代码清理 ──
  {
    dim: '死代码清理',
    items: [
      { name: 'dead_rust_unused_imports', prompt: '扫描 core/router-rs/src/ 中的未使用导入和未使用变量。运行 grep 查找 #[allow(unused)]、#[allow(dead_code)] 标注，分析是否真的需要保留。这些标注可能是死代码的信号' },
      { name: 'dead_rust_unreachable', prompt: '扫描 core/router-rs/src/ 中的 unreachable!()、todo!()、unimplemented!() 宏。检查哪些是计划中的实现（应有对应issue/todo）哪些是真正的死代码' },
      { name: 'dead_json_configs', prompt: '扫描仓库中所有 JSON 配置文件，检查是否有过期或不再被引用的配置。特别关注 skills/ 下的 JSON 文件（SKILL_MANIFEST.json, SKILL_PLUGIN_CATALOG.json, SKILL_ROUTING_*.json）是否有冗余字段或废弃条目' },
      { name: 'dead_test_output', prompt: '检查仓库中的 .test-output.txt 文件（git status 显示有未跟踪的 .test-output.txt 和 core/router-rs/.test-output.txt）。这些是测试残留，应清理或加入 .gitignore' },
      { name: 'dead_archived_skills', prompt: '检查 skills/.archive-cold/ 目录。确认归档的 skills 是否还有被引用、是否应彻底删除、归档元数据是否完整。同时检查 skills/ 下是否有空目录或只有元数据的空 skill' },
      { name: 'dead_scripts_and_hooks', prompt: '扫描 hooks/ 目录和仓库中的 shell 脚本。检查：是否有不再被调用的脚本、是否有过期的 hook 配置、.claude/settings.json 中的 hook 引用是否指向有效脚本' },
      { name: 'dead_cross_references', prompt: '全面检查文件间的交叉引用。搜索文档中的文件路径引用（如 "see docs/xxx.md"），验证目标文件是否存在。特别关注本周重命名或移动的文件' },
    ],
  },

  // ── DIM 4: 过期设计扫描 ──
  {
    dim: '过期设计扫描',
    items: [
      { name: 'design_architecture_drift', prompt: '对比 docs/ARCHITECTURE.md 和 docs/architecture/ 下的设计文档与实际代码结构。检查：设计中描述的模块是否都存在、实际模块是否都在设计中有记录、接口签名是否匹配。读 ARCHITECTURE.md 并对比 core/router-rs/src/ 结构' },
      { name: 'design_contract_violations', prompt: '检查 docs/host_adapter_contract.md 和 docs/runtime_unified_spec.md 定义的契约是否被所有宿主适配器遵守。如果有契约但没有对应的验证测试，这是风险点' },
      { name: 'design_adr_currency', prompt: '审查 docs/adr/ 下的架构决策记录（ADR）。检查：(1)每个 ADR 的状态（proposed/accepted/deprecated）是否准确 (2)是否有已被代码推翻但未更新状态的 ADR (3)是否有应创建但未创建的 ADR' },
      { name: 'design_naming_conventions', prompt: '对比 docs/framework_naming_conventions.md 与实际代码命名。检查：代码中的函数名、模块名、文件名是否遵循文档中的约定、是否有不一致的命名模式' },
      { name: 'design_profile_contract', prompt: '审查 docs/framework_profile_contract.md 和 docs/operator_profiles.md。检查：my-light profile 的行为是否与代码实现一致、其他 profile（如 my-full）是否还有残留代码或配置' },
      { name: 'design_rfv_and_closeout', prompt: '审查 docs/rfv_loop_harness.md 和 docs/closeout_enforcement.md。检查：RFV 循环的实现是否与文档描述一致、closeout 的 advisory vs hard-block 行为是否准确文档化。对比 core/router-rs/src/ 中的实际实现' },
      { name: 'design_skill_tier_system', prompt: '审查 skills/SKILL_TIERS.json 和相关的 tier 分级逻辑。检查：tier 分级是否仍然有意义、是否有 skill 的 tier 分配不合理、tier 相关的代码逻辑是否与配置一致' },
    ],
  },

  // ── DIM 5: 测试覆盖度 ──
  {
    dim: '测试覆盖度',
    items: [
      { name: 'test_rust_unit', prompt: '检查 core/router-rs/ 中的单元测试覆盖。统计 #[test] 函数数量、检查哪些模块完全没有测试、哪些模块测试密度高。运行 find core/router-rs -name "*.rs" -exec grep -l "#\[test\]" {} \; 并分析覆盖分布' },
      { name: 'test_integration', prompt: '检查集成测试：是否有 tests/ 目录、集成测试是否覆盖关键路径（路由决策、状态管理、MCP工具调用、hook执行）。检查集成测试是否能独立运行（不依赖外部服务）' },
      { name: 'test_critical_paths', prompt: '识别代码中最关键的未测试路径：(1)错误恢复逻辑 (2)并发/锁操作 (3)磁盘I/O失败处理 (4)配置解析边界 (5)hook超时处理。为每个未覆盖的路径创建测试需求' },
      { name: 'test_edge_cases', prompt: '检查现有测试是否覆盖边界条件：空输入、超大输入、特殊字符、并发访问、资源耗尽场景。分析 core/router-rs/src/ 中的 public API 的边界条件测试' },
      { name: 'test_config_validation', prompt: '检查配置验证的测试覆盖：JSON schema 验证、必填字段缺失、类型错误、跨配置文件一致性检查。skills/ 下的 JSON 配置文件是否有验证测试' },
      { name: 'test_regression', prompt: '检查本周的修复 commits（e09cdbd4, f4a77758, 4e334409, 47a703e4）是否有对应的回归测试。每个 bug fix 应该有一个测试确保不会再次出现' },
      { name: 'test_ci_and_hooks', prompt: '检查 CI/测试基础设施：是否有 CI 配置文件、pre-commit hooks 是否运行测试、测试命令是否在 CONTRIBUTING.md 中文档化。检查 .github/ 或 Makefile 中的测试相关配置' },
    ],
  },

  // ── DIM 6: 宿主特化检查 ──
  {
    dim: '宿主特化检查',
    items: [
      { name: 'host_agents_variants', prompt: '逐文件对比 AGENTS_CLAUDE.md、AGENTS_CURSOR.md、AGENTS_CODEX.md、AGENTS_ANTIGRAVITY.md、AGENTS_OPENCODE.md。检查：(1)共享内容是否真的相同（不应有差异）(2)宿主特化内容是否隔离在正确的段落 (3)是否有宿主特化内容泄漏到应该共享的部分。用 diff 或逐段对比' },
      { name: 'host_shared_vs_specific_rust', prompt: '检查 core/router-rs/src/ 中的 Rust 代码。识别哪些代码是宿主特化的（应该在适配器层）哪些是共享的（应该在核心层）。检查是否有核心层代码包含 if host == "claude" 之类的条件分支' },
      { name: 'host_skill_routing', prompt: '检查 skills/SKILL_ROUTING_RUNTIME.json 和 SKILL_ROUTING_METADATA.json。确认路由规则不偏向任何特定宿主、每个 skill 的路由条件是否宿主无关、是否有宿主特化的路由条目不应在共享配置中' },
      { name: 'host_hook_scripts', prompt: '检查 hook 脚本是否宿主无关。如果有宿主特化的 hook 逻辑，应该通过参数或配置注入而不是硬编码。检查 hooks/ 目录下的脚本' },
      { name: 'host_docs_shared', prompt: '检查 docs/ 下的文档。共享文档（如 ARCHITECTURE.md, runtime_unified_spec.md）中不应包含宿主特化内容。宿主特化文档应在 docs/hosts/ 目录下' },
      { name: 'host_config_files', prompt: '检查 .claude/ 目录下的配置。settings.json 和相关配置中的 MCP 服务器、权限等是否宿主无关。如果配置中硬编码了特定宿主的路径或设置，这是宿主特化泄漏' },
      { name: 'host_env_dependencies', prompt: '检查代码和配置中的环境变量依赖。是否有只在特定宿主环境中存在的环境变量被假设存在？是否有平台特定路径（如 macOS 特定路径）被硬编码在共享代码中？' },
    ],
  },

  // ── DIM 7: 运维文档优化 ──
  {
    dim: '运维文档优化',
    items: [
      { name: 'ops_installation', prompt: '审查安装和初始设置文档。检查：(1)新用户能否按文档成功安装 (2)依赖项是否完整列出 (3)是否有隐含的假设未文档化 (4)不同宿主的安装步骤是否有差异文档' },
      { name: 'ops_daily_maintenance', prompt: '审查日常运维文档：框架更新流程、skill 更新流程、配置迁移流程。检查是否有文档化的 SOP（标准操作流程）' },
      { name: 'ops_multi_host_sync', prompt: '审查多端同步机制。用户希望多端同步：(1)框架状态如何跨设备同步 (2)配置如何分发 (3)冲突解决策略是否文档化 (4)是否有 sync 命令或工具' },
      { name: 'ops_new_host_onboarding', prompt: '审查新宿主接入指南。用户希望方便接入新宿主：(1)是否有 step-by-step 接入文档 (2)需要修改哪些文件的清单 (3)验证接入成功的检查列表 (4)现有宿主（Claude/Cursor/Codex/Antigravity/OpenCode）的接入是否可作为模板' },
      { name: 'ops_troubleshooting', prompt: '审查故障排查文档。检查：(1)常见错误是否有对应的排查指南 (2)日志在哪里查看 (3)状态重置流程 (4)框架 snapshot 和 goal state 的恢复流程' },
      { name: 'ops_monitoring', prompt: '审查监控和健康检查机制。检查：(1)是否有框架健康检查命令 (2)skill 路由是否正常如何验证 (3)缓存状态如何查看 (4)evidence 积累是否正常如何确认' },
      { name: 'ops_backup_and_recovery', prompt: '审查备份和恢复策略。检查：(1)artifacts/ 目录是否有备份策略 (2)会话状态丢失后如何恢复 (3)框架升级失败如何回滚 (4)是否有灾难恢复文档' },
    ],
  },

  // ── DIM 8: 对抗性测试 ──
  {
    dim: '对抗性测试',
    items: [
      { name: 'perf_bottlenecks', prompt: '性能瓶颈分析：审查 core/router-rs/src/ 中的热路径。检查：(1)是否有不必要的同步操作 (2)磁盘I/O是否可以批量化 (3)字符串分配是否可以优化 (4)是否有 O(n²) 或更差的算法。关注路由决策和缓存查找路径' },
      { name: 'sec_vulnerabilities', prompt: '安全漏洞扫描：检查 (1)路径遍历风险（文件路径处理）(2)JSON注入（配置解析）(3)命令注入（shell脚本执行）(4)信息泄露（错误消息中暴露内部状态）(5)权限提升（hook执行权限）' },
      { name: 'stress_race_conditions', prompt: '并发和竞争条件分析：检查 (1)文件锁的使用是否正确 (2)是否有 TOCTOU（time-of-check-time-of-use）问题 (3)多进程/多会话并发访问同一文件是否安全 (4)CAS 操作是否有 ABA 问题' },
      { name: 'stress_memory_leaks', prompt: '内存问题分析：检查 Rust 代码中是否有 (1)循环引用（Rc/Arc 环）(2)未释放的资源（文件句柄、临时文件）(3)缓存无限增长 (4)大字符串不必要的 clone' },
      { name: 'stress_error_handling', prompt: '错误处理全面审查：检查 core/router-rs/src/ 中的错误传播。是否有 (1)吞掉错误的 .ok() 或 let _ = (2)panic 路径在非测试代码中 (3)错误信息不充分导致无法调试 (4)错误恢复逻辑中的状态不一致' },
      { name: 'stress_input_validation', prompt: '输入验证审查：检查所有外部输入点（MCP tool 参数、配置文件解析、CLI 参数、环境变量）。是否有 (1)缺失的长度限制 (2)缺失的格式验证 (3)类型混淆风险 (4)特殊字符未转义' },
      { name: 'stress_resource_exhaustion', prompt: '资源耗尽攻击分析：检查 (1)是否可以构造超大配置导致 OOM (2)是否有递归深度限制 (3)临时文件是否有清理机制 (4)日志是否有大小限制 (5)缓存是否有容量上限' },
    ],
  },

  // ── DIM 9: 验证 ──
  {
    dim: '验证',
    items: [
      { name: 'verify_commit_findings', prompt: '独立验证 Commit 审查维度的发现。对每个声称的问题：(1)读取相关代码确认 (2)检查是否有意为之的设计 (3)评估严重程度。只确认真实问题，驳回误报' },
      { name: 'verify_harness_findings', prompt: '独立验证 Harness 代码审计的发现。对每个声称的代码质量问题：(1)读取代码上下文确认 (2)检查是否有测试覆盖 (3)评估修复优先级' },
      { name: 'verify_doc_findings', prompt: '独立验证文档审计的发现。对每个文档不准确的地方：(1)对比文档和代码确认差异 (2)评估差异的实际影响 (3)确认是否需要更新' },
      { name: 'verify_deadcode_findings', prompt: '独立验证死代码发现。对每个声称的死代码：(1)grep 全仓库确认无引用 (2)检查是否有动态引用（字符串拼接路径等）(3)确认删除安全性' },
      { name: 'verify_test_findings', prompt: '独立验证测试覆盖度发现。对每个声称的测试缺口：(1)确认确实没有测试 (2)评估该路径的风险等级 (3)确认是否可以通过现有测试间接覆盖' },
      { name: 'verify_host_findings', prompt: '独立验证宿主特化发现。对每个声称的宿主耦合：(1)确认是否是有意的宿主适配 (2)检查是否应在适配器层而非核心层 (3)评估重构成本' },
      { name: 'verify_perf_sec_findings', prompt: '独立验证性能和安全发现。对每个瓶颈/漏洞：(1)评估是否在实际使用中会被触发 (2)确认利用难度 (3)评估修复收益是否值得' },
    ],
  },

  // ── DIM 10: 修复规划 ──
  {
    dim: '修复规划',
    items: [
      { name: 'plan_critical_fixes', prompt: '规划关键修复（P0）：安全漏洞、数据损坏风险、阻塞性 bug。每个修复包含：问题描述、修复方案、涉及文件、预估工作量、测试策略' },
      { name: 'plan_doc_updates', prompt: '规划文档更新批次：识别所有需要更新的文档，按依赖关系排序（先更新核心文档再更新引用文档），确定每个文档的更新内容' },
      { name: 'plan_deadcode_removal', prompt: '规划死代码清理批次：按风险排序（先删确认安全的再删需要进一步验证的），确定每个删除的影响范围，规划回归测试策略' },
      { name: 'plan_test_additions', prompt: '规划测试补充：按风险优先级排序测试缺口，确定每个测试的类型（单元/集成/端到端），规划测试数据准备' },
      { name: 'plan_host_cleanup', prompt: '规划宿主特化清理：识别需要重构为共享的代码，确定重构策略（提取到共享模块 vs 配置化），规划向后兼容策略' },
      { name: 'plan_ops_improvements', prompt: '规划运维文档改进：确定新增文档列表、更新文档列表、文档结构重组方案。重点关注多端同步和新宿主接入的文档化' },
      { name: 'plan_commit_strategy', prompt: '规划 commit 策略：将所有修复按逻辑分组为 commit 批次，确定每个 commit 的 message、影响范围、是否需要 PR。确保每个 commit 可独立通过 CI' },
    ],
  },
]

// ── stage prompt composers ──

function reviewPrompt(dim, item) {
  return `面向用户的可见输出使用简体中文。

你是一个严格的代码审计 agent。任务：对 "${dim}" 维度中的 "${item.name}" 进行深度审查。

## 审查要求
${item.prompt}

## 输出格式
对每个发现的问题，输出以下 JSON 格式：
{
  "findings": [
    {
      "title": "简短标题",
      "severity": "critical|high|medium|low|info",
      "file": "文件路径:行号",
      "description": "详细描述问题",
      "evidence": "代码片段或具体证据",
      "suggestion": "建议的修复方案"
    }
  ]
}

## 要求
- 仔细阅读实际代码/文件，不要猜测
- 每个发现必须有具体证据（文件路径+行号+代码片段）
- 如果没有发现问题，返回 {"findings": []}
- severity 分级标准：critical=安全/数据风险, high=功能缺陷, medium=质量/维护性, low=风格/建议, info=信息性
- 目标是发现真实问题，不要为了凑数而创造问题`
}

function verifyPrompt(dim, item) {
  return `面向用户的可见输出使用简体中文。

你是一个独立的验证 agent。任务：对 "${dim}" 维度 "${item.name}" 的审查发现进行独立验证。

## 上一阶段的审查结果
\`\`\`
${'${prevResult}'}
\`\`\`

## 验证要求
对每个 finding 进行独立验证：
1. 读取实际代码/文件确认问题是否真实存在
2. 检查是否是有意的设计决策（而非 bug）
3. 评估问题的实际影响（是否在生产中会被触发）

## 输出格式
{
  "verdicts": [
    {
      "title": "原始发现标题",
      "verdict": "confirmed|false_positive|needs_review",
      "confidence": "high|medium|low",
      "reasoning": "验证推理过程",
      "adjusted_severity": "如果确认，可以调整原始 severity"
    }
  ],
  "confirmed_count": 0,
  "false_positive_count": 0
}

## 要求
- 必须读取实际代码进行验证，不能仅凭描述判断
- 对 false positive 给出明确理由
- 对 needs_review 给出需要人工确认的原因`
}

function planPrompt(dim, item) {
  return `面向用户的可见输出使用简体中文。

你是一个修复规划 agent。任务：为 "${dim}" 维度 "${item.name}" 的已确认问题制定修复方案。

## 验证结果
\`\`\`
${'${prevResult}'}
\`\`\`

## 规划要求
仅为 confirmed（已确认）的问题制定修复方案。每个修复包含：
1. 具体的修复步骤
2. 需要修改的文件和位置
3. 修改前后的代码对比
4. 需要添加的测试
5. 修复优先级和分批建议

## 输出格式
{
  "fixes": [
    {
      "title": "修复标题",
      "priority": "P0|P1|P2|P3",
      "files_to_modify": ["文件路径"],
      "fix_description": "具体修复步骤",
      "test_needed": "需要添加的测试",
      "estimated_effort": "small|medium|large"
    }
  ],
  "batch_suggestion": "建议的 commit 分批策略"
}

## 要求
- P0: 安全/数据风险, P1: 功能缺陷, P2: 质量/维护性, P3: 优化/美化
- fix_description 要具体到代码级别
- 无 confirmed 问题时返回 {"fixes": [], "batch_suggestion": "无需修复"}`
}

// ── pipeline runner ──

async function runDimension(dimDef) {
  const { dim, items } = dimDef
  const results = await pipeline(
    items,
    // Stage 1: Review
    (item) => agent(reviewPrompt(dim, item), {
      label: `review:${item.name}`,
      phase: dim,
    }),
    // Stage 2: Verify
    (prevResult, item) => agent(verifyPrompt(dim, item).replace('${prevResult}', prevResult), {
      label: `verify:${item.name}`,
      phase: dim,
    }),
    // Stage 3: Fix Plan
    (prevResult, item) => agent(planPrompt(dim, item).replace('${prevResult}', prevResult), {
      label: `plan:${item.name}`,
      phase: dim,
    })
  )
  return results.filter(Boolean)
}

// ============================================================
// MAIN EXECUTION
// ============================================================

log('=== 开始全面审计：11维度 × 21 agents/维度 = 231 agents ===')

// Phase 0: Run all dimensions sequentially
const allDimensionResults = []
for (const dimDef of PIPELINE) {
  log(`▸ 开始维度: ${dimDef.dim} (${dimDef.items.length} 子项 × 3 阶段 = ${dimDef.items.length * 3} agents)`)
  const results = await runDimension(dimDef)
  allDimensionResults.push({ dim: dimDef.dim, results })
  log(`▸ 完成维度: ${dimDef.dim}`)
}

// Phase 1: Cross-dimension synthesis
log('=== 开始跨维度综合分析 ===')

// Extract plan results: pipeline returns [r0,v0,p0, r1,v1,p1, ...] per dimension
// Plans are at indices 2, 5, 8, ... (every 3rd starting from 2)
const planResults = allDimensionResults.map(d => {
  const plans = []
  for (let i = 2; i < d.results.length; i += 3) {
    if (d.results[i]) plans.push(d.results[i])
  }
  return { dim: d.dim, plans }
})

const finalReport = await agent(`面向用户的可见输出使用简体中文。

你是一个综合审计报告 agent。你收到了 11 个维度的完整审计结果，每个维度包含 review→verify→plan 三个阶段的输出。

## 各维度修复规划结果

${planResults.map(d => `### ${d.dim}\n${d.plans.map(p => p.substring(0, 500)).join('\n---\n')}`).join('\n\n')}

## 任务
1. 合并所有维度的修复方案
2. 按优先级排序（P0 > P1 > P2 > P3）
3. 按逻辑分组为 commit 批次（每个 commit 应可独立通过 CI）
4. 识别跨维度的依赖关系
5. 输出最终的执行计划

## 输出格式（严格 JSON）
{
  "summary": "整体审计概况（200字以内）",
  "stats": {
    "total_findings": 0,
    "confirmed": 0,
    "by_severity": {"critical": 0, "high": 0, "medium": 0, "low": 0},
    "by_dimension": {"维度名": 0}
  },
  "commit_batches": [
    {
      "batch_id": 1,
      "message": "commit message",
      "description": "这批修改的内容描述",
      "priority": "P0",
      "fixes": ["修复1描述", "修复2描述"],
      "files": ["涉及的文件"],
      "effort": "small|medium|large"
    }
  ],
  "documentation_updates": ["文档更新列表"],
  "dead_code_removals": ["待删除文件/代码列表"],
  "test_additions": ["待添加测试列表"],
  "risks_and_notes": ["风险提示和注意事项"]
}`, {
  label: 'synthesis:final-report',
  phase: '综合报告',
})

log('=== 审计完成：232 agents 已执行 ===')
return { allDimensionResults, planResults, finalReport }
