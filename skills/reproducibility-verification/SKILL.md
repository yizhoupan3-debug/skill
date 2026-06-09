---
name: reproducibility-verification
description: |
  无状态内部 skill：验证实验的可复现性——种子、确定性、环境锁定、数据版本和
  checkpoint 恢复。由 research-execution、paper-workbench 内联调用。
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

## Input / Output

| 输入 | 输出 |
|------|------|
| 实验代码目录 | 每项检查的 PASS / FAIL / WARN 状态 |
| 运行日志（可选） | 两次运行结果的 hash 比较 |
| 环境配置文件 | 环境完整性报告 |

## Verification Checklist

> **Note**: verify commands below are pattern templates. Actual commands depend on project setup and available tools.

| # | 检查名 | PASS 条件 | verify command |
|---|--------|-----------|----------------|
| 1 | 种子已设置 | 代码中存在 seed 设置且非 None/随机 | `grep -rnE 'seed\s*=\s*[0-9]+' src/` → 匹配 ≥ 1 处 |
| 2 | 确定性重跑 | 两次运行的输出 hash 一致 | `sha256sum run1_output.json run2_output.json` → hash 相同 |
| 3 | 环境可复现 | lock file 存在且与代码同步 | `test -f uv.lock && uv lock --check` → exit 0 |
| 4 | 数据版本化 | 输入数据有 DVC tracking 或 Git LFS | `test -f .dvc || git lfs track --list` → 非空 |
| 5 | Checkpoint 可恢复 | checkpoint 文件存在且 load 无报错 | `python -c "import torch; torch.load('ckpt.pt')"` → exit 0 |

## References

- experiment-reproducibility skill：[`../experiment-reproducibility/SKILL.md`](../experiment-reproducibility/SKILL.md)（可复现性管理知识库）
- experiment-reproducibility 模板：[`../experiment-reproducibility/references/templates.md`](../experiment-reproducibility/references/templates.md)
- 科研纪录最低清单：[`../experiment-reproducibility/references/research-record-minimum.md`](../experiment-reproducibility/references/research-record-minimum.md)

## Integration

前门 skill 在以下时机内联调用本 skill：

- **research-execution**：实验运行完成后，验证可复现性
- **paper-workbench**：投稿前对实验部分做可复现性门禁

调用方式：按验证清单逐项执行，FAIL 项作为 blocker 回写前门 skill。
