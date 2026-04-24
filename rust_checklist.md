# Rust 化执行清单

> 状态：已关闭。本文不再是活跃执行清单，只保留上一轮 Rust 化的收口结论和防回退规则。

## 当前事实

- `framework_runtime/` Python 包已退场；仓库内不再保留 Python runtime 主面。
- 路由、runtime snapshot、contract summary、memory recall、session artifact 写入、prompt/memory policy、host entrypoint sync、native install/bootstrap 都走 `scripts/router-rs/`。
- `scripts/route.py`、`scripts/materialize_cli_host_entrypoints.py`、`scripts/install_codex_native_integration.py`、`scripts/write_session_artifacts.py`、`scripts/framework_hook_bridge.py` 等旧 Python wrapper 已删除。
- OMC / `.omc/**` 不再是 runtime 依赖；`autopilot`、`deepinterview`、`team` 是 framework-native skill/command 投影。
- `codex_desktop_host_adapter`、`aionrs_companion_adapter`、`aionui_host_adapter` 只属于显式 compatibility/inventory 语境，不进入默认 host peer set。

## 已完成范围

| Lane | 结果 |
| --- | --- |
| Route facade | 旧 Python route shim 删除，route/search/report/policy 由 `router-rs` 负责。 |
| Runtime control plane | runtime/control/trace/memory/background authority 由 Rust surface 发布。 |
| Registry / host entrypoints | host entrypoint materialization 与 native install 归入 Rust host-integration。 |
| Hooks / artifacts | hook projection 与 session artifact writer 归入 `router-rs`。 |
| Continuity / policy | memory recall、memory extraction、prompt compression、continuity snapshot 归入 Rust。 |
| Python 残余清点 | 仓库级 Python source/cache/pytest entrypoints 已退场，仅保留外部工具生态描述中的语言名。 |

## 防回退规则

- 不新增 `framework_runtime/` Python 包。
- 不新增 `scripts/*.py` 作为 runtime、routing、host integration、artifact writer、hook bridge、memory policy 的实现面。
- 不把 `rust_execute_fallback_to_python`、Python live fallback、Python/Rust parity report 写回当前能力描述。
- 不把历史 checklist 当作活跃任务恢复；新工作应开新清单，并以 `router-rs` / Rust tool surfaces 为写入范围。

## 后续方向

活跃工作不再是“继续搬 Python”，而是增强已 Rust-owned 的能力面：

- remote-capable event transport / attach handoff
- backend-family persistence 与 compaction
- sandbox lifecycle control plane
- skill compiler / routing health
- Rust CLI 工具体验和文档一致性

对应的新工作见 `rust_next_phase_checklist.md`。
