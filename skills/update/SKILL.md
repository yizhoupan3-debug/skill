---

allowed_tools:
- shell
- git
approval_required_tools:
- git push
description: Refresh key docs, git tracking, and stale/dead repo surfaces.
metadata:
  platforms:
  - supported
  tags:
  - maintenance
  - docs
  - git-tracking
  - cleanup
  name: update
  scene: general
  network_access: local
  risk: medium
  routing_layer: L0
  source: runtime
  version: '3.1.0'
name: update
routing_gate: none
routing_owner: owner
routing_priority: P1
session_start: n/a
short_description: 仓库知识状态维护：文档刷新 → git 跟踪审计 → 旧代码/旧文档清理。
trigger_hints:
- /update
- 更新文档
- 刷新文档
- 文档同步
- git tracking
- 死代码清理
- 旧文档清理
- 仓库卫生
---
# update

`update` 是仓库知识状态维护的通用入口。**推荐写法：`/update`**。

它的主目标：让仓库的知识状态保持真实、可追踪、可继续推进。关键文档反映当前事实，git 跟踪面干净，旧代码/旧文件/旧文档有证据地清理。

## Quick Start

```markdown
# 在任意 git 仓库根目录键入：
/update

# 你会看到 3 阶段输出：
▸ 阶段 1：扫描 — README、AGENTS、docs/、研究目录、未跟踪文件、死代码标记
▸ 阶段 2：核查 — 引用搜索 / 编译 / 测试交叉验证
▸ 阶段 3：清理 — 展示发现，逐项确认后执行
```

---

## 执行模型（3 阶段）

每一阶段标注交互模式：

| 标记 | 含义 |
|------|------|
| ✅ auto | 自动执行，不暂停 |
| ⚠️ confirm | 展示给用户，确认后才执行 |
| 📋 report-only | 只报告，不执行 |

### 阶段 1：扫描发现（✅ auto）

扫描仓库关键知识面：

- **关键文档**：README、AGENTS.md、docs 索引、计划/研究目录、实验记录、artifact 指针
- **git 跟踪面**：未跟踪文件、误跟踪的生成物/缓存、.gitignore 漂移、应纳管的 tracked Markdown
- **旧代码/旧文件/旧文档**：疑似死代码、死文件、过期文档、重复入口、历史残留
- **科研材料**：论文草稿、rebuttal、cover letter、实验数据、研究计划、文献综述、引用库、环境/复现说明、结果表、figure/table 说明、状态 ledger

不推测，不做判断——只列出候选。

### 阶段 2：证据核查与分类（⚠️ confirm / 📋 report-only）

对阶段 1 的候选列表做**交叉确认**——用引用搜索、编译/测试结果、文档索引、git 修改历史、文件命名线索来验证每一种猜测。

核查后将发现归入三类：

| 类别 | 处理方式 | 交互 |
|------|----------|------|
| **可清理** | 有明确证据：无引用、已被替代、测试确认不再需要 | ⚠️ 逐项确认后执行 |
| **待确认** | 证据不足，无法判定 | 📋 写入待确认清单，说明证据缺口 |
| **科研材料（不可删除）** | 无法证明废弃的原始数据、手稿、引用库、实验记录、中间结论 | 📋 归档或标注状态，不删除 |

> **科研材料默认更保守**。无法确定价值的原始数据、手稿、引用库不删除——优先归档到 `archive/` 或标注 `(状态：待定)`。

### 阶段 3：清理与验证（⚠️ confirm）

只执行阶段 2 中确认为「可清理」的项目。

收口必须提供至少一项作为验证证据：

- 测试或 `cargo test` 通过
- 编译通过
- `git diff` / `git status` 变化清晰
- 生成物系统状态 `ok: true`
- 明确的 blocker 说明

---

## When to use

- 用户显式调用 **`/update`**
- 需要刷新关键文档、docs 索引、计划状态、研究状态或 artifact 索引
- 需要检查 git 跟踪面：未跟踪文件、误跟踪生成物、ignore 漂移
- 需要定位并清理旧代码、旧文件、旧文档、重复入口或历史残留

## Do not use

- 普通依赖升级或包版本升级
- 单一功能修 bug 且不涉及仓库知识状态或卫生维护
- Git commit / push / merge 收口
- 只改单个文案或单篇文档内容

---

## 参考

### Rust 审计入口（框架仓库可用）

`update-audit` 是 dry-run 清单入口，只读审计，不删除、不改文件：

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-audit
```

输出 JSON 字段：`key_document_candidates`、`git_tracking`、`suspected_dead_code_markers`、`suspected_stale_docs`、`suspected_retired_files`、`recommended_actions`。

仓库外 cwd 时可用 `--repo-root` 传目标仓库根：

```bash
cargo run --manifest-path /abs/path/to/framework/Cargo.toml -- framework maint update-audit --repo-root /abs/path/to/repo
```

### 框架仓库一条龙验证

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-one-shot
```

等价于：`refresh-host-projections` → `framework skills refresh --write` → 离线契约测试 → 生成物 drift-gate → 可选 host skill publish。

日常快检：

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"
```

可选外网套件：

```bash
ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS=1 cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-one-shot
```

可选宿主投影发布：

```bash
ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1 cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-one-shot
```
