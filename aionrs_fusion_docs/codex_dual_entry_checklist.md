# Codex 双入口执行清单

> 这是当前四份文档的总入口。它服务于已经落地的战略前提：
> 放弃 `aionrs` 和 `AionUI` 作为目标宿主，专注 `Codex Desktop + codexcli` 双入口；
> `codex_dual_entry_parity_snapshot` 是主回归基线，`codex_desktop_host_adapter` 只保留为 compatibility-only mirror。

## 0. 最高优先级硬约束

- 不再围绕 `aionrs` 继续投资新方案
- 不再围绕 `AionUI` 继续投资新方案
- 不让 `codexcli` 变成框架真源
- 不把框架主控下沉成单一宿主私有控制器

所有新实施只能落在：

- 外层 framework core
- Codex common / desktop / cli adapters
- contract / artifacts / bridge
- docs / tests

## 1. 文档分工

### `codex_dual_entry_outline.md`

用途：

- 作为总蓝图和架构决策文档
- 定义为什么主控仍然是 framework core，而不是 `codexcli`
- 定义 `Codex Desktop + codexcli` 双入口的层次边界
- 标记哪些现有 `aionrs` / `AionUI` 资产属于 legacy migration debt

### `codex_dual_entry_rust_checklist.md`

用途：

- 作为当前阶段的 canonical checklist
- 聚焦 Codex-only 方案下已经落地的基线和剩余清理项
- 明确 Rust 只继续做 contract / artifact / parity lane

### `codex_dual_entry_next_phase_checklist.md`

用途：

- 作为下一阶段执行蓝图
- 聚焦 Desktop / CLI parity、adapter 拆分、legacy deprecation
- 明确 Rust 化不进入底层 runtime 魔改

## 2. 当前总判断

- [x] 方案已切换成 `Codex Desktop + codexcli` 双入口
- [x] framework core 继续是唯一真源
- [x] `codex_common_adapter` 已是 Desktop / CLI 共享投影层
- [x] `codex_desktop_adapter` 已是 canonical interactive desktop entrypoint
- [x] `codex_desktop_host_adapter` 已降级为 compatibility-only mirror alias
- [x] `codex_cli_adapter` 已是 formal headless entrypoint
- [x] `codex_dual_entry_parity_snapshot` 已是主回归基线
- [x] 需要把现有 `aionrs` / `AionUI` 适配面降级为 legacy migration debt

## 3. 当前执行顺序

1. 先读 `codex_dual_entry_outline.md`
   目标：确认总架构、主控边界、Desktop / CLI 分工

2. 再看 `codex_dual_entry_rust_checklist.md`
   目标：确认基线复用面、废弃面、Rust 边界

3. 最后看 `codex_dual_entry_next_phase_checklist.md`
   目标：确认下一阶段的 workstream、优先级和并行矩阵

## 4. 当前优先级最高的事项

- [x] 冻结“主控不是 `codexcli`，而是 framework core”的结论
- [ ] 清理仍滞后于现状的 checklist / outline 叙事
- [x] 为 `codex_desktop_host_adapter` 定义明确 alias retirement gate
- [ ] 继续把旧的 `aionrs` / `upgrade_compatibility_matrix` 主线叙事改写为 parity-snapshot-first
- [ ] 决定 `upgrade_compatibility_matrix` 只保留为 secondary compatibility inventory 还是继续收缩
- [ ] 继续清理 Rust / Python / docs 中残留的 legacy 主线措辞
- [ ] 启动下一轮 runtime control-plane 深化，优先做 run-manager / stream /
  persistence seams，而不是直接 runtime rewrite

## 5. 当前明确不做的事

- [x] 不继续规划 `aionrs` companion lane
- [x] 不继续规划 `AionUI` host lane
- [x] 不让 `codexcli` 接管 framework core
- [x] 不为了 CLI 入口破坏 Desktop / CLI 双入口共用 contract
- [x] 不直接进入底层 runtime 魔改

## 6. 当前基线判断

当前代码里仍然存在可复用的资产：

- [x] `framework_profile` 仍然可作为唯一外层 contract 真源
- [x] `codex_common_adapter` 已是 shared projection layer
- [x] `codex_desktop_adapter` 已是 desktop lane 正式入口
- [x] `codex_desktop_host_adapter` 仅作为 compatibility-only mirror alias 保留
- [x] `codex_cli_adapter` 已是 CLI lane 正式入口
- [x] `generic_host_adapter` 可以作为 common fallback 语义的迁移起点
- [x] Rust `router-rs` 的 profile compiler / projection 能继续作为 contract emission 基线
- [x] `codex_dual_entry_parity_snapshot` 已是主 regression baseline

当前代码里也存在需要降级处理的 legacy 资产：

- [ ] `aionrs_companion_adapter`
- [ ] `aionui_host_adapter`
- [ ] 任何仍把 `upgrade_compatibility_matrix` 写成主回归基线的叙事
- [ ] 任何继续假设 `aionrs` 是未来执行核的文档和测试

## 7. 成功定义

如果这一套 Codex-only 双入口方案推进正确，最终应该满足：

- `Codex Desktop` 和 `codexcli` 共同消费同一份 outer contract
- framework core 始终是主控，不被 Desktop 或 CLI 反向绑死
- `codex_desktop_adapter` 负责交互、thread、automation bridge
- `codex_desktop_host_adapter` 只作为 compatibility-only mirror，不再扩成正式 API 面
- CLI lane 负责 batch、headless、cron、CI、脚本化执行
- `codex_dual_entry_parity_snapshot` 是主回归基线，`upgrade_compatibility_matrix` 最多只保留 secondary inventory / smoke 角色
- Rust 化继续优先聚焦“contract emission / parity / regression”，并逐步向
  runtime-adjacent control-plane seams 收口，而不是直接 runtime rewrite
