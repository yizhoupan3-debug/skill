---
parent: docs/spec.md
version: unified-v7
---

## 4. 运行期沙箱契约

### 4.1 生命周期状态机

```
created → warm → busy → draining → recycled → warm
                  ↓         ↓
                failed    failed
```

### 4.2 工具能力策略

类别：`read_only` · `workspace_mutating` · `networked` · `high_risk`

规则：按 Profile 声明 · 高风险独立 Profile · 重用保留边界 · deny-by-default

### 4.3 资源预算

维度：`cpu` · `memory` · `wall_clock` · `output_size`

超限 → `draining` + 持久失败原因。输出溢出不得包装为通用超时。

### 4.4 异步清理与隔离

- `draining` 时启动：释放临时文件/子进程/套接字/句柄（实现**异步清理**）
- 清理 100% 成功 → `recycled`；失败 → `failed`
- 单沙箱崩溃不得污染其他沙箱（进行**故障隔离**，确保 **recoverability boundary**）

---

