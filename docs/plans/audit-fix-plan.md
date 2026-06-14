# 审计修复全量执行计划

## Context

项目文档体系 376 个 Markdown 文件 / 44,265 行，复杂度过高。经三方并行审计（文档复杂度、重复实现、死代码）并交叉验证后，确认 10 个 Batch 需执行。排除了 3 个误报（mcp_main.rs 签名不同、deprecated env 变量实际在用、ghost entries 实为虚拟 slug）。

## 执行总览

| Batch | 内容 | 风险 | 操作类型 |
|-------|------|------|----------|
| 1 | 归档重复清理 | 零 | 删除 |
| 2 | 孤立脚本删除 | 零 | 删除 |
| 3 | orphan archive crates | 零 | 删除 |
| 4 | Rust dead_code 函数 | 低 | 编辑 |
| 5 | test support 合并 | 低 | 编辑 |
| 6 | parse_range 合并 | 中 | 创建+编辑 |
| 7 | install 脚本合并 | 低 | 创建+编辑 |
| 8 | hook 脚本共享函数 | 低 | 创建+编辑 |
| 9 | docs/spec.md 拆分 | 中 | 创建+编辑 |
| 10 | 全量编译验证 | -- | cargo check |

---

## Batch 1: 归档重复清理

删除与活跃 skill 完全相同的归档副本（经 diff -rq 确认）。

- `rm -rf skills/.archive-cold/systematic-debugging/`
- `rm -rf skills/.archive-cold/tikz-paper-figure/`
- 删除 `skills/.archive-cold/paper-writing/references/` 下 7 个重复文件
- 删除 `skills/.archive-cold/paper-reviewer/references/review-rubric-playbook.md`

---

## Batch 2: 孤立脚本删除

7 个 0 引用脚本 + 1 个归档孤立脚本（全部经 grep 确认无引用）。

---

## Batch 3: Orphan Archive Crates 删除

5 个不在 workspace 中、无引用的归档 crate（`archive/rust_tools/` 下）。

---

## Batch 4: Rust Dead Code 函数删除

删除 7 个 `#[allow(dead_code)]` 未调用函数。从文件末尾向开头编辑避免行偏移。

涉及文件：
- `core/host-projection/src/hosts/cursor_hooks/handlers.rs` (3 个函数)
- `core/host-projection/src/hosts/claude_code_hooks.rs` (3 个函数)
- `core/browser-mcp/src/frag_rest.rs` (1 个函数)

编译检查点 CP-1: `cargo check --workspace`

---

## Batch 5: Test Support 合并

`router-rs` 和 `runtime-core` 的 `mcp_stdio_test_support.rs` (70行) 100% 相同。
将 `router-rs` 版本改为 re-export `runtime_core::mcp_stdio_test_support::*`。

编译检查点 CP-2: `cargo check -p router-rs`

---

## Batch 6: parse_range 合并

`pdf_tool_rs` 和 `pptx_tool_rs` 的 range parsing 函数逻辑完全相同。
提取到 `mcp-stdio-common/src/util.rs`，两个 crate 改为调用共享版本。
不改动 `pptx_tool_rs/src/lib.rs` 的 pub 版本（类型不同）。

编译检查点 CP-3: `cargo check -p pptx_tool_rs -p pdf_tool_rs -p mcp-stdio-common`

---

## Batch 7: Install 脚本合并

创建 `scripts/install-rust-tool.sh`（参数化），三个原脚本改为 thin wrapper。

---

## Batch 8: Hook 脚本共享函数提取

创建 `configs/framework/_router_rs_hook_common.sh`，提取 15 行 router-rs 二进制查找循环。
三个 hook 脚本改为 source 共享函数。

---

## Batch 9: docs/spec.md 拆分

862 行 / 18 章拆为 9 个子文档。spec.md 保留为 ~150 行轻量入口。

---

## Batch 10: 全量验证

`cargo check --workspace && cargo test --workspace --no-run` + shell 语法检查。

每个 Batch 作为独立 git commit。
