---
name: refresh
description: "使用 Rust refresh 命令生成并复制下一轮执行提示"
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - $refresh
  - refresh
  - 下一轮执行提示
  - 复制到剪贴板
---

# Refresh

这是 Codex 侧的 `$refresh` 能力。

## 何时使用

- 用户想为当前仓库准备下一轮执行提示
- 这段提示需要复制到 macOS 剪贴板
- 用户期待收到一句固定确认语

## 不要用于

- 任务目标是刷新共享 memory 制品，或重写 continuity 状态
- 任务只是通用记笔记、只做总结输出，或处理无关的 skill 维护

## 目标

用 Rust refresh 命令生成下一轮执行提示，复制到剪贴板，并返回固定确认语。

## 操作说明

1. 运行：

```bash
PROJECT_DIR="${CODEX_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/run_router_rs.sh "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --framework-refresh-json --repo-root "$PROJECT_DIR"
```

2. 然后严格回复：

```text
下一轮执行 prompt 已准备好，并且已经复制到剪贴板。
```

### 可选 debug 模式

如果用户明确要求 verbose/debug refresh，就在同一条命令后追加 `--framework-refresh-verbose`，检查返回 JSON 里的 `refresh.debug`，只概括用户点名要看的诊断字段。
正常执行 `$refresh` 时，不要改动默认确认语。

## 约束

- 不要重写根目录 continuity 制品
- 不要刷新 host-specific memory projection
- 面向用户的回复里，不要提 memory refresh、summary、`/clear` 或内部诊断
- 行为要与 Codex `$refresh` 入口保持一致

## 用法

```text
$refresh
```
