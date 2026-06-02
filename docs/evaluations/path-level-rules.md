# P3 评估备忘：路径级规则激活

> 生成日期：2026-06-02
> 状态：待评估（不阻塞任何 Phase）

## 背景

Claude Code 支持 `.claude/rules/` 目录下的路径级规则——按文件路径模式自动加载，减少上下文浪费。Cursor 的 `.mdc` 格式更进一步，支持 globs + intelligent + manual 四种激活模式。

当前框架规模：~40 个 skill，SKILL.md 总行数约 6000+ 行。规则通过 `SKILL_ROUTING_RUNTIME.json` 按自然语言查询匹配。

## 评估问题

1. **当前规模是否需要路径级规则？**
   - ~40 个 skill 的规模不需要路径级优化
   - 自然语言路由（skill_route）已足够灵活
   - 结论：**当前不需要**

2. **何时值得引入？**
   - skill 数量超过 100 个
   - 出现明确的上下文窗口溢出问题
   - 需要为特定目录（如 `core/`、`configs/`）定义编码约束

3. **潜在方案**：
   - 方案 A：利用 Claude Code 原生 `.claude/rules/` 机制，为 `skills/implementx/`、`skills/verifyx/` 等创建路径规则
   - 方案 B：在 SKILL.md 的 frontmatter 中增加 `path_patterns` 字段，由路由层在匹配时注入路径上下文
   - 方案 C：维持现状，通过 skill 自身的 trigger_hints 隐式路径匹配

## 建议

- **不阻塞**任何现有工作
- 记录为架构备忘，待出现明确痛点时再评估
- 优先级：P3（最低）

## 参考

- Claude Code rules 文档: https://code.claude.com/docs/en/rules
- Cursor .mdc 格式: https://cursor.com/docs/rules
- 本框架路由架构: docs/ARCHITECTURE.md
