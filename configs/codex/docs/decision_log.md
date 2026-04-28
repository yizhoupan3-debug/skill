# Decision Log: OpenAI Plus Aggregation System

## D-1: 核心代理引擎选型 (2026-03-23)

### 候选方案矩阵

| 维度 | CPA (CLIProxyAPI) | chatgpt-to-api | 9Router | OmniRoute |
|------|:-:|:-:|:-:|:-:|
| 语言 | Go | Python | Next.js (JS) | TypeScript |
| 活跃度 | ★★★★★ (3/22 仍在更新) | ★★☆ (2/19 最后更新) | ★★★★ (npm 发布) | ★★★★ (npm 发布) |
| 多账号 RR | ✅ 原生支持 | ❌ 仅单账号 | ✅ 多账号 per provider | ✅ 多账号 per provider |
| OpenAI OAuth | ✅ Codex OAuth 原生 | ❌ 手动 Session Token | ✅ Codex OAuth | ✅ Codex OAuth |
| 多 Provider | ✅ Gemini/Codex/Qwen/iFlow | ❌ 仅 ChatGPT | ✅ 44+ | ✅ 44+ |
| TLS 指纹 | ❌ 无内置 (依赖外部 Sing-box utls) | ✅ curl_cffi Chrome 131 | ✅ wreq-js | ✅ wreq-js + CLI Fingerprint |
| CLI 指纹匹配 | ❌ | ❌ | ❌ | ✅ header/body 排序匹配 CLI 二进制特征 |
| Circuit Breaker | ❌ 需自建 | ❌ | ✅ Combo CB | ✅ per-model CB + Anti-Herd |
| Management API | ✅ 完整 REST API | ✅ /admin/* | ✅ Dashboard | ✅ Dashboard + MCP + A2A |
| Token 自动刷新 | ✅ (配合 codex-auth-refresher) | ✅ Session → Access 自刷 | ✅ 后台自刷 | ✅ 后台自刷 |
| SDK 可嵌入 | ✅ Go SDK | ❌ | ❌ | ❌ |
| Docker 容器化 | ✅ 官方镜像 | ❌ | ✅ | ✅ Docker Hub 多架构 |
| SOCKS5 Proxy | ✅ config.yaml proxy-url | ✅ CHATGPT_PROXY | ✅ HTTP_PROXY | ✅ 3 级代理 + SOCKS5 |
| 启动资源占用 | 低 (Go 二进制 ~20MB) | 低 (Python ~50MB) | 中 (Node.js ~150MB) | 中 (Node.js ~250MB) |
| 生态配套 | ★★★★★ (20+ 衍生项目) | ★☆ (无生态) | ★★★ (OmniRoute fork) | ★★★★ (完整 docs) |

### 决策：采用 CPA + OmniRoute 双轨混合架构

**结论**：不存在单一工具能完美满足所有需求。我们采用**分层混合架构**：

1. **底层代理实例**：继续使用 **CPA**，因为：
   - Go 二进制极轻量，适合每账号一实例的隔离部署
   - 原生 OpenAI Codex OAuth + Management API 最成熟
   - 配合 `codex-auth-refresher` sidecar 实现 Token 自动续期
   - 生态最广、社区最大、出问题最容易找到解法

2. **上层聚合网关**：引入 **OmniRoute** 替代自研 Evolution Server，因为：
   - 内置 per-model Circuit Breaker + Anti-Thundering Herd
   - 内置 CLI Fingerprint Matching（减少账号标记）
   - 内置 TLS Fingerprint Spoofing（wreq-js）
   - 内置 4 层 Fallback + 6 种路由策略
   - 内置 Quota Tracking + Usage Analytics
   - 900+ 自动化测试，工业级稳定性
   - 已有 Docker 多架构镜像，部署简单

3. **隧道层**：保留 **Sing-box** 作为底层网络隧道，因为：
   - 提供 6 独立 SOCKS5 inbound → 6 个 VMess outbound 的流量隔离
   - ⚠️ `utls: chrome` 在 `tls: false` 的 VMess 隧道中**不生效**（见 outline.md 修正12/28/32）
   - TLS 防御实际由 **CPA 原生 Go TLS（≈ Codex CLI 指纹）**承担（Accept Risk）
   - OmniRoute CLI Fingerprint 作为辅助层（影响 OmniRoute→CPA 本地链路，非 CPA→OpenAI）

---

## D-2: 废弃自研 Evolution Server 聚合层 (2026-03-23)

**原因**：
- 自研的高斯抖动漏桶 / Circuit Breaker / Token 监控均已被 OmniRoute 原生覆盖
- OmniRoute 的 MCP 和 A2A 协议支持远超自研方案的扩展性
- 维护一个自研 FastAPI 聚合器 vs 使用有 900+ 测试的成熟开源方案 = 明显不经济

**保留 Evolution Server 的部分**：
- Auth Jump Page (防泄密跳转页) 仍有价值，但可迁移为 OmniRoute 的自定义路由或 Electron App 的一部分

---

## D-3: chatgpt-to-api 降级为备选分支 (2026-03-23)

**优势记录** (留作逃生舱口)：
- Codex Responses API 端点 `/backend-api/codex/responses` 绕过 Cloudflare Turnstile
- `curl_cffi` Chrome 131 TLS 指纹极致伪装
- 当 CPA 的 OAuth 方式被封堵时，可作为 Plan B

**劣势导致降级**：
- 仅支持单账号，无多账号隔离
- Python 实现，与 Go (CPA) + Node.js (OmniRoute) 技术栈不匹配
- Session Token 30天就过期，长期稳定性不如 OAuth Refresh Flow
- 2026-02-19 后无更新，维护前景不明

---

## D-4: codex-auth-refresher 纳入必选组件 (2026-03-23)

**理由**：
- 解决 CPA 最致命的短板：Token 24h 过期导致凌晨断连
- 提前 6h 刷新 Token（`CODEX_REFRESH_BEFORE=6h`）
- 支持强制定期刷新（`CODEX_REFRESH_MAX_AGE=20h`）
- 内置 Email 告警（degraded / reauth_required / invalid_json）
- Docker sidecar 部署，与 CPA 共享 auth 目录，零侵入
- Go 语言与 CPA 技术栈一致

---

## D-5: IP 隔离策略升级 (2026-03-23)

**原方案**：Sing-box 多节点 + CPA 多实例
**升级后**：保持不变，但引入 OmniRoute 的「CLI Fingerprint Matching」作为第三层防护

新三层防封架构（修正51：同步 outline.md 修正12/28/32/55）：
1. **物理层**：Sing-box per-instance SOCKS5 IP 隔离 + CPA 原生 Go TLS ≈ Codex CLI 指纹（`utls:chrome` 在 `tls:false` VMess 隧道中不生效，Accept Risk）
2. **协议层**：OmniRoute CLI Fingerprint Matching（影响 OmniRoute→CPA 本地链路，对 CPA→OpenAI 无直接防封价值）
3. **行为层**：OmniRoute 内置 Rate Limiter + Jitter + Circuit Breaker（流量整形）
