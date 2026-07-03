# Execution Lane — 实验设计、数学建模/验证、代码验证、研究工作区

本 lane 是 `$research` 统一前门的执行子模式。覆盖实验/数学/代码等执行类工作。包含科研工作区 CLI（原 autoresearch）作为工具入口。

## 使用场景

- 实验设计：ablation、benchmark、实验方案、forensics（失败复盘）
- 数学建模：控制方程、量纲分析、本构/闭合模型
- 数学验证：推导审核、witness + checker、假设依赖图
- 代码验证：实现审计、测试、确定性复现、benchmark 命令
- 研究工作区管理：研究初始化、claim/假设跟踪、实验记录、日志
- 可重复性：委托给 `$experiment-reproducibility`（L3）
- 用户从 `$research` 统一前门接收到此 lane

## 不要使用

- 用户需要文献调研/理论背景 → route to `$research` discovery lane
- 用户有手稿对象 → route to `$research` paper-workbench lane
- 用户只问统计检验 → `$statistical-analysis`
- 用户只需要纯数学验证/探索 → `$math-verify` 或 `$math-explore`
- 用户只需要引用格式 → `$citation-management`
- 用户只需要可重复性管理 → `$experiment-reproducibility`
- 普通代码实现（无研究级证据闸门）→ 当前 coding context

## Lane routing

| Lane | 说明 | 输出 |
|------|------|------|
| `experiment_design` | 变量/对照/ablation/baseline/metrics/sample size/failure criteria | experiment plan |
| `experiment_forensics` | 失败复盘：哪些假设/变量导致失败，下一假设 | forensics report |
| `math_verification` | 推导审核 + checker 选项 | witness list + checker result |
| `math_modeling` | 建模型：变量/方程/闭合/无量纲/regime chart | model_spec |
| `code_verification` | 代码审计 + 测试 + 确定性复现 | test results + repro commands |
| `reproducibility` | 委托给 `$experiment-reproducibility`（执行）+ `$reproducibility-verification`（审计）| reproducibility record |
| `workspace management` | 研究初始化/claim/假设/日志/run record，调用 research-harness CLI | CLI output |
| `barrier_escalation` | loop bridge：瓶颈系统研究 → BARRIER_REPORT.json | candidate solutions |

## 研究工作区 CLI（research-harness）

研究工作区能力由 `research-harness` CLI 提供（二进制在 `core/research-harness/src/bin/autoresearch.rs`）：

| CLI 子命令 | 用途 |
|------------|------|
| `init` | 初始化研究方向 |
| `status / next / resume` | 查看/继续工作区 |
| `draft-claims / compare-claim / set-novelty-gate` | Claim 生命周期 |
| `add-hypothesis / list-hypotheses` | 假设 CRUD |
| `record-run` | 实验记录（环境 + git）|
| `reflect` | 实验反思 + drift 检测 |
| `sync` | 同步到 artifact |
| `barrier <problem>` | **Loop bridge**：瓶颈研究 → BARRIER_REPORT.json |

分层日志功能通过 `research_harness::log` 库接口提供（SQLite FTS5 日志存储 + 知识图谱）。快速实验探测通过 `research_smoke` MCP 工具调用模板执行。

调用方式：`cargo run -p research-harness --bin autoresearch -- <subcommand>`。

**Barrier escalation protocol**：loop-auto 遇 `consecutive_failures ≥ threshold` 时：
1. 调用 `autoresearch barrier <description>`
2. 初始化研究方向（以 barrier 问题为 question）
3. 文献调研（通过 `$research` discovery lane）
4. 假设生成（draft-claims）
5. 输 BARRIER_REPORT.json → `artifacts/research-barrier/<barrier-id>/`
6. Loop runner 消费 report → 选 candidate → resume

## 验证合约

- 没有 baselines + controls + metrics + reproducibility 要求不得声称实验有效性
- 所有数学验证必须附带 witness + checker（或 blocker）
- 失败复盘后如发现 discovery 阶段未覆盖的新未知 → **必须 loop-back** 到 discovery lane

## 相关资源

- 链路协议：详见 [`../references/research-lane-routing.md`](../references/research-lane-routing.md)
- Math reasoning harness：[docs/math-reasoning-harness.md](../../../docs/math-reasoning-harness.md)
- 研究工作区 CLI：详见 [`../research-harness/SKILL.md`](../research-harness/SKILL.md)
- Quality gates（退出门）：
  - `math_verification` / `math_modeling` → `$formal-verification`
  - `reproducibility` → `$reproducibility-verification`
  - `code_verification` → `$code-review-deep`
