# Pilot Matrix: OpenAI Plus 聚合系统

## 候选分支与试点队列

| Branch ID | 假设 (Hypothesis) | 范围 (Scope) | 预期产物 | 成功标准 | Kill Rule | 最大预算 | 执行 Owner |
|-----------|-------------------|-------------|---------|---------|-----------|---------|-----------|
| B1-singbox | **[假设修订-修正45]** CPA 原生 Go TLS 指纹 ≈ Codex CLI 指纹，OpenAI 不会封自家 CLI 的 TLS 签名（注：utls:chrome 在 VMess tls:false 隧道中不生效-修正12/28/32） | 启动 Sing-box + CPA Instance-1，分别获取 CPA 和 Codex CLI 的 JA3 hash | CPA JA3 与 Codex CLI JA3 对比报告 | CPA 和 Codex CLI JA3 hash 一致（允许 ≤3 字段差异） | CPA JA3 与 Codex CLI 差异 > 3 字段 **且** OpenAI 实测拒绝请求 → Kill，启用 Plan B | 30 min | plan-to-code |
| B2-cpa-iso | 6 CPA 实例 + 独立 SOCKS5 能实现完全 IP 隔离 | 6 个 CPA 实例分别绑定不同 SOCKS5 | 6 个 `api.ipify.org` 返回完全不同的 IP | 6 IP 两两不同，且均为代理节点 IP | 任何一个返回本机真实 IP → Kill | 15 min | plan-to-code |
| B3-omniroute-cb | OmniRoute Circuit Breaker 能在 CPA 故障时正确熔断并 fallback | 模拟 CPA-A 宕机，观察 OmniRoute 行为 | OmniRoute 自动切换到 CPA-B 的日志证据 | 请求在 CPA-A 宕机后 < 10s 内自动路由到 CPA-B | OmniRoute 持续报错不切换 → 架构不可行 | 20 min | plan-to-code |
| B4-auth-refresh | codex-auth-refresher 能在 Token 临期前自动刷新 | 部署 sidecar，观察 Token 刷新日志 | 日志显示 Token 在过期前被成功刷新 | `CODEX_REFRESH_BEFORE=6h` 触发刷新 | 连续 3 次刷新失败 → 检查 OAuth 端点 | 60 min (需等刷新窗口) | plan-to-code |
| B5-cli-fp | **[修正55-对齐 outline.md 修正126-129]** OmniRoute CLI Fingerprint Matching 对 OmniRoute→CPA 本地链路生效，但对 CPA→OpenAI 链路无直接防封作用（与 outline.md §"自审查修正"一致） | 对比开启/关闭时 OmniRoute→CPA 的请求 header 顺序 | 抓包对比报告（本地链路） | 抓包确认 OmniRoute→CPA 的请求 header 顺序匹配配置，且 Codex App 端行为正常 | 该特性对 OpenAI 无防封价值（已知）→ 标记为可选优化，不阻止主方案继续，B5 不是 Go/No-Go 项 | 15 min | plan-to-code |
| B6-e2e-24h | 完整系统持续运行 24h 稳定性验证 | 全系统 + 定时请求脚本 | 24h 无中断的请求成功率报告 | 成功率 > 99.5%，无任何 Token 过期中断 | 成功率 < 95% → 架构需要重大调整 | 24h | execution-controller |

## 试点优先级

1. **B1-singbox** + **B2-cpa-iso** (可并行) → 基础设施验证
2. **B4-auth-refresh** → Token 续期验证 (需要等待刷新窗口)
3. **B3-omniroute-cb** + **B5-cli-fp** (可并行) → 网关特性验证
4. **B6-e2e-24h** → 最终稳定性验证 (所有前置通过后)

## 分支汇聚策略

- **Promote**: B1-B5 全部通过 → 汇入主方案，执行 B6
- **Kill B5 alone**: B5 不生效但其他通过 → 仍可接受，降为可选优化
- **Kill B1 or B2**: TLS/IP 隔离不可行 → 需重新评估 Sing-box 替代品（如 Xray-core）
- **Kill B3**: OmniRoute CB 不工作 → 降级回自研 Circuit Breaker in Evolution Server
- **Kill B4**: Token 刷新失败 → 改为 crontab 定时手动刷新方案
