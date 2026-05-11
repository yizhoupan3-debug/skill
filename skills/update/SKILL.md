---
name: update
description: |
  仓库知识状态与卫生维护入口：`/update` 用于更新关键文档、科研文档、git 跟踪面，并定位/清除有证据支持的旧代码、旧文件、旧文档和历史残留。
  现有 framework 生成物刷新与契约测试仍保留为本 skill 框架仓库的专用验证子阶段，而不是 `/update` 的全部语义。
  Use only when the user explicitly invokes `/update`；不要与普通依赖升级、Git 收口或单点功能修复混用。
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: false
disable-model-invocation: true
short_description: Refresh key docs, git tracking, and stale/dead repo surfaces; framework checks when applicable.
trigger_hints:
  - /update
  - 一口气更新
  - 更新关键文档
  - 科研文档更新
  - 刷新文档
  - 扫描文档
  - git 跟踪文件
  - git tracking
  - 死代码清理
  - 旧文档清理
  - stale files
  - dead code
  - registry 更新
  - 同步投影
allowed_tools:
  - shell
  - git
approval_required_tools:
  - git push
metadata:
  version: "3.0.0"
  platforms: [codex, cursor]
  tags: [maintenance, docs, research-docs, git-tracking, cleanup, contracts, router-rs]
risk: medium
source: project
filesystem_scope:
  - repo
network_access: local
---

# update

`update` 是仓库维护用的**显式强制入口**。**推荐显式写法：`/update`**（对齐 `/gitx`）。

它的主目标是让仓库的知识状态保持真实、可追踪、可继续推进：关键文档要反映当前事实，科研材料要被纳管或标注状态，git 跟踪面要干净，旧代码/旧文件/旧文档要被证据驱动地清理。

## 执行模型

1. **document refresh**：读取 README、AGENTS、docs 索引、计划/研究目录、实验记录和 artifact 指针；更新关键说明、状态索引、研究计划、文献综述、方法说明、结果表、论文草稿或实验记录中已经过期的部分。
2. **git tracking audit**：检查已跟踪、未跟踪、被 ignore 但可能应纳管、已跟踪但像生成物/缓存/临时物的文件；给出 add/ignore/remove/migrate 建议。
3. **stale/dead inventory**：定位疑似死代码、死文件、过期文档、重复入口和历史残留；优先用引用搜索、编译/测试、文档索引、修改历史和命名线索交叉确认。
4. **cleanup + verification**：只清除有证据支持的对象；不确定项写入待确认清单。收口必须给出测试、检查、diff、生成物状态或明确 blocker。

## 科研文档是一等维护对象

`/update` 不只维护代码文档。以下材料都应进入关键文档刷新和 git 跟踪审计：

- 论文草稿、rebuttal、cover letter、实验记录、研究计划、计划 todo
- 文献综述、引用清单、方法说明、统计分析说明、结果表、figure/table 说明
- 数据/模型/实验 artifact 的索引、复现说明、环境说明、状态 ledger

删除科研材料时默认更保守：无法证明废弃的原始数据、手稿、引用库、实验记录和中间结论不直接删除；优先归档、标注状态或列为待确认。

## When to use

- 用户显式调用 **`/update`**
- 需要更新关键文档、科研文档、docs 索引、计划状态或 artifact 索引
- 需要检查 git 跟踪面：未跟踪文件、误跟踪生成物、ignore 漂移、tracked Markdown 纳管
- 需要定位并清理旧代码、旧文件、旧文档、重复入口或历史残留
- 改了 `AGENTS.md`、`.cursor/rules/*`、`configs/framework/*.json`、`skills/*`、`docs/*`、`router-rs` 任一，并需要框架生成物/契约测试一起收口

## Do not use

- 普通依赖升级或包版本升级
- 单一功能修 bug 且不涉及仓库知识状态或卫生维护
- Git commit、branch、merge、push 收口：用 **`/gitx`**
- 只改单个 skill 文案且不刷新 registry：用 **`$skill-creator`**

## Rust audit entrypoint

`update-audit` 是 dry-run 清单入口，只读审计，不删除、不改文件。**在框架仓内优先使用源码入口**；只有当 `router-rs framework --help` 已显示 `maint` 时，才直接用已安装二进制：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint update-audit
```

它输出 JSON，至少包含：

- `key_document_candidates`
- `git_tracking`
- `suspected_dead_code_markers`
- `suspected_stale_docs`
- `suspected_retired_files`
- `recommended_actions`

仓库外 cwd 时建议显式传目标仓库根；该仓库只需是 git repo，不要求是 skill framework checkout：

```bash
cargo run --manifest-path /abs/path/to/framework-repo/scripts/router-rs/Cargo.toml -- framework maint update-audit --repo-root /abs/path/to/repo
```

`--framework-root` 仍作为旧脚本兼容别名保留，但不再代表 audit 只能跑在 framework 仓库。

## Framework-specific validation

本 skill 框架仓库仍保留原有生成物刷新与契约测试；它是 `/update` 的验证子阶段，而不是全部定义。

完整框架一条龙仍可运行：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint update-one-shot
```

等价于：`refresh-host-projections` → `skill-compiler-rs --apply` → 默认离线契约测试 → `skill-compiler-rs` crate tests → `generated-artifacts-status` OK → 可选 host skill publish。

默认离线套件包括：

```bash
cargo test --test policy_contracts
cargo test --test documentation_contracts
cargo test --test tracked_markdown_utf8_contract
cargo test --test rust_cli_tools
cargo test --test host_integration
cargo test --test browser_mcp_scripts
cargo test --test codex_aggregator_rustification
cargo test --manifest-path scripts/skill-compiler-rs/Cargo.toml
```

可选外网套件：

```bash
ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS=1 cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint update-one-shot
```

可选全局宿主投影：

```bash
ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1 cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint update-one-shot
```

## 删除策略

- 可以删除：明确无引用、已被替代、测试覆盖确认不再需要的代码/文件/文档。
- 不直接删除：无法证明废弃的科研资料、实验记录、原始数据、手稿、引用库。
- 不确定项：写入待确认清单，说明证据缺口和建议下一步。

## Expected outputs

- 关键文档和科研文档状态与当前仓库事实一致
- git 跟踪面建议清晰：应纳管、应忽略、应删除、应迁移
- stale/dead inventory 有证据链，不把猜测当事实
- 框架仓库中，生成物与 `configs/framework/GENERATED_ARTIFACTS.json` 一致，相关契约测试通过
