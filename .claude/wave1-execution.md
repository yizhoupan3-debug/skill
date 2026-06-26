
# ROADMAP-v10 Wave 1 执行

## Wave 1: 定义与标注（零行为变更）

总目标和范围：
- 创建 `core/quality-gate/` crate（GateChecker trait, CheckContext, CheckerRegistry, Severity, Finding, GateVerdict, aggregate）
- 所有 SKILL.md 加 `scene:` 字段（~47 个文件，默认 `general`）
- frontmatter_parser 解析 scene 字段
- RouteDecision 数据模型增加 scene 字段

**关键约束**：
- 零行为变更。新 crate 不被任何现有代码依赖
- scene 缺失或无效时降级为 `"general"`
- Type design（不可违反的决策来自 ROADMAP-v10.md）：
  - GateChecker 的 check() 签名同步（async 通过 runtime_handle）
  - CheckResult 无 severity 字段（severity 只属于 Finding）
  - CheckContext 无 previous_results
  - Severity 沿用 research-harness 规范：P0/A/B/Warning/C
  - P0/A/B → blockers；Warning/C → advisories
  - AdversarialChecker（通用兜底）在 general scene 下注册
  - CheckerRegistry 启动时由 runtime_core::init() 中的 register_*_checkers() 显式注册

参考文档：
- `/Users/joe/Developer/skill/docs/ROADMAP-v10.md` 全量架构设计
- `/Users/joe/Developer/skill/core/research-harness/src/review/severity.rs`（Severity 规范）
- `/Users/joe/Developer/skill/core/research-harness/src/`（checker 实现参考）

开始执行 Wave 1。每个子任务独立、并行。
