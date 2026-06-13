---
last_verified: "2026-06-13"
---

# 模块文档索引

按代码 crate 组织，每个模块文档记录：职责、核心功能、pub 接口、依赖关系、近期变更、已知技术债。

## Crate 分层

| 层级 | Crate | 行数 | 文档 |
|------|-------|------|------|
| B0 | `framework-kernel` | ~3,900 | [framework-kernel.md](framework-kernel.md) |
| B1 | `runtime-core` | ~38,000 | [runtime-core.md](runtime-core.md) |
| B1 | `core-policy` | ~4,400 | [core-policy.md](core-policy.md) |
| B1 | `core-state` | ~6,900 | （薄代理，见 runtime-core） |
| B2 | `host-projection` | ~34,000 | [host-projection.md](host-projection.md) |
| B2 | `browser-mcp` | ~5,600 | [browser-mcp.md](browser-mcp.md) |

## 子模块详解

| 文档 | 范围 |
|------|------|
| [runtime-core-framework-runtime.md](runtime-core-framework-runtime.md) | `framework_runtime/` 子模块（27 个文件，12,000+ 行） |
| [host-projection-projection.md](host-projection-projection.md) | `host_integration/projection/` 子模块（4 个文件，3,700 行） |
| [host-projection-hosts.md](host-projection-hosts.md) | `hosts/` 子模块（四宿主 hook 实现 + 共享抽象） |

## 阅读路径

1. **快速了解架构**：[framework-kernel.md](framework-kernel.md) → [runtime-core.md](runtime-core.md) → [host-projection.md](host-projection.md)
2. **深入 hook 系统**：[host-projection-hosts.md](host-projection-hosts.md) → [core-policy.md](core-policy.md)
3. **框架运行时核心**：[runtime-core-framework-runtime.md](runtime-core-framework-runtime.md)
4. **浏览器自动化**：[browser-mcp.md](browser-mcp.md)
