---
last_verified: "2026-06-19"
---

# 备份、恢复与卸载

## 备份优先级

| 路径 | 重要性 | 说明 |
|------|--------|------|
| 仓库内宿主投影（`.claude/`、`.cursor/`、`.codex/`、`.opencode/`、`.gemini/`） | 高 | 建议 Git 管理 |
| `artifacts/current/<task_id>/` | 中 | 进行中的 goal / RFV / wave |
| `~/.local/share/skill-framework/bin/router-rs` | 低 | 可重编译 |
| `artifacts/telemetry/` | 中 | evolution 分析输入 |

具体投影目录名以 [`RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json) 与各宿主手册为准。

## 恢复

1. `git clone` / `git pull` 恢复仓库与投影文件  
2. `cargo build --release` 重建 `router-rs`  
3. `framework host-integration install --to <host_id>` 刷新 MCP / hooks  
4. `framework doctor` 确认无 drift WARN  

任务级状态：从 `artifacts/current/<task_id>/` 恢复 `GOAL_STATE.json` 等后，用 `framework_goal_drive` resume（见 B3）。

## 卸载框架投影（仓库内）

按所用宿主删除对应投影目录与 hook 配置；**不要**删除整个仓库除非放弃项目。

通用模式（细节见 `host-integration` 与各宿主手册）：

```bash
# 示例：移除 Cursor project hook 面（保留手维护 rules 时请只删列出的文件）
rm -rf .cursor/hooks.json .cursor/router-rs-hook.env .cursor/hook-state/

# 可选：移除稳定二进制
rm -f ~/.local/share/skill-framework/bin/router-rs
```

卸载前备份 `artifacts/current/` 与未提交的宿主 `settings.local.json`。
