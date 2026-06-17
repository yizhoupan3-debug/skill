---
last_verified: "2026-06-16"
depends_on:
  - ../spec.md
  - ../../skills/pdf/SKILL.md
  - ../../skills/doc/SKILL.md
  - ../../skills/primary-runtime/spreadsheets/SKILL.md
  - ../../skills/slides/SKILL.md
---

# Office 文档 CLI（pdf / ooxml / ppt）

Rust-first 阅读与批处理真源。Skill 路由（`pdf` / `doc` / `spreadsheets` / `slides`）在会话开始即生效；**二进制需本机一次性安装**，不会随 `router-rs` 或 host hook 自动下发。

## 安装（推荐）

在仓库根执行（honors 根目录 [`.cargo/config.toml`](../../.cargo/config.toml) 的 `target-dir`）：

```bash
bash scripts/install-pdf-tool.sh
bash scripts/install-ooxml-tool.sh
bash scripts/install-ppt-tool.sh
# 或三条合一（需本机有 `just`）：
just install-office-tools
```

默认安装到 `~/.local/bin/`。可覆盖：

| 变量 | 默认 | 二进制 |
|------|------|--------|
| `PDF_BIN_DIR` | `~/.local/bin` | `pdf` |
| `OOXML_BIN_DIR` | `~/.local/bin` | `ooxml` |
| `PPT_BIN_DIR` | `~/.local/bin` | `ppt` |

安装后确认 PATH：

```bash
export PATH="$HOME/.local/bin:$PATH"
which pdf ooxml ppt
```

## 命令速查

| 格式 | 单文件阅读 | 批量阅读 | Skill |
|------|-----------|---------|-------|
| PDF | `pdf read <f> --json` | `pdf batch --stdin-paths --out-dir …` | `skills/pdf` |
| DOCX | `ooxml read-docx <f>` | `ooxml batch …`（与 xlsx 混批） | `skills/doc` |
| XLSX | `ooxml read-xlsx <f>` | 同上 | `skills/spreadsheets` |
| PPTX | `ppt read-full <f>` | 暂无 | `skills/slides` |

批处理并行度：`PDF_BATCH_JOBS` / `OOXML_BATCH_JOBS`；产物目录约定见各 skill frontmatter（`pdf-batch/`、`ooxml-batch/`）。

## 开发 vs 生产

- **单文件探测**：可用 `cargo run --manifest-path rust_tools/<crate>/Cargo.toml --bin <name> -- …`
- **batch**：必须用 PATH 上的已安装二进制（禁止 `cargo run` batch，冷启动会拖垮并行）

## 版式 / 渲染依赖（可选）

| 任务 | 系统工具 |
|------|----------|
| DOCX/XLSX 渲染 QA | LibreOffice (`soffice`)、Poppler |
| PPTX 渲染 / QA | 同上 + `ppt render` |
| 扫描 PDF 版式判断 | `pdftoppm`（Poppler）；`content_class: scanned` 时不要仅靠 `pdf read` |

macOS：`brew install --cask libreoffice poppler`

## 自检

```bash
pdf read --help
ooxml batch --help
ppt read-full --help
```

未安装时 agent 可能 fallback 到 `cargo run`（仅单文件）；批处理会失败或不应启动。

## 清理编译产物

Workspace 构建输出在根目录 [`.cargo/config.toml`](../../.cargo/config.toml) 指定的 `target-dir`（默认 `/tmp/skill-cargo-target`），不在仓库内。

```bash
just clean
# 或：
cargo clean && rm -rf target target-router-rs-subagent
```

清理后 `~/.local/bin` 里**实体拷贝**的 `pdf`/`ooxml` 仍可用；若 `ppt` 是指向 `target-dir` 的符号链接，需重新 `bash scripts/install-ppt-tool.sh`。
