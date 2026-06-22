# 文档

**7 层架构**（详见 [spec.md](spec.md)）：
1. **宿主层** (host-projection) — 轻薄适配器
2. **路由层** (routing-engine, router-rs) — 意图匹配
3. **Skill 层** (framework-kernel::skill_lint) — 技能契约
4. **工具层** (tool-layer) — ToolRegistry
5. **运行层** (runtime-core) — 编排
6. **Hook 层** (hook-layer) — 函数指针注册
7. **Feature 层** (research-harness) — 领域插件

**运维**：[operations/index.md](operations/index.md) · [getting-started.md](operations/getting-started.md)
**宿主手册**：[hosts/_common.md](hosts/_common.md) · [hosts/hook-hosts.md](hosts/hook-hosts.md) · [hosts/opencode.md](hosts/opencode.md)
**架构决策**：[adr/006-six-layer-architecture.md](adr/006-six-layer-architecture.md) · [adr/007-dual-exit-gates.md](adr/007-dual-exit-gates.md)
**顶级策略**：[AGENTS.md](../AGENTS.md)（跨宿主执行与语言策略）
