---
parent: docs/spec.md
version: unified-v7
---

## 8. 路由与插件契约

### 8.1 Skill 注册

**真源**：`skills/SKILL_ROUTING_RUNTIME.json`（列索引格式）

- `load_records_from_runtime()` — 纯配置加载
- `SKILL_ROUTING_METADATA.json` — 运行时元数据补丁
- `filter_records_for_host()` — 按 host_platforms 过滤
  - `SKILL_MANIFEST.json` 中 `host_platforms` 支持通配符令牌：`"supported"` / `"all-hosts"` 展开为 `host_targets.supported` 全集（规范化逻辑见 `tests/host_platforms.rs`）
- 可插拔性：**4.0/5**

### 8.2 路由评分

- `route_task()` → `score_route_candidate()`（815 行硬编码，可插拔性 **2.0/5**）
- `signals/`：5 个子模块（design_artifact/devtools/markers/orchestration/paper）
- `nl_route_adjustments.rs`：NL suppress/boost 调整

### 8.3 路由引擎模块

| 子模块 | 功能 |
|--------|------|
| `routing.rs` | 主路由逻辑 + manifest fallback |
| `scoring.rs` | 候选评分（boost/suppress/overlay） |
| `records.rs` | 记录加载 + 缓存（mtime-based OnceLock+RwLock） |
| `policy.rs` | 路由策略载荷 |
| `text.rs` | 文本规范化 + 分词 |
| `aliases.rs` | 框架 alias 检测 |

---

