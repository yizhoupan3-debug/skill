# Spec: browser-mcp 与 router-rs-framework 工具职责分离

**状态**: ✅ 已完成
**日期**: 2026-06-13
**作者**: planx-r

---

## 1. 问题陈述

### 1.1 当前架构问题

存在严重的 **功能重叠** 和 **职责错位**：

| 工具 | browser-mcp | router-rs-framework | 问题 |
|------|:------------|:-------------------:|------|
| `skill_route` | ✅ | ✅ | **重复** |
| `skill_search` | ✅ | ❌ | **错位**（应该在 framework） |
| `skill_read` | ✅ | ❌ | **错位**（应该在 framework） |
| `skill_route_status` | ✅ | ❌ | **错位**（应该在 framework） |
| `web_fetch` | ❌ | ✅ | **缺失**（browser-mcp 也需要） |

### 1.2 根本原因

1. **browser-mcp 职责溢出**：从纯浏览器自动化 MCP 演变成了承载框架技能路由的"大杂烩"
2. **router-rs-framework 职责不完整**：作为框架核心，只实现了部分技能路由能力
3. **代码重复**：两个 MCP 各自调用 `runtime_core::routing_engine` 的相同逻辑

### 1.3 业务影响

- Cowork 环境中，Claude 被迫使用 browser-mcp 的 `skill_route`（authority: "router-rs-browser-mcp"）
- 破坏了"框架功能只由 router-rs-framework 承载"的架构原则
- 增加维护成本，修改路由逻辑需要同步两处

---

## 2. 设计原则

### 2.1 职责单一原则

**router-rs-framework** = 框架运行时（Goal/RFV/Closeout/Skill/WebFetch）
**browser-mcp** = 浏览器自动化（Chrome CDP 操控/截图/网络监听）

### 2.2 边界清晰原则

```
┌─────────────────────────────────────────────────────────┐
│                    router-rs-framework                  │
│  ┌─────────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐  │
│  │ goal_state  │ │ closeout │ │ rfv_loop │ │ skill_ │  │
│  │   _manage   │ │  _gate   │ │          │ │ route  │  │
│  └─────────────┘ └──────────┘ └──────────┘ └────────┘  │
│  ┌─────────────┐ ┌──────────┐ ┌──────────────────────┐  │
│  │ skill_search│ │skill_read│ │ skill_route_status   │  │
│  └─────────────┘ └──────────┘ └──────────────────────┘  │
│  ┌─────────────┐                                        │
│  │  web_fetch  │                                        │
│  └─────────────┘                                        │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│                      browser-mcp                        │
│  ┌─────────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐  │
│  │browser_open │ │browser_  │ │browser_  │ │browser │  │
│  │             │ │ get_state│ │  click   │ │_screenshot│ │
│  └─────────────┘ └──────────┘ └──────────┘ └────────┘  │
│  ┌─────────────┐ ┌──────────┐ ┌──────────────────────┐  │
│  │browser_fill │ │browser_  │ │ session_launch/      │  │
│  │             │ │ press    │ │ list/terminate       │  │
│  └─────────────┘ └──────────┘ └──────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 2.3 无重叠原则

任何工具只在一个 MCP 中出现，绝不重复。

---

## 3. 实施方案

### 3.1 Phase 1: router-rs-framework 补全技能路由能力

**目标**：在 router-rs-framework 中实现完整的技能路由工具集

**修改文件**：
- `core/router-rs/src/lib.rs` - 添加技能路由工具实现
- `core/runtime-core/src/framework_runtime.rs` - 复用现有 routing_engine 逻辑

**新增工具**：
```rust
// router-rs-framework 新增工具
skill_route       // 已存在，保持不变
skill_search      // 从 browser-mcp 迁移
skill_read        // 从 browser-mcp 迁移
skill_route_status // 从 browser-mcp 迁移
```

**实现策略**：
- 直接复用 `runtime_core::routing_engine` 的现有函数
- 保持 schema 与 browser-mcp 版本一致（向后兼容）
- 修改 authority 为 `"router-rs-framework"`

### 3.2 Phase 2: browser-mcp 补全 web_fetch 能力

**目标**：在 browser-mcp 中实现 web_fetch 工具

**修改文件**：
- `core/browser-mcp/src/frag_impl_browser_runtime.rs` - 添加 web_fetch 实现

**新增工具**：
```rust
// browser-mcp 新增工具
web_fetch  // 从 router-rs-framework 迁移逻辑
```

**实现策略**：
- 复用 `runtime_core::web_fetch_guard` 的安全校验逻辑
- 保持 schema 与 router-rs-framework 版本一致
- authority 为 `"router-rs-browser-mcp"`

### 3.3 Phase 3: 从 browser-mcp 移除重复技能路由工具

**目标**：移除 browser-mcp 中的 skill_route/skill_search/skill_read/skill_route_status

**修改文件**：
- `core/browser-mcp/src/frag_impl_browser_runtime.rs` - 移除 4 个函数
- `core/browser-mcp/src/frag_01_through_types.rs` - 移除工具注册

**风险控制**：
- 分阶段移除，先移除 skill_search/skill_read/skill_route_status
- 最后移除 skill_route
- 每阶段验证 Cowork 环境不受影响

### 3.4 Phase 4: 更新配置和文档

**修改文件**：
- `CLAUDE.md` - 更新网络访问优先级说明
- `AGENTS.md` - 更新 MCP 工具清单
- `docs/hosts/claude.md` - 更新宿主特有说明

---

## 4. 实施计划

### 4.1 Wave 1: router-rs-framework 补全（预计 4h）

```
router-rs-framework:
├── skill_route       ✅ 保持不变
├── skill_search      🆕 从 browser-mcp 迁移
├── skill_read        🆕 从 browser-mcp 迁移
└── skill_route_status 🆕 从 browser-mcp 迁移
```

**验证标准**：
- [x] router-rs-framework 测试通过
- [x] Claude Desktop 环境 skill 路由正常
- [x] authority 字段正确

### 4.2 Wave 2: browser-mcp 补全 web_fetch（预计 2h）

```
browser-mcp:
├── browser_open      ✅ 保持不变
├── browser_get_state ✅ 保持不变
├── web_fetch         🆕 从 router-rs-framework 迁移
└── ...
```

**验证标准**：
- [x] browser-mcp 测试通过
- [x] Cowork 环境 web_fetch 正常
- [x] SSRF 防护生效

### 4.3 Wave 3: browser-mcp 移除重复工具（预计 3h）

```
browser-mcp:
├── skill_route        ❌ 移除
├── skill_search       ❌ 移除
├── skill_read         ❌ 移除
└── skill_route_status ❌ 移除
```

**验证标准**：
- [x] Cowork 环境使用 router-rs-framework 的 skill 路由
- [x] 所有测试通过
- [x] 无功能回归

### 4.4 Wave 4: 文档更新（预计 1h）

- [x] 更新 CLAUDE.md 网络访问说明
- [x] 更新 AGENTS.md MCP 工具清单
- [x] 更新宿主特定文档

---

## 5. 迁移策略

### 5.1 向后兼容

**Phase 1-2**：新增工具，不删除旧工具
- Cowork 环境继续使用 browser-mcp 的 skill_route
- Claude Desktop 环境开始使用 router-rs-framework 的新工具

**Phase 3**：删除旧工具
- Cowork 环境切换到 router-rs-framework 的 skill_route
- 通过配置控制切换时机

### 5.2 切换控制

> **Deferred**：当前工具通过代码硬编码分离，无需动态切换。如未来需要，可通过 `skills/SKILL_ROUTING_RUNTIME.json` 中的 `mcp_tool_authority` 字段实现。

---

## 6. 验证标准

### 6.1 功能验证

- [x] Claude Desktop 环境：skill 路由正常
- [x] Cowork 环境：skill 路由正常
- [x] 两个环境的 web_fetch 都正常
- [x] SSRF 防护在两个 MCP 中都生效

### 6.2 架构验证

- [x] 每个工具只在一个 MCP 中出现（web_fetch 例外，两个 MCP 都有）
- [x] 职责边界清晰（browser-mcp = 浏览器，framework = 框架）
- [x] 无代码重复

### 6.3 性能验证

- [x] skill_route 响应时间 < 100ms
- [x] web_fetch 响应时间 < 5s
- [x] MCP 工具列表加载时间 < 500ms

---

## 7. 风险评估

### 7.1 技术风险

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| Cowork 环境 skill 路由失败 | 中 | 高 | 分阶段迁移，保留旧工具直到验证通过 |
| web_fetch SSRF 防护失效 | 低 | 高 | 复用现有防护逻辑，添加回归测试 |
| 测试覆盖不足 | 中 | 中 | 补充集成测试 |

### 7.2 时间风险

| 阶段 | 预计时间 | 缓冲 | 最晚完成 |
|------|----------|------|----------|
| Phase 1 | 4h | +1h | Day 1 |
| Phase 2 | 2h | +0.5h | Day 1 |
| Phase 3 | 3h | +1h | Day 2 |
| Phase 4 | 1h | +0.5h | Day 2 |

---

## 8. 附录

### 8.1 相关文件

- `core/browser-mcp/src/frag_impl_browser_runtime.rs` - browser-mcp 工具实现
- `core/router-rs/src/lib.rs` - router-rs-framework 入口
- `core/runtime-core/src/framework_runtime.rs` - 框架运行时核心
- `core/runtime-core/src/routing_engine/` - 路由引擎
- `core/runtime-core/src/web_fetch_guard.rs` - web_fetch 安全防护

### 8.2 相关测试

- `core/router-rs/tests/mcp_stdio_harness_tests.rs`
- `core/browser-mcp/src/tests.rs`
- `tests/smoke_browser_mcp_routing_e2e_tests.rs`
