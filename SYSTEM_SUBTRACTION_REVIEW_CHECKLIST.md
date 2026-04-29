# 减法系统与第一性原理 Review Checklist

状态：已完成静态系统 review
范围：Codex-only 前提下，审查路由、runtime、host 集成、memory、artifact、browser MCP、skill surface 与入口形态
原则：只保留当前 Codex 真正需要的路径；兼容、桥接、fallback、历史迁移与多入口只在有现役调用证据时保留

## 0. 第一性原理结论

- [x] 明确系统核心目标：把用户请求路由到最小必要 skill，并让 Codex 能稳定执行、记录、恢复。
- [x] 明确 steady-state 主链路：`AGENTS.md` -> `skills/SKILL_ROUTING_RUNTIME.json` -> `skills/<name>/SKILL.md` -> Codex 工具执行。
- [x] 明确当前最大问题：系统不是缺能力，而是保留了过多控制面、兼容面、迁移面和自描述面。
- [x] 明确减法方向：默认面只保留 routing、Codex host、必要 continuity、必要 artifact；其余作为显式工具或历史文档。
- [x] 明确 Codex-only 前提：不再为非 Codex host、旧 Python、桌面/CLI 多 adapter parity 保留默认代码路径。

## 1. 高优先级问题

- [x] `scripts/router-rs/src/main.rs` 的 `Cli` 已膨胀到 60+ flags，入口语义混杂了 routing、browser MCP、trace、storage、profile、host integration、hooks、memory、alias、observability。
- [x] `scripts/router-rs/src/main.rs` 的 `main()` 是长串 `if args.xxx` 调度器，缺少一级命令边界，新增能力只能继续加 flag 和分支。
- [x] `scripts/router-rs/src/runtime_storage.rs` 同时支持 filesystem、sqlite、memory 三套 backend，但默认策略仍是 filesystem，sqlite/legacy key 逻辑仍在主路径。
- [x] `scripts/router-rs/src/host_integration.rs` 仍保留 install/status/remove/migration/bootstrap 多职责，已超出 Codex-only 最小安装器。
- [x] `scripts/router-rs/src/framework_profile.rs` 仍输出 `codex_adapter` 与 `host_adapter_payload`，虽然文档说这不是多 host adapter，但命名和结构仍在保留旧抽象。
- [x] `tools/browser-mcp/src/runtime.ts` 自带 RouterRs stdio client pool，会在 TypeScript 侧再维护一套 router-rs 进程管理。
- [x] `skills/SKILL_LOADOUTS.json` 与 `skills/SKILL_TIERS.json` 描述了 default/explicit/core/optional/experimental/deprecated，但运行时主路由主要仍从 runtime JSON 直接加载，surface 策略更像旁路文档。

## 2. 过度抽象层

- [x] `profile -> codex_adapter -> host_adapter_payload` 过度抽象：Codex-only 时可以直接叫 `codex_profile` 或 `codex_host_payload`，不需要 adapter 叙事。
- [x] `workspace_bootstrap.bridges.skills/memory` 过度抽象：当前只有 Codex skills 和 project memory，`bridge` 名称制造了“还有其他 host 需要投影”的错觉。
- [x] `framework_native_aliases` 过度抽象：`autopilot`、`team`、`deepinterview` 实际是入口/模式，不应同时以 skill、alias、host entrypoint、runtime state machine 多重身份出现。
- [x] `control_plane_contracts_json` 过度抽象：大量 contract descriptor 是自描述控制面，不在默认执行链路中产生直接价值。
- [x] `runtime backend family parity` 过度抽象：filesystem/sqlite/memory parity 更适合测试或迁移工具，不应常驻默认 runtime。
- [x] `framework_surface_policy + loadouts + tiers` 过度抽象：三份文件表达类似激活策略，建议压缩成一个 runtime-consumed policy 或降级为文档。
- [x] `session_supervisor + framework_runtime + host_integration` 对 continuity/artifact 的职责重叠：都在描述状态、恢复、迁移或生命周期。

## 3. 仍保留的胶水层和兼容层

- [x] `runtime_storage.rs` 的 sqlite legacy key 读取仍保留：`stable_key OR legacy_key`。
- [x] `runtime_storage.rs` 的 `coerce_legacy_service_delegate_kind()` 仍保留旧 delegate 兼容。
- [x] `host_integration.rs` 的 legacy artifact migration 仍保留：`PlanLegacyArtifactRoots`、`MigrateLegacyArtifactRoots`、`artifacts/memory_automation`、`tmp-*` 迁移。
- [x] `framework_runtime.rs` 的 legacy memory archive 仍保留：`MEMORY_AUTO.md` 与 `sessions/` 迁移。
- [x] `framework_runtime.rs` 的 supervisor fallback route 仍保留：trace 不匹配时回落到 supervisor controller 字段。
- [x] `framework_profile.rs` 的 `compatibility_rules.python_may_continue_to_author = true` 与当前“Rust 真源，不恢复 Python”目标冲突。
- [x] `codex_hooks.rs` 不再维护非 Codex 旧 entrypoint 清理面。
- [x] `.codex/hooks.json` 不再由 host-entrypoint sync 生成，`.codex/config.toml` 保持 `codex_hooks = false`，默认不安装/启用 hook。
- [x] `tools/browser-mcp/scripts/start_browser_mcp.sh` 是 shell wrapper，实际只是转发到 router-rs 的 `--browser-mcp-stdio`。
- [x] `tools/browser-mcp/src/index.ts` 仍支持 HTTP transport；如果只在 Codex MCP stdio 下使用，可降为开发调试路径或删除。

## 4. 可以合并的入口

- [x] 合并 `--json`、`--route-json`、`--route-policy-json`、`--route-snapshot-json`、`--route-report-json`、`--route-resolution-json` 到 `router route|search|eval|report` 子命令。
- [x] 合并 `--framework-runtime-snapshot-json`、`--framework-memory-recall-json`、`--framework-session-artifact-write-json`、`--framework-refresh-json`、`--framework-alias-json` 到 `router framework ...` 子命令。
- [x] 合并 trace/checkpoint/attached-event flags 到 `router trace ...` 子命令。
- [x] 合并 storage/backend/checkpoint-control-plane flags 到 `router storage ...` 子命令。
- [x] 合并 `--host-integration <subcommand>` 与 `--sync-host-entrypoints-json`、`--codex-hook-projection-json`、`--codex-hook-command` 到 `router codex ...` 子命令。
- [x] 合并 browser MCP wrapper：Codex 配置直接调用 `router-rs browser-mcp stdio`，删除 `start_browser_mcp.sh` 或只保留开发 shim。
- [x] 合并 `autopilot/team/deepinterview`：从 skill + alias + registry 三层，收敛为一种入口模型。推荐作为 Codex command mode，而不是普通 skill。

## 5. 建议删除或降级为历史文档

- [x] 删除默认路径中的 non-Codex 旧路径清理逻辑，只保留一次性迁移脚本或历史说明。
- [x] 删除 `python_may_continue_to_author` 兼容声明，改成 `rust_only_authority = true`。
- [x] 删除 sqlite legacy absolute-key fallback，若仍担心旧数据，先提供一次性 migration command。
- [x] 删除 memory `MEMORY_AUTO.md`/`sessions` 自动 archive 主路径，改成显式 `router migrate legacy-memory`。
- [x] 删除 `template_root` 参数，当前 `InstallNativeIntegration` 接收但不使用。
- [x] 删除或隐藏 `profile_artifacts_json` 的 adapter 命名，Codex-only 下不再暴露 adapter artifact 叙事。
- [x] 删除 `ROUTER_BIN_EXPLICIT`，当前 `start_browser_mcp.sh` 设置后未使用。
- [x] 删除 default loadout 里的语言 owner 预设，Codex 实际可以通过路由 query 选择 `python-pro` / `typescript-pro`，默认面无需硬编码。

## 6. 建议保留的底层能力

- [x] 保留 `skills/SKILL_ROUTING_RUNTIME.json` 作为首轮路由索引，符合 AGENTS 策略且便宜。
- [x] 保留 `scripts/skill-compiler-rs`，因为它负责生成 runtime registry 和 health manifest。
- [x] 保留 `router-rs` 的核心路由 scorer，但应拆出 CLI 控制面。
- [x] 保留 Codex hook 投影能力，但默认只生成一个清晰入口，不再同时维护多 host 历史 inventory。
- [x] 保留 artifacts/current 的 active/focus/task registry，但减少镜像和历史迁移自动逻辑。
- [x] 保留 browser MCP 的真实浏览器能力，但把 TypeScript 侧 router-rs 进程池视为可疑重复层。

## 7. 推荐执行顺序

- [x] 第一步：冻结新抽象。禁止再新增 `--xxx-json` 平级 flag，所有新增入口必须进子命令。
- [x] 第二步：把 `router-rs main.rs` 的大 `Cli` 拆成 `RouteCommand`、`FrameworkCommand`、`CodexCommand`、`TraceCommand`、`StorageCommand`、`BrowserCommand`。
- [x] 第三步：把旧迁移逻辑从默认 runtime 路径挪到 `router migrate ...`。
- [x] 第四步：把 `codex_adapter` 命名收敛成 `codex_profile`，仅在输出兼容层保留旧 key 一版。
- [x] 第五步：把 `autopilot/team/deepinterview` 从普通 skill 和 alias registry 中抽出来，明确成 Codex command mode。
- [x] 第六步：精简 runtime storage，只保留 filesystem 默认；sqlite 作为显式 opt-in，memory 只在测试启用。
- [x] 第七步：清理 `.codex/hooks.json` 与 `.codex/config.toml` 的不一致，采用“不生成 hook + 强制关闭 codex_hooks”的方向。
- [x] 第八步：重新生成 `skills/SKILL_ROUTING_RUNTIME.json`、`SKILL_TIERS.json`、`SKILL_LOADOUTS.json`，并跑 routing eval。

## 8. 验收标准

- [x] 默认入口数量减少：普通使用只需要 `route/search`、`framework recall/write`、`codex sync/install` 三类。
- [x] 默认 steady-state 不再执行 legacy migration、旧 host cleanup、Python compatibility 或 multi-host adapter projection。
- [x] `router-rs --help` 不再展示几十个平级 JSON flags。
- [x] `codex_adapter` 兼容 key 不再作为内部结构名传播。
- [x] `autopilot/team/deepinterview` 只有一个权威入口来源。
- [x] browser MCP 不再同时维护 shell wrapper、Rust stdio、TypeScript router-rs stdio pool 三层进程控制。
- [x] 路由首轮仍只读 `skills/SKILL_ROUTING_RUNTIME.json` 和命中 skill 的 `SKILL.md`。

## 9. 执行记录

- [x] 已把 `router-rs` 平级 JSON flag 主入口收敛到 `route/search/framework/codex/trace/storage/browser/profile/migrate` 子命令；`--help` 默认只展示一级命令。
- [x] 已从默认 steady-state 移除 legacy memory 自动 archive、legacy artifact root 自动迁移、旧 host path 默认清理、sqlite legacy absolute-key fallback、delegate kind legacy coercion。
- [x] 已把 `codex_adapter` / `host_adapter_payload` 内部命名收敛为 `codex_profile` / `codex_host_payload`，并把 `python_may_continue_to_author` 改为 Rust-only authority 规则。
- [x] 已把 browser MCP wrapper 改为 `router-rs browser mcp-stdio` dev shim，并移除 TypeScript HTTP transport 入口。
- [x] 已修正 `.codex/config.toml` 与 `.codex/hooks.json` 的 hook 启用状态不一致：默认关闭 hook，sync manifest 不再管理 `.codex/hooks.json`。
- [x] 已验证：`cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet`、`cargo test --quiet`、`cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet --no-run`、`cargo test --quiet --no-run`、`cd tools/browser-mcp && npm test -- --run`。
