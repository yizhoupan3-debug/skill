---
name: pdf
description: Handle layout-aware PDF reading, editing, repair, and review.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
routing_priority: P1
session_start: required
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - pdf
  - rendering
  - layout
  - typography
  - reportlab
runtime_requirements:
  commands:
    - cargo
    - pdf
metadata:
  version: "2.2.0"
  platforms: [supported]
  tags:
    - pdf
    - rendering
    - layout
    - typography
    - reportlab
    - rust
framework_roles:
  - gate
  - detector
  - verifier
framework_phase: 2
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: true
risk: low
source: local
allowed_tools:
  - shell
  - python
  - rust
approval_required_tools:
  - file overwrite
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - pdf_review.md
  - EVIDENCE_INDEX.json
  - pdf-batch/catalog.json
  - pdf-batch/results.jsonl
  - pdf-batch/index.md
  - pdf-batch/checkpoint.json
  - pdf-batch/text/*.txt

---

# pdf

本 skill 负责**版式敏感**的 PDF 工作：最终渲染外观比纯文本更重要。

默认执行面为 **Rust-first**：单文件用 `pdf read` / `pdf info`，多文件批量抽取用 `pdf batch`；Python 工具链仅作 fallback。

## 优先级路由

若主工件是 PDF，且任务为检查、生成、修复、抽取或视觉验证，应在本 skill 之前于通用文档 / 视觉 review / 领域建议中命中。

此时：

1. 本 skill 拥有 PDF 原生与 render-aware 工作流
2. 配对 skill 仅在 PDF 工件处理正确后再叠加

## Vision gate（模式分流）

按用户意图选择路径，**不要**用文本抽取代替版式判断：

| 意图 | 首选路径 | 说明 |
| --- | --- | --- |
| 理解 / 摘要 / 结构化抽取 / 多 PDF 批处理 | `pdf read` 或 `pdf batch` | 纯 Rust，可并行、可 resume |
| 版式 / 裁剪 / 重叠 / 表格错位 / 渲染缺陷 | `pdftoppm`（Poppler）或后续 `pdf render` | 先转 PNG 再目视或视觉 review |

`pdf read --json` 的 `content_class` 为 `scanned` 时，文本层不可靠，应走渲染路径而非强行摘要。

## Rust CLI 快速路径

安装（一次性，对齐 `router-rs self install` 模式）：

```bash
bash ${SKILL_FRAMEWORK_ROOT}/scripts/install-pdf-tool.sh
# 或：just install-pdf
```

开发 / 单文件探测（可用 `cargo run`）：

```bash
cargo run --manifest-path ${SKILL_FRAMEWORK_ROOT}/rust_tools/pdf_tool_rs/Cargo.toml --bin pdf -- read <input.pdf> --json
cargo run --manifest-path ${SKILL_FRAMEWORK_ROOT}/rust_tools/pdf_tool_rs/Cargo.toml --bin pdf -- info <input.pdf> --json
```

已安装 `pdf` 二进制后（**batch 必须用此路径，禁止 `cargo run`**）：

```bash
pdf read <input.pdf> --json
pdf info <input.pdf> --json
pdf batch --manifest <paths.json> --out-dir artifacts/current/<task_id>/pdf-batch
pdf batch --stdin-paths --out-dir artifacts/current/<task_id>/pdf-batch <<'EOF'
/path/to/a.pdf
/path/to/b.pdf
EOF
```

`batch` 常用选项：`--jobs auto|N`（默认 `auto`；可设 `PDF_BATCH_JOBS`）、`--resume`、`--skip-scanned`（前 1–3 页浅扫）、`--fail-fast`、`--max-chars`。

## Rust-first batch 契约

- 输出目录默认为 `artifacts/current/<task_id>/pdf-batch/`（或任务内显式 `--out-dir`）。
- 产物：
  - `catalog.json` — 批处理总览与逐文件 `content_class` / `status`
  - `results.jsonl` — 追加式逐文件结果
  - `checkpoint.json` — resume 检查点
  - `index.md` — 人类可读索引
  - `text/<sha256>.txt` — 抽取文本（相对 `out_dir`）
- **性能硬约束**：`pdf batch` 必须调用已安装的 `pdf` 二进制；**禁止**对 batch 使用 `cargo run`（每次冷启动会拖垮并行吞吐）。
- 将 `catalog.json` 路径写入 `EVIDENCE_INDEX.json` 的 `artifacts[]` 行，供下游 gate 消费。

## 贫血宿主 `record_evidence` 模板

Cursor / Codex 在 `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1` 时可由 PostTool hook 自动记录。Claude Desktop 等**无 shell hook** 的宿主，在 `pdf read` / `pdf batch` 完成后手动调用 MCP `record_evidence`：

```json
{
  "command": "pdf batch --manifest paths.json --out-dir artifacts/current/<task_id>/pdf-batch",
  "tool_name": "pdf",
  "exit_code": 0,
  "output": "catalog.json processed=N failed=0"
}
```

单文件 read 时将 `command` 换成实际 `pdf read ... --json` 行即可。

## 何时使用

- 用户要读、检查、生成、编辑或修复 PDF
- 用户关心页版式、字体、裁剪、重叠或渲染质量
- 用户要基于渲染的检查，而非仅纯文本抽取
- 用户要抽取 PDF 内容，但版式或结构仍重要
- 典型请求：
  - 「检查这个 PDF 排版」
  - 「生成一个 PDF」
  - 「把这个 PDF 读出来并看看有没有渲染问题」
  - 「批量抽取这批 PDF 的文本」

## 不要使用

- 任务实为 `.docx` Word 编辑 → 用 `$doc`
- 任务是 UI 截图 / 通用视觉 review 而非 PDF 工件 → 用 `$visual-review`
- 用户只需对已粘贴纯文本做摘要
- 文件并非 PDF

## 共享工件协议

遵循 [`primary-runtime/references/artifact-protocol.md`](../primary-runtime/references/artifact-protocol.md)。

## 硬约束

- 不要把文本抽取当作版式正确的证明。
- 版式相关变更后必须 re-render 再验收。
- 不要忽略裁剪文字、重叠元素或破损表格。
- 依赖缺失时明确说明阻塞项。
- 生成文本内容仅使用 ASCII 连字符。

## 参考

详细工作流见 [references/detailed-guide.md](./references/detailed-guide.md)。
