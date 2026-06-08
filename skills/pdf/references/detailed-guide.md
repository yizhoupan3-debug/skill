# pdf — 详细指南

> 从 SKILL.md 拆出，降低路由时 token 消耗。

## 任务归属与边界

本 skill 拥有：

- 带渲染意识的 PDF 阅读
- PDF 生成工作流
- PDF 版式缺陷检查
- 与 PDF 工件绑定的文本 / 结构抽取
- PDF 变更后的 re-render 复验循环

本 skill 不拥有：

- Word `.docx` 编辑
- 与 PDF 无关的通用图像 / UI review
- 论文科研逻辑审阅

邻近任务路由：

- `$doc` → `.docx`
- `$visual-review` → 已有页面截图的图像级 review
- `$paper-reviewer` / `$paper-workbench` → 手稿级 review 中的 PDF 版式判断

## Finding-driven 框架角色

本 skill 是共享 finding-driven 框架中的 **Phase-2 artifact gate / detector / verifier**。保持 PDF 工作 PDF-native，再向下游发射 findings 或 verification results。结构见 [`../../SKILL_FRAMEWORK_PROTOCOLS.md`](../../SKILL_FRAMEWORK_PROTOCOLS.md)。

PDF 审计至少保留：

- `finding_id`
- `artifact_ref`
- `evidence`（渲染页或抽取不匹配）
- `fixability`
- `verification_method`
- `status`

## 必需工作流

1. 识别任务模式：read / generate / edit / audit / **batch**
2. 版式重要时优先 render-based 检查。
3. 选用最小正确工具；**默认 Rust `pdf` CLI**。
4. 有意义变更后 re-render。
5. 同时交付实质结果与渲染质量状态。

## Vision gate

| 模式 | 工具 | 备注 |
| --- | --- | --- |
| 理解 / 单文件抽取 | `pdf read` | `--json` 含 `content_class`、`page_count`、`warnings` |
| 元数据快探 | `pdf info` | 不全文抽取时用 |
| 多文件批处理 | `pdf batch` | 见下文 batch 路径；**禁止 cargo run** |
| 版式 / 视觉缺陷 | `pdftoppm -png` + 图像检查 | 待 `pdf render` 子命令落地后可切换 |

`content_class: scanned` → 不要仅靠 `read` 文本做结论；转渲染路径。

## Rust CLI 路径

### 安装

```bash
bash ${SKILL_FRAMEWORK_ROOT}/scripts/install-pdf-tool.sh
# 或：just install-pdf
```

将 `~/.local/bin`（或 `PDF_BIN_DIR`）加入 `PATH`，确保 `which pdf` 命中 release 二进制。

安装脚本用 `scripts/rust-release-bin.sh` 解析 workspace `target-dir`（根目录 `.cargo/config.toml` 可能指向统一目录，勿假设 `rust_tools/pdf_tool_rs/target/release/`）。

### 单文件 read / info（开发可用 cargo run）

```bash
cargo run --manifest-path ${SKILL_FRAMEWORK_ROOT}/rust_tools/pdf_tool_rs/Cargo.toml --bin pdf -- read <input.pdf> --json
cargo run --manifest-path ${SKILL_FRAMEWORK_ROOT}/rust_tools/pdf_tool_rs/Cargo.toml --bin pdf -- info <input.pdf> --json
```

生产 / 重复调用：

```bash
pdf read <input.pdf> --json
pdf info <input.pdf> --json
```

### batch 路径（性能关键）

**硬约束：batch 禁止 `cargo run`。** 必须先 `install-pdf-tool.sh` 或 `just install-pdf`，再调用 PATH 上的 `pdf`。

manifest 示例（`paths.json`）：

```json
{
  "paths": [
    "papers/a.pdf",
    "papers/b.pdf"
  ]
}
```

或 JSON 字符串数组。

```bash
pdf batch --manifest paths.json \
  --out-dir artifacts/current/<task_id>/pdf-batch \
  --jobs auto --resume --skip-scanned

# stdin 路径列表
printf '%s\n' papers/*.pdf | pdf batch --stdin-paths \
  --out-dir artifacts/current/<task_id>/pdf-batch
```

输出布局（`--out-dir` 下）：

| 文件 | 用途 |
| --- | --- |
| `catalog.json` | 批处理汇总；下游 gate 主入口 |
| `results.jsonl` | 逐文件追加结果 |
| `checkpoint.json` | `--resume` 检查点 |
| `index.md` | 人类可读索引表 |
| `text/<sha256>.txt` | 抽取文本片段 |

stdout 为紧凑 `CatalogSummary` JSON（通常 < 4 KB）；完整明细读 `catalog.json`。

**并行与浅扫（batch 性能）**

| 选项 / 环境变量 | 行为 |
| --- | --- |
| `--jobs auto`（默认） | `min(8, CPU 核数)`；batch 内不调 `pdfinfo` 子进程 |
| `PDF_BATCH_JOBS=<N>` | 覆盖 `--jobs`，强制并行度（≥1） |
| `PDF_BATCH_SLOW_FS=1` | 与 `/Volumes`、`/mnt`、`/net` 路径启发式一同将 `auto` 降至 ≤2 |
| `--skip-scanned` | 浅扫前 1–3 页（`SHALLOW_SAMPLE_PAGES`）；无文本层 → `content_class: scanned` 或 `empty`，`status: skipped`，`warnings` 含 `skip_scanned`，不写 `text/<sha>.txt`；有文本则照常全文抽取 |
| `content_class` | `text` / `scanned` / `empty` / `mixed` / `error`；`scanned` 表示抽取窗口内无可读文本（扫描件或纯图页），下游应转 render/OCR |

**`content_class` 判定（全文 `read` 与浅扫共用密度阈值 80 字符）**

| 类 | 含义 |
| --- | --- |
| `text` | 总字符 ≥80 或 平均每页 ≥80 |
| `mixed` | 有文本但低于上述密度（短注、页眉等） |
| `scanned` | 抽取结果为空（典型扫描件 / 无 text layer） |
| `empty` | 0 页 |
| `error` | 加载或抽取失败 |

基准（可选，需 release 编译；未设 `PDF_BENCH=1` 时 bench 二进制立即退出）：

```bash
PDF_BENCH=1 cargo bench -p pdf_tool_rs --bench batch_bench
# 快速冒烟：PDF_BENCH=1 cargo bench -p pdf_tool_rs --bench batch_bench -- --sample-size 10
```

报告写入 `target/criterion/`。

### 版式 render（Poppler）

```bash
pdftoppm -png <input.pdf> <output_prefix>
```

macOS：`brew install poppler`；Debian：`apt-get install -y poppler-utils`。

## Python fallback

仅当 Rust CLI 不可用、或任务需要 **生成 / 复杂编辑**（ReportLab 等）时使用 Python：

```bash
uv add reportlab pdfplumber pypdf
uv sync
```

或一次性：`uvx --with reportlab --with pdfplumber --with pypdf python script.py`

优先级：

1. 抽取 / 批处理 → Rust `pdf`
2. 版式 QA → Poppler render + 图像检查
3. 程序化生成 / 非平凡编辑 → ReportLab / pdfplumber 等 Python 栈

## 核心工作流

### 1. 接入

确认：

- 输入路径或生成目标
- 模式：read / generate / edit / audit / batch
- 版式保真是否重要
- 期望交付物

### 2. 模式选择

#### Read / inspect

- 默认 `pdf read --json`。
- 版式重要时并行或随后 `pdftoppm`。
- 不信仅靠文本抽取证明表格、间距、裁剪正确。

#### Batch

- 用已安装 `pdf batch`；输出收拢到 `artifacts/current/<task_id>/pdf-batch/`。
- 大目录用 `--resume`；扫描件密集目录可加 `--skip-scanned`。
- 将 `catalog.json` 登记到 `EVIDENCE_INDEX.json`。

#### Generate

- 优先 `reportlab` 程序化生成。
- 生成后 render 为图像做 QA。

#### Edit / fix

- 尽量改源或再生管线，而非硬改已损坏 PDF。
- 每次有意义更新后 re-render。

#### Audit

- `pdftoppm` 转 PNG，检查裁剪、重叠、破表、边距、字形、层级。

### 3. 验证 / 复验

- 修复或生成后：re-render 受影响页并检查最新图像。
- 渲染依赖缺失时明确说明剩余风险。

## 输出默认结构

```markdown
## PDF Summary
- Mode: read / batch / generate / edit / audit
- Target: ...

## Findings / Result
- ...

## Render Review
- Pages checked: ...
- Defects found: ...

## Risks / Assumptions
- ...
```

## 硬约束

- 文本抽取不能证明版式正确。
- 版式相关变更不得跳过 render 复验。
- 不要忽略裁剪、重叠、破表。
- 依赖缺失时报告具体阻塞项。
- 生成文本仅使用 ASCII 连字符。

## 触发示例

- 「用 $pdf 检查这个 PDF 渲染是否损坏」
- 「用 $pdf 生成 PDF 并验证渲染页」
- 「读取这个 PDF，检查表格或文字是否错位」
- 「批量抽取 `papers/` 下所有 PDF 文本」
