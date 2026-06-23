# Contributing to Skill Framework

> 完整文档索引见 [`docs/README.md`](docs/README.md)（含文档地图、已合并文件记录）。

## 角色阅读路径

| 角色 | 推荐阅读顺序 |
|------|-------------|
| **框架开发者** | [`docs/README.md`](docs/README.md) → [`docs/adr/010-ideal-architecture-v10.md`](docs/adr/010-ideal-architecture-v10.md)（架构） → [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md)（宿主） → [`AGENTS.md`](AGENTS.md)（策略） |
| **Skill 作者** | [`README.md`](README.md) §系统包含内容 → `skills/<skill-name>/SKILL.md` 模板 → 本文件 §Skill 贡献 |
| **普通用户** | [`README.md`](README.md) → [`docs/operations/index.md`](docs/operations/index.md)（安装/升级） → [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md)（对应宿主节） |
| **宿主实现者** | [`docs/hosts/_common.md`](docs/hosts/_common.md) → [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md) → [`docs/hosts/opencode.md`](docs/hosts/opencode.md) |

## 开发环境

- **Rust**: stable (edition 2024, rust-version 1.85)
- **Python**: 3.12 (uv-only, 禁止 pip)
- **OS**: macOS / Linux / Windows (CI 覆盖 Ubuntu + macOS)

## Crate 目录导览

| Crate | 路径 | 职责 |
|-------|------|------|
| **router-rs** | `core/router-rs/` | CLI 入口，宿主 hook 分发、MCP server 管理 |
| **runtime-core** | `core/runtime-core/` | 运行层编排、session/context/loop 管理 |
| **host-projection** | `core/host-projection/` | 宿主适配层（Claude/Cursor/Codex/OpenCode hook 分发） |
| **core-policy** | `core/core-policy/` | 跨宿主策略（hook 公共逻辑、env flags） |
| **routing-engine** | `core/routing-engine/` | 路由引擎：意图匹配、Skill/Tool 路由 |
| **framework-kernel** | `core/framework-kernel/` | Skill 加载、RUNTIME_REGISTRY 解析 |
| **research-harness** | `core/research-harness/` | 科研日志、知识图谱、Barrier 系统 |
| **codegraph-rs** | `tools/codegraph-rs/` | 代码图谱索引工具（MCP 八工具） |
| **evolution-rs** | `tools/evolution-rs/` | 路由日志演化分析（零 crate 依赖） |

依赖方向：`router-rs` → `runtime-core` → `host-projection`, `core-policy`, `routing-engine`, `framework-kernel`。B0 crates（`core-policy`/`routing-engine`/`framework-kernel`）不依赖 `router-rs`。

## 注册表驱动架构

所有宿主元数据从 `configs/framework/RUNTIME_REGISTRY.json` 的 `host_targets.metadata.*` 生成。

### 添加新宿主

1. 在 `RUNTIME_REGISTRY.json` 的 `host_targets.supported` 数组中添加新的 host_id
2. 在 `host_targets.metadata` 中添加对应的完整字段（参考现有宿主）
3. 在 `host_targets.host_providers` 中添加 provider 声明（`dispatcher_type` 等）
4. 运行 `cargo build` — `framework-kernel/build.rs` 和 `host-projection/build.rs` 自动生成所有代码
5. 运行 `cargo test` — 合约测试验证注册表一致性

**零手动 Rust 代码变更** — 所有 provider struct、trait impl、常量和查找函数均从注册表自动生成。

## 快速开始

```bash
# 克隆仓库
git clone <repo-url> && cd skill

# 构建（默认 profile，不含 codegraph 特性）
cargo build --release --manifest-path core/router-rs/Cargo.toml

# 运行全量测试（常用三 crate）
cargo test -p router-rs -p codegraph-rs -p evolution-rs

# 格式化 + lint
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## 分支与提交

- 从 `main` 创建功能分支
- 提交信息使用英文，格式：`<type>: <description>`（type: feat/fix/refactor/docs/test/ci）
- 提交前确保：`cargo fmt` + `cargo clippy -- -D warnings` + `cargo test` 全通过

## 代码规范

- **Rust**: 遵循 clippy.toml 配置（too-many-arguments=8, cognitive-complexity=30）
- **错误处理**: 核心 crate 使用 `FrameworkError`（thiserror），工具 crate 可使用 anyhow
- **异步测试**: 优先使用 `#[tokio::test]`
- **热路径优化**: Regex 使用 `OnceLock`/`LazyLock` 缓存，避免裸 `Regex::new`

## 测试要求

- 新功能必须附带测试
- CI 运行全量 workspace 测试 + feature matrix（default/codegraph/host-*）
- 测试文件 ≤2000 行，超过请拆分
- 测试分层：单元测试（`#[cfg(test)]`） → 集成测试（`tests/`） → 合约测试（`smoke_*`）

## 文档

- 公共 API 必须有文档注释
- 完整文档体系见 [`docs/README.md`](docs/README.md)（含文档地图）
- 架构决策记录在 [`docs/adr/`](docs/adr/)
- 宿主差异文档在 [`docs/hosts/`](docs/hosts/)
- Skill 开发指南见 [`README.md`](README.md) §框架路径与路由

## Pull Request

1. 确保 CI 全绿
2. PR 描述说明变更内容和动机
3. 大型变更请先开 Issue 讨论

## License

贡献即表示你同意你的代码在 MIT License 下发布。

## 契约漂移规则

机器可读 Schema、状态流转图、指标定义是开发和测试的第一断言断点。配置规则变更须先改文档，再实现与回归。

## Skill 贡献

### 快速开始

1. 在 `skills/` 下创建目录：`skills/<slug>/`
2. 编写 `SKILL.md`（参考下方模板）
3. 在 `skills/SKILL_MANIFEST.json` 中注册
4. 运行 `router-rs framework skills refresh --write --write-companions` 同步 catalog/tiers
5. 运行 `python3 scripts/validate-manifest.py` 确认全绿
6. 提交 PR

### SKILL.md 模板

```markdown
---
name: my-new-skill
description: |
  一句话描述技能做什么。可多行。
routing_layer: L3
routing_owner: none
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: true
disable-model-invocation: false
risk: low
source: local
trigger_hints:
  - 中文触发词
  - english trigger phrase
  - /slash-command
---

# my-new-skill

## 触发条件
明确描述何时激活此 skill。

## 执行指令
Claude 执行的具体步骤。

## Do not use
明确列出不应使用此 skill 的场景。

## References
- [参考文档](../skills/SKILL_ROUTING_LAYERS.md)
```

### Frontmatter 字段说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `name` | 是 | kebab-case 唯一标识符（原 `slug`） |
| `description` | 是 | 多行描述技能功能和触发语义 |
| `routing_layer` | 是 | L0/L1/L2/L3/L4（原 `layer`） |
| `routing_owner` | 是 | gate/none/evidence/artifact/delegation/source |
| `routing_gate` | 是 | 门控类型（delegation/source/artifact/evidence/none） |
| `routing_gate_evidence` | 否 | 门控所需的证据描述 |
| `routing_priority` | L0 必填 | P1/P2 |
| `session_start` | 是 | required/preferred/n/a |
| `user-invocable` | 是 | true/false — 用户是否可手动调用 |
| `disable-model-invocation` | 是 | true/false — 禁止模型自动调用 |
| `risk` | 否 | low/medium/high — skill 风险等级 |
| `source` | 否 | local/external — skill 来源 |
| `trigger_hints` | 是 | 中英文触发词 + /slash-command 列表 |
| `metadata` | 否 | 扩展 JSON 块（键值对） |

### 分层规范

| 层 | 定位 | gate | 典型 priority |
|----|------|------|---------------|
| L0 | 框架内核/路由/gate | delegation/source | P1-P2 |
| L1 | 核心方法论 | 混合 | P1 |
| L2 | 技术底座 | 混合 | P1-P2 |
| L3 | 平台/工具/产物 | artifact/evidence | P1-P2 |
| L4 | 高语义专业领域 | none | P2 |

### 三层元数据同步

本仓库有三层元数据，修改时需保持一致：

1. **SKILL_MANIFEST.json** — 源头数据（手动编辑）
2. **SKILL_ROUTING_RUNTIME.json** — 路由表（手动编辑或 `router-rs` 生成）
3. **SKILL_PLUGIN_CATALOG.json** — 插件目录（自动生成，勿手动编辑）

**同步流程**：
```bash
router-rs framework skills refresh --write --write-companions
python3 scripts/validate-manifest.py
```

### 校验规则

运行 `python3 scripts/validate-manifest.py` 检查：

| 规则 | 说明 |
|------|------|
| R1 | host_platforms 无重复（manifest + routing） |
| R2 | routing skill_path 文件存在性 |
| R3 | 冷热一致性（kind=cold 不在 routing） |
| R4 | L0 gate skill 必须有 priority |
| R5 | manifest 与 catalog slug 集合一致 |
| R6 | catalog host_support.platforms 无重复 |
| R7 | catalog skill_path 文件存在性 |
| R8 | trigger_hints 跨 skill 重叠检测（INFO 级别） |

### CI 管线

| 脚本 | 功能 |
|------|------|
| `scripts/validate-manifest.py` | 8 条元数据校验规则 |
| `scripts/ci/check-routing-regression.sh` | 路由回归测试（accuracy >= 0.95） |
| `scripts/ci/check-skills-no-operator-pip.sh` | 禁止 pip install（用 uv） |
| `scripts/ci/check-cursor-hooks-parity.sh` | Cursor hooks 一致性 |

### References 目录

大型 skill 应将详细指南放入 `references/` 子目录，实现 deferred loading：

```
skills/my-skill/
  SKILL.md          # 主文件（精简，<15KB）
  references/
    guide.md        # 详细指南（按需加载）
    checklist.md    # 检查清单
```

主文件通过 [`SKILL_ROUTING_LAYERS.md`](skills/SKILL_ROUTING_LAYERS.md) 引用扩展参考，Claude 仅在需要时读取。

### 改 Skill 必查

- 触发词是否变化 → 更新 description
- 边界是否变化 → 同步改 `SKILL_ROUTING_RUNTIME.json` / `SKILL_MANIFEST.json`，再 `framework skills refresh --write --write-companions`
- 是否引入第二份 live source → 删除多余副本
- 是否需要刷新 Claude 可见入口 → 运行 `codex host-integration install-skills --repo-root "$PWD" install`

### 边界重叠处理

默认 **incumbent-first**：优先修改旧 skill。仅当 owner/gate/overlay 角色变化、运行时差异明显、或旧 skill 触发精度严重受损时才新建。

### Description 写法

```
[角色] + [领域名词] + [用户自然说法] + [边界词]
```

- 第一行 brief：≤ 120 chars
- 整体推荐：180–450 chars，> 600 chars 视为偏重
- 覆盖用户真实说法（中英混合）
- session_start 为 required/preferred 时，必须包含 "每轮对话开始 / first-turn / conversation start"

### Git 安全基线

该仓库高频变更且可能同时存在多个 worktree。做清理、切分提交、rebase、stash 前，先运行：

```bash
git status --short --branch
git diff --stat
git worktree list --porcelain
```
