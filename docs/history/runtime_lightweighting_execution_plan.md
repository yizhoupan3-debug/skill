# Runtime Lightweighting Execution Plan

日期：2026-04-27

目标：让默认 runtime 更轻，减少兼容层、胶水层和入口数量，同时不损害现有功能。这里的“减法”不是直接删能力，而是把默认路径压窄，把调试、兼容、迁移和专业能力移到显式入口或生成物里。

## 1. 直接结论

当前系统真正的负担不在“core skill 太多”。`skills/SKILL_TIERS.json` 现在已经是 13 个 core、113 个 optional；这个默认面比旧文档里的 16 core 更干净。

更值得优先处理的是四类负担：

| 优先级 | 负担 | 为什么重 | 推荐动作 |
|---|---|---|---|
| P0 | hot runtime 到 full manifest 的 fallback 太宽 | “runtime 轻量化”类任务会误打到 `backend-runtime-debugging` 或 `plan-to-code` | 收紧 fallback，只在 hot no-hit 或显式 full search 时进入 full manifest |
| P1 | 文档和 skill 仍暴露旧顶层 flags | 已退休入口还在被文档教用户使用，会制造兼容层需求 | 全部改成 canonical subcommands |
| P2 | surface/loadout/tier 三套激活叙事重复 | 使用者和维护者会以为有三套默认面 | 只保留一个真源，其余变成生成报告 |
| P3 | `router-rs/src/main.rs` 承载太多域逻辑 | CLI、stdio、runtime control、sandbox、trace、execute、tests 混在一起 | 先搬模块，不改语义，再删胶水 |

## 2. 当前证据快照

| 项 | 当前值 | 判断 |
|---|---:|---|
| hot runtime entries | 24 | 合理，13 required gates 加 11 preferred owners |
| core skills | 13 | 已经完成一轮减法，不是当前最大问题 |
| optional skills | 113 | 不在默认面，保留为能力库 |
| `SKILL_ROUTING_RUNTIME.json` | 7.9 KB | 默认路由索引很轻 |
| `SKILL_MANIFEST.json` | 50.5 KB | full manifest 应只用于搜索和显式 fallback |
| `SKILL_TIERS.json` | 50.1 KB | 更像生成报告，不应被当作第二真源 |
| `SKILL_LOADOUTS.json` | 1.7 KB | 内容小，但概念上重复 |
| `SKILL_APPROVAL_POLICY.json` | 29.1 KB | 先确认活跃消费者，再决定是否移动 |
| `SKILL_SHADOW_MAP.json` | 34.0 KB | 编译诊断，不应进入默认 runtime 路径 |
| `configs/framework/FRAMEWORK_SURFACE_POLICY.json` | 2.6 KB | 最适合做 surface 真源 |
| `configs/framework/RUNTIME_REGISTRY.json` | 10.5 KB | alias、explicit command 和 host entrypoints 真源，保留 |
| `scripts/router-rs/src/main.rs` | 10,541 lines | 结构性负担最大 |
| stdio op dispatch cases | 53 | 需要分组隐藏，不应该像公开入口一样散落 |
| repo-local Rust target dirs | 329M + 836M | 构建缓存负担，可清理或统一 target dir |
| `tools/browser-mcp/node_modules` | 99M | dev/parity 负担，不能作为 live runtime |

实测问题：

```bash
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml route '核查我现在的runtime，什么部分是没有用的或者加重负担的，值得去掉的？'
```

当前会选到 `backend-runtime-debugging`。原因是 full manifest fallback 接受了 generic `runtime` 命中。

```bash
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml route '继续深度诊断，写更周密的可执行计划，我希望更加轻量化，减少兼容层和胶水层，减少入口，不要损害功能'
```

当前会落到 `plan-to-code` fallback，score 为 0。原因是 `skill-framework-developer` 的 hot trigger 还没有覆盖“轻量化 / 兼容层 / 胶水层 / 减入口”这些表达。

这说明下一步不能先删文件。要先修路由判断，否则后续自动化会继续把框架减法任务导向错误 owner。

## 3. 减法判定标准

一个部分值得去掉或下沉，需要满足至少一个条件：

| 条件 | 说明 | 处理 |
|---|---|---|
| 默认路径会读取它，但它不决定默认行为 | 典型是重复生成物或 debug 报告 | 移出默认路径 |
| 它暴露了旧入口 | 典型是 retired top-level flags 文档 | 改成 canonical entrypoint |
| 它只是把 A 转发到 B | 典型是兼容 shim、bridge、wrapper | 删除或改为 fail-fast migration |
| 它和另一个文件表达同一份策略 | 典型是 surface/loadout/tier 重复 | 保留一个真源 |
| 它让错误 owner 更容易被选中 | 典型是宽 fallback 或 generic trigger | 收紧路由 |
| 它是 dev/parity 依赖却常驻仓库运行路径 | 典型是 TypeScript Browser MCP live fallback | 标记 dev-only，默认不安装不调用 |

一个部分暂时不该删，需要满足任一条件：

| 条件 | 说明 |
|---|---|
| 它是用户显式能力入口 | 例如 `browser mcp-stdio`、显式 `router framework ...` 子命令 |
| 它是安全的迁移保护 | 例如 retired flag fail-fast guidance |
| 它是功能回归测试的唯一覆盖 | 先迁移测试，再删实现 |
| 它只按需读取，不在默认 runtime 热路径 | 例如 Cloudflare references |
| 消费者还没有确认 | 例如 `SKILL_APPROVAL_POLICY.json` |

## 4. 目标形态

默认链路只保留这条：

```text
AGENTS.md
-> skills/SKILL_ROUTING_RUNTIME.json
-> narrow owner / required gate / optional overlay
-> skills/<name>/SKILL.md
-> evidence-backed completion
```

默认 kernel 只回答四个问题：

| Kernel 轴 | 负责什么 | 不负责什么 |
|---|---|---|
| routing | 选最窄 owner/gate/overlay | 不承载全部 skill 库 |
| continuity | 读写当前任务状态 | 不复制历史恢复工件 |
| memory | 召回稳定项目事实 | 不做技能路由 |
| codex host payload | 生成 Codex 宿主配置和入口 | 不投影多宿主兼容层 |

其他能力全部变成显式 capability：

| Capability | 保留方式 |
|---|---|
| trace/storage/sandbox/background | Rust stdio 内部能力，不作为用户默认入口 |
| observability dashboard/catalog/exporter | capability artifact 或 debug output |
| approval/loadout/profile 扩展 | profile artifacts 或显式 framework command |
| full manifest search | `search` 或显式 route fallback |
| dev/parity harness | dev-only，不在 live runtime |

## 5. P0：先修路由，不让减法任务跑偏

### 问题

`route_task_with_manifest_fallback()` 现在会在 hot decision 之后加载 full manifest，并且只要 full decision 分数更高就接受。这会让 generic `runtime` 把框架 runtime 减法任务导到 `backend-runtime-debugging`。

同时，当前 hot triggers 没有覆盖这些框架减法表达：

- `runtime 轻量化`
- `兼容层`
- `胶水层`
- `减少入口`
- `减入口`
- `不损害功能`

### 修改

1. 修改 `route_task_with_manifest_fallback()` 的接受条件。
2. full manifest 只允许在这些情况接管：
   - hot decision 是真正 no-hit。
   - hot decision 是 fallback owner，且 full decision 有明确非 generic trigger。
   - 用户显式传入 full manifest 搜索或显式 slug。
   - hot runtime 没有某个必需 artifact/source gate，而 full manifest 有精确命中。
3. generic tokens 不允许独自触发 full manifest 接管：
   - `runtime`
   - `debug`
   - `backend`
   - `review`
   - `plan`
4. 给 `skill-framework-developer` 增加框架减法触发词，而不是让 `plan-to-code` 或 `backend-runtime-debugging` 接管。
5. 保留 full manifest search 的能力，不影响 `search <query> --json`。

### 回归用例

加入 `tests/routing_eval_cases.json`：

```json
{
  "id": "skill-framework-runtime-lightweighting-review",
  "category": "should-trigger",
  "task": "核查我现在的runtime，什么部分是没有用的或者加重负担的，值得去掉的？",
  "focus_skill": "skill-framework-developer",
  "expected_owner": "skill-framework-developer",
  "expected_overlay": null,
  "forbidden_owners": ["backend-runtime-debugging", "plan-to-code"],
  "first_turn": true
}
```

```json
{
  "id": "skill-framework-compat-glue-entrypoint-plan",
  "category": "should-trigger",
  "task": "继续深度诊断，写更周密的可执行计划，我希望更加轻量化，减少兼容层和胶水层，减少入口，不要损害功能",
  "focus_skill": "skill-framework-developer",
  "expected_owner": "skill-framework-developer",
  "expected_overlay": null,
  "forbidden_owners": ["backend-runtime-debugging", "plan-to-code"],
  "first_turn": true
}
```

还要加入一个反向保护用例：

```json
{
  "id": "backend-runtime-failure-still-routes-to-debugging",
  "category": "should-trigger",
  "task": "后端 Rust 服务运行时 OOM 并且请求一直 hang，先诊断 traceback、deadlock 和资源泄漏",
  "focus_skill": "backend-runtime-debugging",
  "expected_owner": "backend-runtime-debugging",
  "expected_overlay": null,
  "forbidden_owners": ["skill-framework-developer"],
  "first_turn": true
}
```

### 验收

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml routing_eval_runtime_fallback_matches_expected_baseline --quiet
cargo test --manifest-path scripts/router-rs/Cargo.toml routing_eval_report_matches_expected_baseline --quiet
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml route '核查我现在的runtime，什么部分是没有用的或者加重负担的，值得去掉的？'
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml route '后端 Rust 服务运行时 OOM 并且请求一直 hang，先诊断 traceback、deadlock 和资源泄漏'
```

通过标准：

- runtime 轻量化任务选择 `skill-framework-developer`。
- 真实 backend runtime failure 仍选择 `backend-runtime-debugging`。
- full manifest search 不被删除。
- hot runtime 默认路径不因为 generic `runtime` 被 full manifest 抢走。

## 6. P1：清理已退休入口，只保留 canonical commands

### 问题

默认 help 已经只展示 9 个 canonical subcommands：

```text
route
search
framework
codex
trace
storage
browser
profile
migrate
```

历史上曾存在若干 **router-rs 顶层 JSON-only flags** 与独立的 **continuity 剪贴板 refresh 入口**；steady-state 已收敛为 `router framework ...` / `codex sync` / stdio `execute` 等 canonical 面（细节以当期实现与 `RTK.md`、`docs/rust_contracts.md` 为准）。

### 修改（归档摘要）

1. 删除已退役 skill 与历史文档中的旧入口叙述；用户侧只保留 canonical 命令示例。
2. `retired_top_level_flag_migration()` 等保护层可继续存在，但**面向用户的 markdown**不得再宣传已删除 flags。

### 验收（归档摘要）

- 跑 `cargo test --test policy_contracts` 与 `router-rs --help`，确认 canonical subcommands 与契约测试仍成立。
- 对用户文档做一次定向检索：不得再出现已删除的顶层 JSON flag 拼写（具体集合以 `policy_contracts::removed_router_flags_are_absent_from_user_docs` 为准）。

通过标准：

- 用户文档不再展示 retired top-level flags。
- 需要 fail-fast 迁移提示时，由代码侧集中处理，而不是复制第二份“教程型”真源。

## 7. P2：合并 surface/loadout/tier 真源

### 问题

现在有三份文件都在讲 default、explicit opt-in、core、optional：

- `configs/framework/FRAMEWORK_SURFACE_POLICY.json`
- `skills/SKILL_LOADOUTS.json`
- `skills/SKILL_TIERS.json`

这不是运行时大文件问题，而是认知负担问题。维护者会不知道到底改哪一个。

### 目标

只保留一个 authoring source：

```text
configs/framework/FRAMEWORK_SURFACE_POLICY.json
```

其余都必须是 compiled output 或 debug output。

### 修改

| 文件 | 目标状态 | 动作 |
|---|---|---|
| `FRAMEWORK_SURFACE_POLICY.json` | 真源 | 保留，并写明 source-of-truth |
| `SKILL_TIERS.json` | 生成报告 | 保留，但标注 generated debug/report |
| `SKILL_LOADOUTS.json` | 可折叠 | 第一阶段保留生成，第二阶段并入 surface policy 或改为 on-demand |
| `SKILL_APPROVAL_POLICY.json` | 待审计 | 先查活跃消费者，再决定保留为 projection 或移到 profile artifacts |
| `SKILL_SHADOW_MAP.json` | 编译诊断 | 移到 debug output 或只在 compiler debug 模式生成 |

### 执行顺序

1. 给 `build_framework_surface_policy()` 增加字段，声明：
   - `source_of_truth: true`
   - `derived_reports: ["skills/SKILL_TIERS.json"]`
   - `deprecated_or_foldable_reports: ["skills/SKILL_LOADOUTS.json"]`
2. 修改 `build_loadouts()`，让它从 surface policy 派生，不再有独立默认值。
3. 查找 `SKILL_LOADOUTS.json` 活跃消费者。
4. 如果只有 docs/tests/compiler 引用，则从默认 `write_bundle()` 中移除，改成 `--emit-debug-artifacts` 或写入 `artifacts/generated/skill-diagnostics/`。
5. 对 `SKILL_APPROVAL_POLICY.json` 单独做消费者审计。
6. 对 `SKILL_SHADOW_MAP.json` 改成 debug-only。

### 验收

```bash
rg -n 'SKILL_LOADOUTS|SKILL_TIERS|FRAMEWORK_SURFACE_POLICY|SKILL_APPROVAL_POLICY|SKILL_SHADOW_MAP' scripts tests docs configs
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- apply --skills-root skills
jq '.skill_system.activation_counts' configs/framework/FRAMEWORK_SURFACE_POLICY.json
jq '.summary.activation_counts' skills/SKILL_TIERS.json
cargo test --test policy_contracts --quiet
```

通过标准：

- activation counts 仍是 default 13、explicit_opt_in 113。
- 只有 `FRAMEWORK_SURFACE_POLICY.json` 被描述为真源。
- `SKILL_LOADOUTS.json` 不再是独立策略来源。
- 如果删除或移动生成物，所有消费者都有替代路径。

## 8. P3：把 stdio op registry 从“入口森林”改成内部能力表

### 问题

`dispatch_stdio_json_request()` 现在有 53 个 op case。它们不是用户入口，但写在 `main.rs` 里，看起来像一个隐形入口森林。

当前 op 可以分成这些族：

| 族 | 例子 | 目标 |
|---|---|---|
| route/search | `route`, `search_skills`, `route_report` | 保留 |
| execution contract | `execute`, `decode_execution_response` | 移到 `execute_contract.rs` |
| profile compile | `compile_profile_bundle` | 移到 profile module |
| sandbox/background | `sandbox_control`, `background_control` | 移到 `sandbox_control.rs` 和 `background_control.rs` |
| trace/transport | `describe_transport`, `write_trace_metadata` | 移到 `trace_io.rs` |
| runtime storage | `runtime_storage` | 已有 module，dispatch 下沉 |
| framework | `framework_runtime_snapshot`, `framework_alias` | 移到 `framework_stdio.rs` |
| observability | exporter/catalog/dashboard/metric | debug/capability output |

### 修改

1. 新建 `stdio_ops.rs`，只保留一个 `dispatch_stdio_json_request()`。
2. 每个族有自己的 `try_dispatch_*()`。
3. `main.rs` 只保留 CLI parse、top-level dispatch、stdio loop entry。
4. op 名不变，响应 schema 不变。
5. 不把 stdio op 写入用户文档，除非它是明确 contract。

### 验收

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
rg -n 'fn dispatch_stdio_json_request|match op|\"runtime_observability_|\"trace_record_event\"|\"sandbox_control\"' scripts/router-rs/src
```

通过标准：

- 所有旧 stdio op 仍可被测试调用。
- `main.rs` 不再直接维护 50 多个 op case。
- 用户可见入口仍只有 9 个 canonical subcommands。

## 9. P4：拆 `main.rs`，减少胶水层

### 问题

`scripts/router-rs/src/main.rs` 现在超过 10k 行，里面同时有：

- CLI struct 和 command dispatch。
- route fallback。
- stdio operation dispatch。
- live execute HTTP client。
- sandbox lifecycle。
- background job control。
- trace attach/replay/compact。
- runtime control plane。
- observability schema。
- 大量测试。

这会造成两个问题：

- 每次改一处 runtime 逻辑，都容易触碰不相关 glue。
- 难判断哪些是默认 kernel，哪些只是 capability。

### 目标模块

| 新模块 | 迁移内容 | 验收 |
|---|---|---|
| `cli_commands.rs` | Clap structs 和 canonical command dispatch | `main.rs` 不再承载 command enum |
| `route_fallback.rs` | manifest fallback policy | P0 tests 通过 |
| `stdio_ops.rs` | stdio op family registry | 53 op 从 `main.rs` 移走 |
| `execute_contract.rs` | dry/live execute contract | execute tests 通过 |
| `sandbox_control.rs` | sandbox lifecycle | sandbox tests 通过 |
| `background_control.rs` | background control response | background tests 通过 |
| `runtime_control.rs` | runtime control plane/integrator | runtime control tests 通过 |
| `runtime_observability.rs` | metrics/catalog/dashboard/record | observability tests 通过 |
| `trace_io.rs` | trace transport/handoff/write helpers | trace tests 通过 |

### 执行规则

1. 每次只搬一个模块。
2. 搬迁时不改 JSON schema。
3. 先搬代码，再跑定向测试。
4. 最后再做命名清理，不在搬迁 PR 里混合行为变化。

### 验收

```bash
wc -l scripts/router-rs/src/main.rs
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
cargo test --test policy_contracts --quiet
```

目标：

- 第一轮把 `main.rs` 压到 7k 以下。
- 第二轮压到 4k 以下。
- 第三轮 `main.rs` 只剩 process entry、CLI parse、dispatch。

## 10. P5：压缩 runtime output modes 和观测胶水

### 问题

`cli_modes.rs` 暴露了这些 stdio-only runtime outputs：

- `runtime_integrator`
- `runtime_control_plane`
- `runtime_observability_exporter_descriptor`
- `runtime_observability_metric_catalog`
- `runtime_observability_dashboard_schema`
- `runtime_metric_record`

这些都不是用户入口，但它们现在像并列 surface。长期看，应该变成一个 capability bundle，而不是多个默认输出模式。

### 修改

1. 保留 `runtime_control_plane` 作为聚合入口。
2. 把 `runtime_integrator` 标成 deprecated internal bundle，确认没有外部 caller 后删除。
3. exporter/catalog/dashboard/metric record 改成 observability capability 下的 debug outputs。
4. `runtime_observability_health_snapshot` 保留为健康检查内部 op。
5. 文档只描述“observability capability”，不描述每个 stdio op。

### 验收

```bash
rg -n 'runtime_integrator|runtime_observability_exporter_descriptor|runtime_observability_metric_catalog|runtime_observability_dashboard_schema|runtime_metric_record' scripts docs tests configs
cargo test --manifest-path scripts/router-rs/Cargo.toml runtime_observability --quiet
cargo test --manifest-path scripts/router-rs/Cargo.toml runtime_control_plane_payload_is_rust_owned --quiet
```

通过标准：

- 功能仍可通过 `runtime_control_plane` 或 explicit capability artifact 获得。
- 默认文档不再把每个 runtime output mode 当入口。
- 旧 op 如果还存在，只在测试和内部 dispatch 中出现。

## 11. P6：清理 dev-only 体积，不动 live 功能

### 问题

本地体积主要来自构建缓存和 dev dependency：

| 路径 | 体积 | 判断 |
|---|---:|---|
| `scripts/router-rs/target` | 329M | 构建缓存 |
| `rust_tools/target` | 836M | 构建缓存 |
| `tools/browser-mcp/node_modules` | 99M | dev/parity dependency |
| `skills/cloudflare-deploy/references` | 1.8M / 307 files | 按需知识库，不是 runtime 热路径 |

### 修改

1. 统一使用 shared target dir：

```bash
export CARGO_TARGET_DIR=/tmp/skill-cargo-target
```

2. 在 cleanup 文档里明确 repo-local target 可删。
3. `tools/browser-mcp/node_modules` 不进入默认 setup。
4. Browser MCP live path 继续是 Rust `browser mcp-stdio`。
5. TypeScript Browser MCP 保留为 dev/parity harness，只有显式 dev test 才安装。
6. Cloudflare references 不作为 runtime lightweighting 删除目标。如果要压 repo 体积，另开“reference packaging”任务。

### 可执行清理命令

这些命令只清缓存，不删源码：

```bash
rm -rf scripts/router-rs/target rust_tools/target scripts/skill-compiler-rs/target
rm -rf tools/browser-mcp/node_modules
```

执行前提：

- 当前没有正在跑的 Cargo 或 npm 任务。
- 用户接受下一次构建需要重新编译。

### 验收

```bash
du -sh scripts/router-rs/target rust_tools/target tools/browser-mcp/node_modules 2>/dev/null || true
cargo test --manifest-path scripts/router-rs/Cargo.toml routing_eval_report_matches_expected_baseline --quiet
```

通过标准：

- 删除的是 cache/dependency install，不是源码。
- Rust live path 仍能重新构建。
- Browser MCP live path 不依赖 Node fallback。

## 12. P7：profile contract 分成 kernel 和 capability

### 问题

`docs/framework_profile_contract.md` 仍展示大量 canonical fields，包括 approval、loadout、tool、mcp、delegation、supervisor 等。功能上没错，但默认阅读路径太重。

### 修改

1. 文档默认只讲 `kernel_profile`：
   - `routing`
   - `runtime_protocol`
   - `memory`
   - `continuity`
   - `codex_host_payload`
2. 其他字段改成 `capability_profile`：
   - `approval_policy`
   - `loadout_policy`
   - `tool_policy`
   - `artifact_contract`
   - `delegation_contract`
   - `supervisor_state_contract`
   - `mcp_servers`
3. JSON artifact 可以保持兼容字段，但文档和默认 mental model 不再把它们当 kernel。
4. `execution_controller_contract` 继续标注 compatibility projection only。

### 验收

```bash
rg -n 'kernel_profile|capability_profile|execution_controller_contract|compatibility projection' docs/framework_profile_contract.md scripts/router-rs/src/framework_profile.rs
cargo test --manifest-path scripts/router-rs/Cargo.toml framework_profile --quiet
```

通过标准：

- 生成 artifact 不破坏旧消费者。
- 默认文档从“大而全 profile”变成“小 kernel + 显式 capability”。

## 13. 不建议现在删除的东西

| 项 | 原因 | 后续条件 |
|---|---|---|
| 13 个 core gates | 当前已合理，直接删会损害 artifact/source/evidence routing | 有测试证明某 gate 不该默认才动 |
| `SKILL_MANIFEST.json` | full search 和显式 fallback 需要它 | 只能收紧 route fallback，不能删 full manifest |
| `SKILL_APPROVAL_POLICY.json` | docs 表示 middleware 可能消费 | 先完成消费者审计 |
| `SKILL_SHADOW_MAP.json` | 可能是 shadow/duplicate 诊断唯一来源 | 先移动到 debug output |
| `RUNTIME_REGISTRY.json` | alias、explicit command、host entrypoint 真源 | 不删，只可拆分或瘦字段 |
| retired flag migration table | 当前是 fail-fast 保护 | stale docs/callers 清完后再缩短 |
| TypeScript Browser MCP | dev/parity harness | Rust live path 完整覆盖并迁移测试后再删 |
| Cloudflare references | 不在 runtime 热路径 | 只在 repo 体积治理时考虑外置 |

## 14. 第一批建议实际改动

第一批只做三件事，收益最大，风险最小：

1. P0 route fallback guard 和两条 runtime-lightweighting regression。
2. P1 清掉 `RTK.md`、`docs/rust_contracts.md`、`docs/deerflow2_runtime_benchmark.md` 等处对用户可见的 retired flag 文案，并移除已退役 continuity refresh skill 包。
3. P2 给 surface/loadout/tier 写清真源关系，先不删除生成物。

不建议第一批就做：

- 不直接删 `SKILL_APPROVAL_POLICY.json`。
- 不直接删 `SKILL_SHADOW_MAP.json`。
- 不直接删 Browser MCP TypeScript。
- 不做大规模 `main.rs` 搬迁和行为修改混合提交。

## 15. 总验收清单

每个阶段都至少跑对应定向测试。完整收口时跑：

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
cargo test --test policy_contracts --quiet
cargo run --manifest-path scripts/router-rs/Cargo.toml -- --help
# （可选）对用户文档定向检索已删除顶层 flags：以 policy_contracts 中 removed_router_flags 列表为准
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml route '核查我现在的runtime，什么部分是没有用的或者加重负担的，值得去掉的？'
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml route '继续深度诊断，写更周密的可执行计划，我希望更加轻量化，减少兼容层和胶水层，减少入口，不要损害功能'
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml route '后端 Rust 服务运行时 OOM 并且请求一直 hang，先诊断 traceback、deadlock 和资源泄漏'
```

成功标准：

- 前两个 route 都选 `skill-framework-developer`。
- backend runtime failure 选 `backend-runtime-debugging`。
- help 只展示 canonical subcommands。
- 用户文档不再宣传 retired top-level flags。
- generated surface 只有一个真源。
- `main.rs` 按阶段下降到 4k 行以内。
- runtime 默认路径不依赖 Node/Python live fallback。
- 所有被下沉的东西都有显式替代入口或生成物位置。

## 16. 回滚策略

| 改动 | 回滚方式 |
|---|---|
| route fallback guard | 回滚单个函数和 regression fixtures |
| trigger 增补 | 回滚对应 skill trigger 和重新编译 runtime artifacts |
| retired docs cleanup | 回滚文档即可，不影响 runtime |
| loadout/tier 真源合并 | 恢复 compiler `write_bundle()` 输出 |
| stdio op 模块迁移 | 因 op 名和 schema 不变，可逐模块回滚 |
| build cache 清理 | 重新运行 Cargo/npm install |

核心原则：

```text
先把默认路径变窄，再移动生成物，最后删除兼容层。
任何删除都必须先证明：功能仍有 canonical path，测试仍覆盖，文档不再教旧入口。
```
