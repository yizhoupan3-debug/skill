# OMC 退场清单

> 状态：已关闭。本文只保留 OMC 替代工作的最终边界，不再作为活跃执行清单。

## 当前事实

- OMC / `oh-my-claudecode` 不是本仓 runtime 依赖、host prompt truth、plugin dependency 或兼容内核。
- `.omc/**` 不再作为状态目录；仓库当前也没有 `.omc` live surface。
- `autopilot`、`deepinterview`、`team` 是 framework-native skills 和 host command aliases，不复制 OMC prompt、agent catalog 或 runtime state。
- shared supervisor、rate-limit resume、tmux worker/session 管理等能力落在 `scripts/router-rs/`，不是 OMC wrapper。

## 已完成范围

- OMC 退场边界写入 `docs/host_adapter_contracts.md` 与 `docs/rust_contracts.md`。
- session supervisor / background state / rate-limit resume 方向已进入 Rust control-plane。
- Claude/Codex/Gemini entrypoint 通过 Rust host-entrypoint sync 投影。
- 旧 Python materializer / runtime wrapper / artifact writer 已删除。

## 防回退规则

- 不重新安装、vendor、代理或包装 OMC。
- 不把 OMC 命令名写成 canonical contract id。
- 不把 `.omc/**` 恢复成 runtime state、resume、trace 或 artifact 真源。
- 不新增 `scripts/*.py` 作为 OMC 替代层；需要新能力时进入 `router-rs` 或对应 Rust tool。

## 后续

后续如果发现全局宿主目录仍有个人历史残留，应按 host-private 清理处理，不写回本仓 shared runtime contract。
