# Skill System — 四宿主共用框架

## 一页纸定位

这是一整套给 Claude、Codex、Cursor 和 OpenCode 共用的 skill 系统：`skills/` 技能库、路由运行表、维护脚本、CI 校验和项目级 `AGENTS.md` 规则。**使用者快查**（宿主差异、`REVIEW_GATE`、真源阅读顺序、自检命令）：[`docs/README.md`](docs/README.md) + [`AGENTS.md`](AGENTS.md)。

**宿主闭集**：`codex`、`claude`、`cursor`、`opencode`，由 `RUNTIME_REGISTRY.json` 驱动。文档地图：[`docs/README.md`](docs/README.md)。

## 两条使用路径

| 路径 | 适合 | 最小准备 |
|------|------|---------|
| **A — 只消费 skill 路由（无 hook）** | 只想让 AI 按路由表查 skill | 本仓库根 + `AGENTS.md` + `skills/`；不要求安装 `router-rs` hook 面 |
| **B — 全量 harness（router-rs + hooks）** | 需要 hook 门控、连续性 `artifacts/current/`、证据索引 | 先 `cargo build --release --manifest-path core/router-rs/Cargo.toml`，再按宿主配置 hooks |

路径 B 全量自检：
```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"
```

## 系统组成

- `AGENTS.md` — 四宿主（Claude、Codex、Cursor、OpenCode）共同项目规则（含宿主行为差异附录）
- `docs/README.md` — 文档索引
- `docs/architecture.md` — 八层架构规约（L0-L7 层、DAG 验证矩阵、宿主隔离契约）
- `skills/` — 所有 skill 源文件（`skills/<name>/SKILL.md`）
- `skills/SKILL_ROUTING_RUNTIME.json` — 运行时路由入口（唯一热表）
- `core/router-rs/` — 编译器、校验、路由刷新入口
- `tests/` — 策略与路由约束测试
- `.github/workflows/` — CI 自动校验

## 分享前检查

```bash
git status --short --branch
git diff --stat
git grep -n -I -E "api_key|secret|token|password|私钥|密码" -- .
```

不要分享：`.env`、`artifacts/`、`output/`、`archives/`、`.supervisor_state.json`。`.gitignore` 已覆盖大部分临时目录。

## 日常维护

```bash
# 全量（推荐，等同 /update）
export SKILL_FRAMEWORK_ROOT=/abs/path/to/framework-repo
cargo run --release --manifest-path "${SKILL_FRAMEWORK_ROOT}/core/router-rs/Cargo.toml" -- framework maint update-one-shot

# 技能刷新 + 测试
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework skills refresh --write
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
cargo test --test policy_contracts
```

## 修改或新增 skill

1. 创建 `skills/<skill-name>/SKILL.md`（frontmatter + `## When to use` / `## Do not use`）
2. 手改热路由真源：`skills/SKILL_ROUTING_RUNTIME.json`
3. 再生 companion：`cargo run --manifest-path core/router-rs/Cargo.toml -- framework skills refresh --write --write-companions`
4. 验证：`framework skills validate`；`cargo test --test policy_contracts`

## 常见问题

| 问题 | 答案 |
|------|------|
| Rust 编译慢？ | 首次慢正常，后续复用缓存 |
| Codex 不按 skill 路由？ | 确认工作目录为此仓库根，告知 Codex "先查 SKILL_ROUTING_RUNTIME.json，命中后只读对应 SKILL.md" |
| 可以只复制 `skills/`？ | 不推荐。`AGENTS.md`、编译器、测试、CI 一起克隆最稳 |
| PowerShell 换行？ | 用反引号 `` ` `` 续行；Git Bash 用反斜杠 `\` |

## Hook integration quickstart

```bash
# 宿主 hooks 自检
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-host-hooks --host-id cursor
# 宿主投影安装
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to codex --scope user
# 全局安装 router-rs
cargo install --path core/router-rs --locked --force
```
