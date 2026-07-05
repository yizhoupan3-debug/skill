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
  source: local
  version: '3.2.0'
name: update
routing_gate: none
routing_owner: owner
routing_priority: P1
session_start: n/a
short_description: 仓库知识状态维护：文档刷新，git 跟踪审计，旧代码/旧文档清理。
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

```
/update

阶段 1: 扫描 -- README, AGENTS, docs/, 研究目录, 未跟踪文件, 死代码标记
阶段 2: 核查 + 清理 -- 交叉验证，确认的直接执行，不确定的写入报告
阶段 3: 汇总 -- 展示执行结果和待人工处理项
```

---

## 执行模型（3 阶段，全自动驱动）

`/update` 设计为**端到端自驱**：启动后不停顿等待确认，主动执行有证据的操作，把不确定性收敛到最终报告。

### 阶段 1：扫描

扫描仓库的关键知识面并列出候选清单。不做判断，不修改文件。

- **关键文档**：README、AGENTS.md、docs 索引、计划/研究目录、实验记录、artifact 指针
- **git 跟踪面**：未跟踪文件、误跟踪的生成物/缓存、.gitignore 漂移、应纳管的 Markdown
- **旧代码/旧文件/旧文档**：疑似死代码、死文件、过期文档、重复入口、历史残留
- **科研材料**：论文草稿、rebuttal、cover letter、实验数据、研究计划、文献综述、引用库、环境/复现说明、结果表、figure/table 说明、状态 ledger

### 阶段 2：核查 + 分类处理

对候选清单做交叉验证：引用搜索、编译/测试、文档索引、git 修改历史、命名线索。验证后分三类自动处理：

| 分类 | 条件 | 行为 |
|------|------|------|
| **可清理** | 明确无引用、已被替代、测试确认不再需要的代码/文件/文档 | 直接执行清理（删除或移动），不留待办 |
| **待确认** | 证据不足，或涉及科研材料 | 写入 `update-report.md`，附证据缺口说明，不做操作 |
| **科研材料** | 原始数据、手稿、引用库、实验记录、中间结论 | 标注状态或归档到 `archive/`，不删除 |

> 科研材料默认保守：无法确定价值的原始数据、手稿、引用库不删除。优先归档或标注 `[状态：待定]`。

### 阶段 3：汇总

操作完成后输出一条摘要，包含：

- 清理了多少项、分别是什么
- 归档/标注了多少科研材料
- 有多少项待确认（列出概要，指向 `update-report.md`）
- git 跟踪面建议（应纳管/应忽略/应迁移）
- 可选的验证证据：测试通过、编译通过、diff 状态

**`update-report.md`** 写入仓库根目录 `.update/` 下，记录待确认项的详细信息。每次 `/update` 覆盖上一份报告。

---

## When to use

- 用户显式调用 `/update`
- 需要刷新关键文档、docs 索引、计划状态、研究状态或 artifact 索引
- 需要检查 git 跟踪面：未跟踪文件、误跟踪生成物、ignore 漂移
- 需要定位并清理旧代码、旧文件、旧文档、重复入口或历史残留

## Do not use

- 普通依赖升级或包版本升级
- 单一功能修 bug 且不涉及仓库知识状态或卫生维护
- Git commit / push / merge 收口
- 只改单个文案或单篇文档内容

---

## 参考：Rust 审计入口（框架仓库）

`update-audit` 是 dry-run 清单入口，只读审计，不删除、不改文件：

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-audit
```

输出 JSON 字段：`key_document_candidates`、`git_tracking`、`suspected_dead_code_markers`、`suspected_stale_docs`、`suspected_retired_files`、`recommended_actions`。

仓库外 cwd 时可用 `--repo-root` 传目标仓库根：

```bash
cargo run --manifest-path /abs/path/to/framework/Cargo.toml -- framework maint update-audit --repo-root /abs/path/to/repo
```
