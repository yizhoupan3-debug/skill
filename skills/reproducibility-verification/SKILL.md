---
name: reproducibility-verification
description: |
  experiment-reproducibility 的无状态审计子能力：验证实验的可复现性——种子、确定性、环境锁定、数据版本和
  checkpoint 恢复。由 research-execution、paper-workbench 内联调用，不独立编排会话。
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
user-invocable: false
disable-model-invocation: true
risk: low
source: local
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [reproducibility, verification, environment, research]
trigger_hints:
  - 可复现性验证
  - 种子检查
  - 确定性重跑
  - 环境锁定验证
  - checkpoint 恢复
---

# Reproducibility Verification

无状态能力 skill：对实验代码和配置做可复现性审计。不独立编排会话。

## When to Use

- 前门 skill 需要验证实验是否可复现
- 需要检查随机种子是否被正确设置
- 需要确认确定性重跑结果一致
- 需要验证 lock file 是否有效
- 需要检查数据版本化是否到位
- 需要验证 checkpoint 可恢复性

## Do not use

- 实验设计、数据收集规划 → 使用 `$research-execution`
- 统计结果审计 → 使用 `$statistical-verification`
- 纯代码实现 without 实验语境 → 在当前 coding context 直接回答

## Hard constraints

- 种子未设置或为随机值为 P0 blocker——无法复现的实验不可接受
- 确定性重跑 hash 不一致为 FAIL，不得以 "浮点精度" 为由自动豁免（需显式证明是浮点问题）
- lock file 缺失为 FAIL——不可复现的环境依赖是系统性风险
- checkpoint 恢复失败为 P0 blocker——长流程实验的 checkpoint 是唯一恢复手段
- 数据版本化缺失为 WARN（非所有项目都需要 DVC/LFS），但必须在报告中显式标注

## Input / Output

| 输入 | 输出 |
|------|------|
| 实验代码目录 | 每项检查的 PASS / FAIL / WARN 状态 |
| 运行日志（可选） | 两次运行结果的 hash 比较 |
| 环境配置文件 | 环境完整性报告 |

## Verification Checklist

可执行脚本：[`scripts/verify/reproducibility.sh`](../../scripts/verify/reproducibility.sh)

```bash
# 自动检测 lock file + source 目录：
SRCDIR=src/ scripts/verify/reproducibility.sh

# 指定 checkpoint 文件：
CKPT=checkpoints/latest.pt scripts/verify/reproducibility.sh
```

| # | 检查名 | PASS 条件 |
|---|--------|-----------|
| 1 | 种子已设置 | 代码中存在 seed 设置且非 None/随机 |
| 2 | 确定性重跑 | 两次运行的输出 hash 一致 |
| 3 | 环境可复现 | lock file 存在且与代码同步 |
| 4 | 数据版本化 | 输入数据有 DVC tracking 或 Git LFS |
| 5 | Checkpoint 可恢复 | checkpoint 文件存在且 load 无报错 |

## References

- experiment-reproducibility skill：[`../experiment-reproducibility/SKILL.md`](../experiment-reproducibility/SKILL.md)（可复现性管理知识库）
- experiment-reproducibility 模板：[`../experiment-reproducibility/references/templates.md`](../experiment-reproducibility/references/templates.md)
- 科研纪录最低清单：[`../experiment-reproducibility/references/research-record-minimum.md`](../experiment-reproducibility/references/research-record-minimum.md)

## Integration

前门 skill 在以下时机内联调用本 skill：

- **research-execution**：实验运行完成后，验证可复现性
- **paper-workbench**：投稿前对实验部分做可复现性门禁

调用方式：按验证清单逐项执行，FAIL 项作为 blocker 回写前门 skill。
