---
name: update
description: |
  仓库维护强制入口：`/update` 一条龙刷新生成物，并以**全量契约测试 + git 跟踪 Markdown 纳管**收口；测试失败即阻断，需改文档/真源直至绿。
  维护面已 **Rust 化为 `router-rs framework maint`**（无 `scripts/*.sh` 包装）。
  可选 `ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS` 将 skill 投影二次写入本机 Codex/Cursor home（全局同步宿主面，不手改 ~/.codex/skills）。
  Use only when the user explicitly invokes `/update`；不要与普通依赖升级混用。
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: false
disable-model-invocation: true
short_description: Rust maint entrypoints + full codegen + all contract tests + optional host publish.
trigger_hints:
  - /update
  - 一口气更新
  - 刷新文档
  - 扫描文档
  - 文档契约
  - registry 更新
  - 同步投影
  - refresh host projections
  - 全局同步
allowed_tools:
  - shell
  - git
approval_required_tools:
  - git push
metadata:
  version: "2.1.0"
  platforms: [codex, cursor]
  tags: [maintenance, docs, contracts, projections, skill-compiler, router-rs]
risk: low
source: project
filesystem_scope:
  - repo
network_access: local
---

# update

`update` 是本仓库维护用的**显式强制入口**。**推荐显式写法：`/update`**（对齐 `/gitx`）。

## 强制执行模型（「所有文档都被检查并驱动修改」）

1. **生成面**：宿主投影 + `skill-compiler-rs --apply` **直接改** tracked 生成物（`skills/SKILL_*`、`SKILL_ROUTING_*.md`、`AGENTS.md` / `.codex/` 等，以 `GENERATED_ARTIFACTS.json` 为准）。
2. **叙事与技能文档**：`docs/**`、`skills/**/*.md`、根 `AGENTS.md` / `README.md` / `RTK.md` 等 **不静默机改正文**；全部由测试与 drift 门禁 **失败即阻断**，你必须按失败信息做最小 diff 修复后再跑通——这就是「检查驱动修改」的工程含义。
3. **.harness（默认可复现 / 离线优先）**：`update-one-shot` 顺序跑 `policy_contracts`、`documentation_contracts`、`tracked_markdown_utf8_contract`、`rust_cli_tools`、`host_integration`、`browser_mcp_scripts`、`codex_aggregator_rustification`，再跑 `skill-compiler-rs` crate tests；**任一失败则整条 `/update` 视为未完成**。依赖外网 arXiv 的 `autoresearch_cli` **默认跳过**；需纳入时设 **`ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS=1`**。若你要无差别 `cargo test`（含所有套件），在仓库根自行执行 `cargo test`。
4. **全局宿主同步（可选）**：在通过上述测试后，若需把投影写回**本机** `CODEX_HOME` / `CURSOR_HOME`，设 `ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1`（见下）；**不要**手改 `~/.codex/skills` 正文（见 `skills/SKILL_MAINTENANCE_GUIDE.md`）。

## When to use

- 用户 **`/update`** 或明确要**全量维护 / registry 更新 / 同步投影 / 文档契约清零**
- 改了 `AGENTS.md`、`.cursor/rules/*`、`configs/framework/*.json`、`skills/*`、`docs/*`、`router-rs` 任一，需要**确定性一条龙**收口

## Do not use

- 单一功能修 bug、且不需要刷新路由/投影 → 按普通开发流
- Git 合并/推送收口 → **`/gitx`**
- 只改单个 skill 文案且不刷 registry → **`$skill-creator`**

## One-shot（真源，Rust）

首选（`router-rs` **已在 PATH**，与 cwd 无关）：

```bash
router-rs framework maint update-one-shot
```

**cwd 落在框架仓库外时**（例如只对子项目开了终端）：`router-rs`/宿主子命令会在 `SKILL_FRAMEWORK_ROOT` 与自 `cwd` 向上探测之后，再尝试 **`ROUTER_RS_CURSOR_WORKSPACE_ROOT` / `CURSOR_WORKSPACE_ROOT`**（经路径归一化并校验 `configs/framework/RUNTIME_REGISTRY.json` + `scripts/router-rs/Cargo.toml`），最后从 **`std::env::current_exe()`** 向上探测同一套标记。你在本机手敲命令时，仍建议显式导出框架根变量，减少歧义：

- **`SKILL_FRAMEWORK_ROOT`**：框架 skill 仓库根的绝对路径（推荐真源）。
- **`CURSOR_WORKSPACE_ROOT`**：Cursor 单根工作区打开该仓库时，通常与上面相同；未使用 Cursor 时可忽略。

未安装 `router-rs` 时，用 `cargo run`（manifest 必须解析到框架仓库内的 `Cargo.toml`）：

```bash
export SKILL_FRAMEWORK_ROOT=/abs/path/to/framework-repo
# 若适用：export CURSOR_WORKSPACE_ROOT=/abs/path/to/framework-repo

cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint update-one-shot
```

等价于：`router-rs framework maint refresh-host-projections` → `skill-compiler-rs/Cargo.toml` `--apply` → **上述 integration 套件** → `skill-compiler-rs` crate tests → `generated-artifacts-status` OK →（可选）`install-skills install`。（未装 `router-rs` 到 PATH 时用上面 `cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" --` 前缀。）

### 可选：会话级隔离 homes（不写用户目录）

```bash
eval "$(cargo run --quiet --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint print-local-homes)"
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint update-one-shot
```

### 可选：把 `autoresearch_cli`（外网）纳入一条龙

```bash
export ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS=1
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint update-one-shot
```

### 可选：全局宿主投影（Codex + Cursor）

在**已通过全部测试**的前提下：

```bash
export ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint update-one-shot
```

或仅发布（等价于一条龙末尾条件块）：

```bash
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- \
  codex host-integration install-skills \
  --repo-root "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}" \
  --artifact-root "${ARTIFACT_ROOT:-${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/artifacts}" \
  --codex-home "${CODEX_HOME:-$HOME/.codex}" \
  --cursor-home "${CURSOR_HOME:-$HOME/.cursor}" \
  install
```

### 单片维护子命令（调试）

```bash
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint refresh-host-projections
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint verify-cursor-hooks
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint verify-codex-hooks
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint clean-rust-targets
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint install-codex-user-hooks
```

## 分步手写（不推荐；顺序与 maint 对齐）

若需徒手拆阶段：

```bash
cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- framework maint refresh-host-projections

cargo run --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/skill-compiler-rs/Cargo.toml" -- \
  --skills-root "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/skills" \
  --source-manifest "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/skills/SKILL_SOURCE_MANIFEST.json" \
  --apply

# 默认与 update-one-shot 一致的离线套件：
cargo test --test policy_contracts
cargo test --test documentation_contracts
cargo test --test tracked_markdown_utf8_contract
cargo test --test rust_cli_tools
cargo test --test host_integration
cargo test --test browser_mcp_scripts
cargo test --test codex_aggregator_rustification
# 可选全量（含外网 autoresearch_cli）：cargo test
cargo test --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/skill-compiler-rs/Cargo.toml"

cargo run --quiet --manifest-path "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/scripts/router-rs/Cargo.toml" -- \
  framework host-integration generated-artifacts-status \
  --framework-root "${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}" \
  --artifact-root "${ARTIFACT_ROOT:-${SKILL_FRAMEWORK_ROOT:-$CURSOR_WORKSPACE_ROOT}/artifacts}" \
  | python3 -c 'import json,sys; j=json.load(sys.stdin); sys.exit(0 if j.get("ok") is True else 1)'
```

## Expected outputs

- 生成物与 `configs/framework/GENERATED_ARTIFACTS.json` 一致（`generated-artifacts-status` `ok: true`）
- **默认套件全绿** = 文档/策略/hook 契约与生成面已纳入 harness；失败项即待修改清单（外网套件见上）
- `tracked_markdown_utf8_contract`：git 跟踪的 `docs/`、`skills/`、根契约 Markdown **全部可读 UTF-8**

