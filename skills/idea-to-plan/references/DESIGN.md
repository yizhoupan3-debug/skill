# Idea-to-Plan Design Specification (Optimized)

## 1. Core Philosophy: Deterministic Ideation (确定性意图塑造)

目前的 `Execution Controller` (L0) 重点在于“执行的确定性”，即如何确保 120+ 技能按契约完成任务。而 `Idea-to-Plan` 控制器应定位为 **L-1 (Strategic Layer)**，其核心哲学是：**“在意图进入执行管道前，通过结构化发散与硬性约约束，将其塑造为高成熟度的可执行蓝图。”**

## 2. 架构设计：Stratospheric Orchestrator (平流层编排器)

该 Skill 将作为所有复杂任务的“漏斗”入口，位于 `brainstorm-research` (发散) 和 `plan-writing` (收敛) 之上。

### 2.1 核心阶段 (The 5-Stage Blueprinting Loop)

1.  **Intake & Sourcing (意图摄入)**
    - 识别原始 Idea 的模糊度（Level 0-2）。
    - 联动 `brainstorm-research` 进行 360 度视角扩展。
2.  **Feasibility & Novelty Sieve (可行性与新颖性双筛)**
    - **Repo-Aware Audit**：实时扫描当前代码库的架构约束。
    - **Scientific Novelty Gate**：联动 `$academic-search` 进行全网文献初筛，并调用 `$literature-synthesis` (Mode B) 确定核心创新点。
3.  **Recursive Decomposition & Architectural Gate (递归分解与架构门禁)**
    - 将验证通过的 Idea 分解为 Atomic Workstreams。
    - **Stage 3.1: Architecture & Security Sieve**：联动 `$system-architect` 验证 Workstream 是否符合仓库目前的模块解耦标准。调用 `$security-audit` 检查是否有高风险的 Eval 或网络请求模式。
    - 指派特定的 L3/L4 技能（如 `math-derivation`, `python-pro`）。
3.5 **Multi-Direction Pilot (多向试点验证与快速判别)**
    - **Agentic Tree Search**：使用 `$subagent-delegation` 开启并行分支。每个分支独立执行 `pilot_spec.json`。
    - **Recursive L0 Micro-Invocation**：Stage 3.5 **直接调用 `$execution-controller-coding` (L0)** 运行“验证性微任务”。
    - **Stage 3.6: Pivot Decision Layer**：基于 Pilot 结果触发以下逻辑：回滚/微调/合并。
4.  **Scientific & Integration Synthesis (科研与工程双重综合)**
    - **Gap-Grounded Synthesis**：调用 `$literature-synthesis` (Mode E) 产出证据关联图。
    - **Spec Alignment**：调用 `$plan-writing` 确保 `implementation_plan.md` 符合 L4 成熟度。
5.  **Handoff Contract (交付契约与标准化产出)**
    - 输出包含引用列表的 `.supervisor_strategy.json`。
    - **标准化交付双文档 (The Duo-Doc Delivery)**：`outline.md` 与 `code_list.md`。

## 3. 技能协同矩阵 (Planning Synergy Matrix - 120+ Core)

- **战略发散层 (L3)**：`$brainstorm-research`, `$academic-search`.
- **可视化与交付层 (L4)**：`$scientific-figure-plotting`, `$imagegen`, `$ppt-markdown`.
- **核心内核 (L0)**：`$execution-controller-coding` (递归调用入口)。

## 4. 关键机制：Optimization & Governance

-   **Agentic Tree Search**：并行探索分支。
-   **Internal Peer Reviewer**：恶意评审层。
-   **Anti-Laziness Enforcement**：强制要求 `code_list.md` 不含任何 Placeholder。

## 5. 状态持久化：`.supervisor_strategy.json` Schema

```json
{
  "strategy_id": "uuid",
  "root_idea": "string",
  "handoff_contract": {
    "outline_path": "outline.md",
    "code_list_path": "code_list.md",
    "maturity_level": 4
  }
}
```
