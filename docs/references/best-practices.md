# 最佳实践（Best Practices）

> 索引入口。各条目链接到权威来源，不重复全文。

---

## 1. 编码首要原则（Coding First Principles）

**来源**：[AGENTS.md](../../AGENTS.md) § Coding First Principles

五门槛（任何代码变更前逐条回答）：

| # | 门槛 | 自检 |
|---|------|------|
| 1 | **Goal** | 这次变更要解决什么问题？ |
| 2 | **Non-goals** | 什么不在范围内？ |
| 3 | **Existing owner** | 是否已有模块/函数负责此功能？ |
| 4 | **Minimal delta** | 能否用最小改动完成？ |
| 5 | **Validation** | 如何验证变更正确？ |

---

## 2. 科研编码标准（Scientific Coding Standards）

**来源**：[AGENTS.md](../../AGENTS.md) § Scientific Coding Standards

- **随机种子**：所有涉及随机性的实验必须显式设置 seed，确保可复现
- **产物归档**：实验产物（数据、图表、模型快照）必须归档到 `artifacts/` 并记录到 EVIDENCE_INDEX
- **Checkpoint**：长运行任务必须支持 checkpoint/resume，避免重复计算

---

## 3. Skill 维护约定

**来源**：[SKILL_MAINTENANCE_GUIDE.md](../../skills/SKILL_MAINTENANCE_GUIDE.md)

- **热表/冷表协议**：`SKILL_ROUTING_RUNTIME.json`（热表，只读）+ `SKILL_MANIFEST.json`（全量索引）
- **Reroute 别名**：双表同构，不得 runtime-only 漂移
- **废弃流程**：从热表和 MANIFEST 移除 → 删除 SKILL.md 及 references/（内容如有价值先吸收到其他 skill）
- **验证命令**：`router-rs framework skills validate`

---

## 4. 错误处理（Error Handling）

**来源**：`core/core-state/src/`（error 类型）+ `core/router-rs/src/framework_error.rs`

### 分层规则

| 层 | 错误类型 | 说明 |
|----|---------|------|
| **内部**（业务逻辑） | `FrameworkResult<T>` | 使用 `FrameworkError` 的语义 variant（`Validation`、`NotFound`、`Conflict` 等） |
| **边界出口**（Hook/CLI/MCP） | `Result<T, String>` | 通过 `.map_hook_exit()` / `.map_stdio_exit()` / `.map_route_exit()` 转换 |

### 迁移模式

| 场景 | 模式 |
|------|------|
| 函数签名 | `Result<T, String>` → `FrameworkResult<T>` |
| 错误构造 | `Err("msg".to_string())` → `Err(FrameworkError::other("msg"))` |
| IO 错误 | `.map_err(\|e\| e.to_string())?` → 直接 `?`（`From<std::io::Error>` 自动转换） |
| JSON 错误 | `.map_err(\|e\| e.to_string())?` → 直接 `?`（`From<serde_json::Error>` 自动转换） |
| 子函数仍返回 String | `.map_framework_err()` 桥接 |

### `FrameworkError` variant 选择指南

| Variant | 适用场景 |
|---------|---------|
| `Validation` | 输入校验失败（字段缺失、格式错误） |
| `NotFound` | 资源不存在（文件、task、配置） |
| `Conflict` | 状态冲突（CAS 失败、重复操作） |
| `PathGuard` | 路径安全违规（越界写入） |
| `Timeout` | 超时 |
| `Other` | 通用兜底 / 未分类 |

---

## 5. 测试约定（Testing Conventions）

### 基本要求

- `cargo test --workspace --no-fail-fast` → 全绿
- `cargo clippy -p core-state -- -D warnings` → 0 warnings
- `cargo clippy -p router-rs -- -D warnings` → 0 warnings（预存 MSRV/clippy 问题除外）

### 断言模式

- 比较 `FrameworkError` 时使用 `.to_string()` 转换：`assert_eq!(err.to_string(), "expected message")`
- 比较 `FrameworkError` 的包含关系时使用 `.to_string().contains(...)`：`assert!(err.to_string().contains("keyword"))`

### 测试隔离

- 使用 `tempdir()` 创建临时目录，测试结束后自动清理
- 并行测试必须使用独立目录（含唯一后缀），避免文件冲突

---

## 6. Git 约定

**来源**：[AGENTS.md](../../AGENTS.md) § Git

- Commit message 使用 conventional commits 格式
- 不在 main 分支直接提交（先创建 feature branch）
- PR 前确保 `cargo test --workspace` 和 `cargo clippy` 全绿

---

*创建于 2026-06-08，roadmap v4 deferred 收尾。*
