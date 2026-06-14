---
parent: docs/spec.md
version: unified-v7
---

## 14. 辅助模块

### 14.1 paper_adversarial_hook.rs — 论文对抗审稿

- opt-in，per-host 环境变量控制
- 文案真源：`configs/framework/PAPER_ADVERSARIAL_HOOK.txt`（`include_str!`）

### 14.2 paper_prose_hook.rs — 论文润色

- 默认开启，per-host 环境变量关闭
- 合并 prose quality chain 短段

### 14.3 harness_operator_nudges.rs — 运维提示

- RFV/Goal drive 的运维提示行注入
- 真源：`configs/framework/HARNESS_OPERATOR_NUDGES.json`

### 14.4 harness_context_signals.rs — 上下文信号

- 数学/形式化验证上下文的启发式检测
- 为 harness 运维提示提供信号

### 14.5 harness_contract.rs — Harness 契约

- 失败分类学（10 类：route_miss/owner_drift/context_rot 等）
- Skill 契约 lint（frontmatter 完整性）

### 14.6 formal_toolchain.rs — 形式化验证检测

- ASCII 小写子串启发式检测形式化证明工具（SymPy/Z3/Lean/Coq/Isabelle/Agda）

### 14.7 schema_drift.rs — Schema 漂移检测

- 任务/harness schema 漂移基线捕获与对比验证
- 确保 hook 事件集、artifact 结构、契约版本不漂移

### 14.8 execution_contract.rs — 执行契约

- 执行内核元数据 + 契约包 + 实时响应序列化契约
- 纯数据构建（无模块依赖），43 个测试

### 14.9 router_rs_observation.rs — 观测

- Hook 出站 JSON 的结构化观测载荷注入
- 门控分类、相关性提取、阻断判定

### 14.10 session_call_tracker.rs — 会话调用追踪

- 工具调用和 token 使用追踪
- 异常检测：单工具上限、总调用上限、skill_route 首次检测
- 持久化：`artifacts/current/SESSION_CALL_TRACKER.json`

### 14.11 html_to_markdown.rs + content_extract.rs

- `html_to_markdown()` — 纯 Rust HTML → Markdown
- `extract_readable_content()` — readability 替代方案

### 14.12 trace_runtime.rs — 追踪运行时

- Trace 事件记录、流式输出、压缩、delta 快照和重放
- schema：`runtime-trace-v2`

### 14.13 runtime_envelope_ids.rs — 常量集中

- 30+ schema_version/authority 常量
- 资源限制：`DEFAULT_MAX_CONCURRENT_SUBAGENTS=8`, `MAX=24`

### 14.14 router_self.rs — 自身管理

- `router-rs self install|clean` — 全局二进制安装/清理
- macOS 自动 ad-hoc 签名刷新

### 14.15 router_env_flags.rs — 环境变量开关

- 30+ `ROUTER_RS_*` 环境变量 helper
- 通用 token：`1`/`true`/`yes`/`on` = enabled；`0`/`false`/`off`/`no` = disabled

---

