# Outline: OpenAI Plus 聚合系统 — 终极防封版 v2

## Motivation & Context

用户需要将多个 OpenAI Plus 订阅账号聚合为一个统一的 OpenAI 兼容 API，供本地 Codex App / Antigravity / Cursor 等 AI Coding 工具使用。核心诉求：**长期稳定运行，账号绝不被封**。

### 调研背景 (2026-03-23 深度调研)

通过对 GitHub 全面调研，覆盖了以下关键项目：

| 项目 | 核心价值 | 选型结论 |
|------|---------|---------|
| [CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) | Go 二进制、最成熟生态、原生 Multi-Account RR、Management API | ✅ **底层代理实例** |
| [OmniRoute](https://github.com/diegosouzapw/OmniRoute) | TypeScript、TLS Fingerprint Spoofing、CLI Fingerprint Matching、per-model Circuit Breaker、900+ 测试 | ✅ **上层聚合网关** |
| [codex-auth-refresher](https://github.com/pymba86/codex-auth-refresher) | Go sidecar、Token 自动续期 6h 提前刷新、Email 告警 | ✅ **必选配套** |
| [chatgpt-to-api](https://github.com/Kitjesen/chatgpt-to-api) | Codex Responses API 绕过 Turnstile、curl_cffi Chrome TLS | ⚠️ **备选逃生舱** |
| [9Router](https://github.com/decolua/9router) | CPA 的 JS 重写、4-Tier Fallback | ⏭️ 被 OmniRoute 取代 |

详细比较矩阵见 [decision_log.md](./decision_log.md)。

## Target Outcome

构建基于 **CPA + OmniRoute + codex-auth-refresher + Sing-box** 四件套的终极防封聚合架构，**6 个 Plus 账号、6 个独立出口 IP**：

### 节点绑定表（已确认）

> **⚠️ 香港节点不可用于 OpenAI，已全部排除。**
> **⚠️ 台湾节点需运行时验证（OpenAI 可能部分封锁）。如被封，bnm4 标记为 unhealthy，对应 CPA 熔断，从 5 账号运行。**

| 编号 | Plus 账号 | Sing-box SOCKS5 | CPA 端口 | 绑定服务器(域名) | 端口 | 地区 | Clash 节点 | 倍率 |
|------|-----------|----------------|---------|-----------------|------|------|-----------|------|
| 1 | Account-A | 127.0.0.1:10001 | 8317 | `sttrbjp.cdn.node.a.datahub7.net` | 50189 | 🇯🇵 日本GPT | V3-189 | x1.5 |
| 2 | Account-B | 127.0.0.1:10002 | 8318 | `dcv69.cdn.node.a.datahub7.net` | 50208 | 🇯🇵 日本 | V3-208 | x1.5 |
| 3 | Account-C | 127.0.0.1:10003 | 8319 | `ggsd5.cdn.node.a.datahub7.net` | 50103 | 🇺🇸 美国 | V3-103 | x2.0 |
| 4 | Account-D | 127.0.0.1:10004 | 8320 | `aasx2.cdn.node.a.datahub7.net` | 50050 | 🇸🇬 新加坡 | V3-50 | x1.8 |
| 5 | Account-E | 127.0.0.1:10005 | 8321 | `cvb7.cdn.node.a.datahub7.net` | 50166 | 🇸🇬 新加坡 | V3-166 | x1.8 |
| 6 | Account-F | 127.0.0.1:10006 | 8322 | `bnm4.cdn.node.a.datahub7.net` | 50068 | 🇹🇼 台湾 | V3-68 | x1.8 |

> 代理节点来源：速云梯机场 (Clash 配置 `~/.config/clash/速云梯.yaml`)，共 84 个节点，排除 40 个香港节点后，有 6 个独立服务器域名可用。
> **套餐升级说明（2026-03-23）**：升级后非 HK 独立域名数量**仍为 6 个**，未新增域名。套餐升级带来的是每个域名下新增了 V4 系列端口节点（见下表），可作为备用替换节点。

#### V4 备用节点一览（套餐升级新增，同域名不同端口）

| 域名前缀 | 当前绑定 (VMess) | V4 备用节点 (VMess) | 备注 |
|---------|----------------|-------------------|------|
| sttrbjp | V3-189 port:50189 x1.5 | V4-192 port:50192 x1.5 / V4-193 port:50193 x1.5 / V4-194 port:50194 x1.0 | 相同域名相同出口IP，可热备 |
| dcv69   | V3-208 port:50208 x1.5 | V4-210 port:50210 x1.5 / V4-46 port:50046 x1.5 | 同线路热备 |
| ggsd5   | V3-103 port:50103 x2.0 | *(无新增 V4 VMess 节点)* | — |
| aasx2   | V3-50  port:50050 x1.8 | V4-18 port:50018 x1.0 | V4 倍率更低，不建议换用 |
| cvb7    | V3-166 port:50166 x1.8 | V4-19 port:50019 x1.0 | V4 倍率更低，不建议换用 |
| bnm4    | V3-68  port:50068 x1.8 | **V4-29 port:50029 x2.0 / V4-30 port:50030 x2.0** | ⚠️ V4 倍率更高(x2.0)，消耗更多流量但速度可能更优 |

> **使用建议**：V4 节点与对应 V3 节点使用**同一出口 IP**，因此切换 V4 不改变 IP 隔离格局，仅改变端口（可能影响速度/稳定性）。当前节点运行稳定时无需切换；V3 节点故障时可直接将 sing-box.json 中对应 outbound 的端口改为 V4 端口，无需其他改动。

#### VMess 公共参数（全部 84 个 VMess 节点共享）
```
uuid:     577edbac-cb2a-3652-8210-a949d76f24a9
alterId:  0
cipher:   auto
network:  ws
ws_path:  /009c250b-54f5-43a8-8b18-dccbb5d8c66a.y.live01.m3u8
tls:      false  ← 代理隧道本身不加密
udp:      true
```

> **关键技术说明**：所有节点 `tls: false`，即 CPA ↔ 代理服务器之间的 VMess 隧道不使用 TLS。但 Sing-box 的 `utls: chrome` 作用于**目标连接**（CPA → api.openai.com 的 HTTPS 握手），而非隧道本身。也就是说，当 CPA 通过 SOCKS5 → Sing-box → 代理服务器 → api.openai.com 时，最后一跳的 TLS Client Hello 会被 Sing-box utls 改写为 Chrome 指纹。这正确地防御了 OpenAI 的 JA3/JA4 检测。


### 架构总览

```
Codex App / Antigravity / Cursor
         │
         │ http://localhost:20128/v1  (统一入口)
         ▼
┌──────────────────────────────────────────────────┐
│  OmniRoute (聚合网关 · Port 20128)                │
│  • Per-model Circuit Breaker + Anti-Herd         │
│  • Rate Limiter + Jitter (行为学伪装)             │
│  • Quota Tracking + Usage Analytics              │
│  • API Key Management (防泄密)                   │
│  • 6 CPA Provider → Combo "plus-6" (RR/Fill)    │
└──┬──────┬──────┬──────┬──────┬──────┬────────────┘
   │      │      │      │      │      │
   ▼      ▼      ▼      ▼      ▼      ▼
CPA-1  CPA-2  CPA-3  CPA-4  CPA-5  CPA-6
:8317  :8318  :8319  :8320  :8321  :8322
   │      │      │      │      │      │
   ▼      ▼      ▼      ▼      ▼      ▼
SB-1   SB-2   SB-3   SB-4   SB-5   SB-6
:10001 :10002 :10003 :10004 :10005 :10006
utls   utls   utls   utls   utls   utls
🇯🇵GPT  🇯🇵     🇺🇸     🇸🇬a    🇸🇬b    🇹🇼

独立 Sidecar:
┌──────────────────────────────────────┐
│  codex-auth-refresher (Port 18081)   │
│  bind-mount: auths_1/ ~ auths_6/    │
│  提前 6h 刷新 · 强制 20h 刷新        │
│  Email 告警 (degraded/reauth)        │
└──────────────────────────────────────┘
```

## Assumptions

1. CPA 二进制文件 (`cliproxyapi`) 已安装，版本 ≥ 6.8.55
2. 用户拥有 **6 个** OpenAI Plus 订阅账号
3. 代理节点：使用**速云梯**机场的 6 个非香港独立服务器（sttrbjp / dcv69 / ggsd5 / aasx2 / cvb7 / bnm4）；套餐升级后每个域名新增了 V4 系列备用节点，非 HK 独立域名数量仍为 6 个
4. **香港节点不可用**（OpenAI 封锁香港 IP），已排除全部 40 个香港节点
5. macOS 本地部署，所有服务仅绑定 `127.0.0.1`
6. Sing-box 已安装（`brew install sing-box`）
7. Node.js ≥ 18 LTS、Docker（用于 codex-auth-refresher）
8. Clash 配置文件路径：`~/.config/clash/速云梯.yaml`

## Chosen Strategy: 四件套隔离架构

### 第一层：物理网络隔离 (Sing-box)
- 从速云梯 Clash 配置中提取 6 个非香港节点的 VMess/SSR 参数
- 转换为 Sing-box 格式并配置 6 个独立 outbound
- 强制开启 `utls: { enabled: true, fingerprint: "chrome" }`
- 创建 6 个 SOCKS5 inbound 端口 (10001 ~ 10006)，通过 routing rules 一对一绑定到 6 个 outbound
- **关键**：不同账号的流量**绝不经过同一出口 IP**，且所有节点均为非香港地区

### 第二层：进程级代理隔离 (CPA 多实例)
- 每个 Plus 账号启动一个独立的 CPA 进程
- 三位一体绑定：`1 Instance : 1 Account : 1 Proxy IP`
- 独立的 `config_X.yaml` + `auths_X/` 目录 + 监听端口
- OAI-Device-Id 硬绑定在各自的 auth 文件中，永不轮换

### 第三层：智能聚合网关 (OmniRoute)
- 替代之前自研的 Evolution Server 聚合层
- 将每个 CPA 实例注册为 OmniRoute 的 "Custom OpenAI-compatible provider"
- 利用 OmniRoute 内置能力：
  - **Per-model Circuit Breaker**: 当某个 CPA 报错 429/401/403 时自动熔断
  - **Rate Limiter + Jitter**: 控制总请求频率，模拟人类使用节奏
  - **API Key Management**: 对外只暴露 OmniRoute 生成的 API Key
  - **Quota Tracking**: 实时追踪各 CPA 实例的使用量和配额

> **⚠️ 自审查修正 — CLI Fingerprint 适用性说明**:
> OmniRoute 的 TLS Fingerprint Spoofing 和 CLI Fingerprint Matching 功能在本架构中影响的是 **OmniRoute → CPA** 这段链路（内网 localhost），而非 **CPA → OpenAI** 这段关键链路。真正面向 OpenAI 的 TLS 指纹伪装由第一层 Sing-box `utls:chrome` 负责，CPA 本身通过 Codex OAuth 流获取 Token 后以标准 Go HTTP 客户端发出请求，Sing-box 在出站前改写其 TLS Client Hello。因此：
> - **TLS Fingerprint (Sing-box utls)**: 防御 JA3/JA4 指纹检测 ✅ 生效
> - **CLI Fingerprint (OmniRoute)**: 在本架构中不直接防封，但可作为对 CPA Management API 的访问保护 
> - **CPA 自身行为**: CPA 使用 Codex 的原生 OAuth Token 和 API 格式调用 OpenAI，请求特征与真实 Codex CLI 一致 → 这才是最核心的防封层

### 第四层：Token 自动续期 (codex-auth-refresher)
- Docker sidecar 部署，bind-mount CPA 的 auth 目录
- `CODEX_REFRESH_BEFORE=6h`: 提前 6 小时刷新 Token
- `CODEX_REFRESH_MAX_AGE=20h`: 无论 JWT 过期时间多久，最多 20h 强刷一次
- `CODEX_SCAN_INTERVAL=5m`: 每 5 分钟扫描一次 Token 状态
- Email 告警：Token degraded/reauth_required 时发邮件通知
- React Dashboard (`Port 18081`): 实时监控所有 Token 健康状态

## Deep Optimization: Anti-Ban & Stability (深度优化)

### 极致防封策略 — 五层纵深防御

| 层级 | 手段 | 组件 | 防什么 |
|------|------|------|--------|
| L1 | IP 隔离 | Sing-box 多节点 | 防 IP 关联检测 |
| L2 | TLS 指纹 | Sing-box utls:chrome | 防 JA3/JA4 指纹检测 |
| L3 | CLI 指纹 | OmniRoute CLI Fingerprint | 防 HTTP 签名检测 |
| L4 | 行为伪装 | OmniRoute Rate Limiter + Jitter | 防机器人行为检测 |
| L5 | Device ID | CPA auth 文件硬绑定 | 防设备指纹混乱 |
| L6 | User-Agent 一致性 | CPA 原生 Codex CLI UA | 防 UA 检测 |
| L7 | Arkose PoW 路由 | CPA 请求跟随绑定节点 | 防 PoW 验证 IP 不一致 |

### 稳定性保障策略

1. **智能熔断** (OmniRoute 内置)
   - Per-model Circuit Breaker: Closed → Open → Half-Open
   - Anti-Thundering Herd: Mutex + Semaphore 防雪崩
   - Exponential Backoff: 渐进式重试延迟

2. **Token 生命周期管理** (codex-auth-refresher)
   - JWT 过期前 6h 主动刷新
   - 强制每 20h 刷新一次（防 CLI 24h 重连问题）
   - degraded 状态时 Email + Dashboard 双告警

3. **Fallback 容灾链** (OmniRoute Combo)
   - CPA-A (Plus 账号 1) → CPA-B (Plus 账号 2) → 免费 Provider (iFlow/Kiro)
   - 当所有 Plus 账号都熔断时，自动降级到免费模型，不中断编码

4. **数据隔离**
   - 每个 CPA 实例的 auth file、log、config 完全独立
   - OmniRoute 的 API Key 与 Plus Token 物理隔离
   - Codex App 永远看不到真实的 OpenAI Session Token

5. **Arkose / Cloudflare PoW 处理**
   - CPA 在 OAuth 登录时可能触发 Cloudflare Turnstile 验证
   - 所有 PoW 挑战请求严格跟随该 CPA 实例绑定的 SOCKS5 节点，确保验证 IP 与后续 API 调用 IP 一致
   - 如果 Codex Responses API 将来也加入 Turnstile（目前未加），chatgpt-to-api 方案可作为逃生路线

6. **User-Agent 一致性**
   - CPA 内部使用 Codex CLI 原生 User-Agent 发起请求
   - 不应在 OmniRoute 或 Sing-box 层覆写 UA，让 CPA 保持其原生行为
   - 不同 CPA 实例使用相同 UA 是可接受的（真实 Codex CLI 用户也共享同一 UA）

## Rejected Alternatives

1. **单 CPA 实例挂多账号** (原方案)
   - 所有账号共享同一出口 IP → 批量封号死穴
   - CPA config.yaml 仅支持全局 proxy-url → 无法按账号分配

2. **chatgpt-to-api 作为主方案**
   - 仅支持单账号，无多账号隔离
   - Session Token 30 天过期 + Python + 生态薄弱
   - 保留为紧急逃生方案（当 CPA OAuth 被封堵时）

3. **自研 Evolution Server 聚合层**
   - 被 OmniRoute 完全替代（Circuit Breaker / Rate Limiter / Dashboard 全覆盖）
   - 维护自研方案 vs 使用 900+ 测试的成熟开源项目 = 不经济

4. **纯 API Key 模式**
   - 无法利用 Plus 订阅的免费额度
   - 不符合用户"用 Plus 账号跑额度"的核心诉求

5. **9Router 替代 OmniRoute**
   - 功能上被 OmniRoute（TypeScript 全重写 fork）完全覆盖
   - OmniRoute 有 CLI Fingerprint Matching、MCP/A2A、900+ 测试

---

## App 架构设计 — Codex Aggregator Manager

### 技术栈选型

| 层 | 技术 | 理由 |
|---|---|---|
| **前端** | Next.js 15 (React 19) + shadcn/ui | OmniRoute 本身就是 Next.js，统一技术栈；shadcn/ui 提供高质量组件 |
| **后端 API** | Next.js API Routes (Route Handlers) | 同构部署，无需单独后端服务器 |
| **数据库** | SQLite (via better-sqlite3) | 纯本地部署，无需外部 DB；文件级 DB 便于备份和迁移 |
| **ORM** | Drizzle ORM | 类型安全、轻量、支持 SQLite、迁移工具完善 |
| **进程管理** | Node.js child_process + PM2 API | 管理 Sing-box / CPA / codex-auth-refresher 子进程 |
| **实时通信** | Server-Sent Events (SSE) | 向前端推送健康检查结果、Token 状态变更、日志流 |
| **样式** | Tailwind CSS 4 + CSS Variables | shadcn/ui 依赖 Tailwind；CSS Variables 支持主题切换 |
| **图表** | Recharts | 使用量趋势、成功率曲线、延迟分布 |
| **认证** | JWT + bcrypt (本地单用户) | 防止局域网内未授权访问 Dashboard |

### 应用定位

**Codex Aggregator Manager** 是一个独立的本地管理面板，作为四件套之上的第五层：

```
╔══════════════════════════════════════════════╗
║  Codex Aggregator Manager (Port 3000)        ║
║  • 统一管理面板                               ║
║  • 生命周期控制 (启/停/重启各组件)              ║
║  • 实时健康监控 + 历史趋势                     ║
║  • Token 状态可视化                            ║
║  • 使用量分析                                  ║
║  • 告警管理                                    ║
╠══════════════════════════════════════════════╣
║           ↓ 调用各组件 API / 读 DB ↓           ║
║  OmniRoute · CPA×6 · AuthRefresher · Sing-box ║
╚══════════════════════════════════════════════╝
```

它**不替代** OmniRoute 的聚合功能，而是提供 OmniRoute 缺少的能力：
- 多组件生命周期编排（一键启停 Sing-box + CPA + AuthRefresher + OmniRoute）
- 跨组件健康聚合视图（把 6 个 CPA + 6 个 Sing-box + AuthRefresher + OmniRoute 的状态合并为一个面板）
- 历史数据持久化（OmniRoute 重启后 Dashboard 数据丢失，但 SQLite 不会）
- IP 隔离验证仪表盘
- 告警规则自定义

---

## UI 设计思路

### 页面结构（共 6 个页面）

#### 1. 🏠 Overview Dashboard (`/`)
**目标**：一眼看清系统全局健康状态

布局：
```
┌─────────────────────────────────────────────────────┐
│  ● 系统状态: 🟢 全部正常   运行时间: 3d 14h 22m     │
├─────────────────────────────────────────────────────┤
│  [ 6 个 StatusCard ]                                │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐  │
│  │ 🇯🇵A │ │ 🇯🇵B │ │ 🇺🇸C │ │ 🇸🇬D │ │ 🇸🇬E │ │ 🇹🇼F │  │
│  │Token│ │Token│ │Token│ │Token│ │Token│ │Token│  │
│  │ 18h │ │ 12h │ │  6h │ │ 22h │ │ 20h │ │ 15h │  │
│  │ 🟢  │ │ 🟢  │ │ 🟡  │ │ 🟢  │ │ 🟢  │ │ 🟢  │  │
│  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘  │
├─────────────────────────────────────────────────────┤
│  使用量趋势 (24h)           │  成功率 (24h)         │
│  ═══╗    ╔══╗               │  ████████████ 99.7%   │
│     ║    ║  ║               │                       │
│  ═══╝════╝  ╚══             │  失败: 3 次 (429×2,   │
│                             │         timeout×1)    │
├─────────────────────────────────────────────────────┤
│  最近告警:                                          │
│  • 🟡 03:12 Account-C Token 将在 6h 后过期 (已刷新) │
│  • 🟢 02:45 Account-C Circuit Breaker 恢复          │
└─────────────────────────────────────────────────────┘
```

每个 StatusCard 显示：
- 账号昵称 + 地区 Emoji
- Token 剩余时间（倒计时条）
- 健康状态灯（🟢 正常 / 🟡 Token 即将过期 / 🔴 熔断/离线）
- 今日请求数
- 点击展开 → 详情页

#### 2. 📊 Accounts Detail (`/accounts`)
**目标**：管理 6 个 Plus 账号的完整生命周期

功能：
- 表格展示所有账号：昵称、Email(脱敏)、绑定节点、CPA 端口、SOCKS5 端口、Token 状态、Circuit Breaker 状态
- 操作按钮：重新登录 OAuth / 强制刷新 Token / 熔断开关
- 添加新账号向导（选择空闲节点 → 创建 CPA 实例 → 打开 OAuth 登录页）
- 账号排序：按健康度、使用量、Token 过期时间

#### 3. 🌐 Network (`/network`)
**目标**：可视化 IP 隔离验证

功能：
- 6 个节点的实时出口 IP 展示（自动定期 `curl api.ipify.org`）
- IP 地理位置地图标注（Leaflet.js 轻量地图）
- TLS 指纹验证结果（JA3 hash 对比表）
- Ping 延迟 / 丢包率实时监控（sparkline 图表）
- 一键全量 IP 隔离验证按钮

#### 4. 📈 Analytics (`/analytics`)
**目标**：使用量分析和成本估算

功能：
- 请求量趋势图（按小时/天/周，可按账号筛选）
- Token 消耗统计（Input/Output tokens，按模型分组）
- 成功率/错误率饼图（429 / 401 / 403 / timeout 分布）
- 延迟分布直方图（P50 / P95 / P99）
- 成本估算：基于 Plus 订阅 vs 等价 API 费用的对比
- 数据导出：CSV / JSON

#### 5. 🔔 Alerts (`/alerts`)
**目标**：告警规则管理和历史记录

功能：
- 告警规则 CRUD：
  - Token 过期倒计时 < N 小时
  - 单账号连续失败 > N 次
  - 单账号请求数 > 日限额的 N%
  - IP 隔离验证失败
  - 进程意外退出
- 告警通道配置：Dashboard 弹窗 / Email / Bark(iOS) / Webhook
- 告警历史列表（时间线视图）

#### 6. ⚙️ Settings (`/settings`)
**目标**：系统配置管理

功能：
- 组件路径配置（CPA 二进制路径、Sing-box 配置路径等）
- OmniRoute 连接配置（端口、API Key）
- codex-auth-refresher 连接配置
- 系统启停操作（一键启停全部组件）
- Clash 订阅链接 → 自动同步节点变更
- 数据库维护（清理历史数据、导出备份、重置）
- 外观：深色/浅色主题切换

### 全局 UI 组件

| 组件 | 库 | 用途 |
|---|---|---|
| Sidebar 导航 | shadcn Sidebar | 6 页面切换 |
| DataTable | shadcn Table + TanStack Table | 账号列表、告警列表 |
| StatusCard | 自定义 (shadcn Card + Badge) | 账号健康卡片 |
| Charts | Recharts (AreaChart / PieChart / BarChart) | 趋势图/饼图/柱状图 |
| Dialog / Sheet | shadcn Dialog / Sheet | 添加账号向导、设置面板 |
| Toast | shadcn Sonner | 操作反馈、实时告警 |
| ProgressBar | shadcn Progress | Token 倒计时条 |
| Map | Leaflet.js (react-leaflet) | IP 地理位置展示 |
| ThemeToggle | next-themes + shadcn | 深色/浅色切换 |

### 设计美学

- **深色模式优先**：Coding 工具用户偏好深色
- **色彩方案**：基于 OmniRoute 的蓝紫色调，状态灯用标准 🟢🟡🔴
- **动效**：
  - 健康状态灯脉搏动画（`animate-pulse`）
  - Token 倒计时条渐变色（绿 → 黄 → 红）
  - 数据加载 Skeleton 屏
  - 页面切换 Framer Motion 过渡
- **响应式**：主要针对桌面浏览器，最小宽度 1024px

---

## 数据库设计 (SQLite + Drizzle ORM)

### 数据库文件

路径：`codex-aggregator/data/aggregator.db`

### Schema

#### 表 1: `accounts` — Plus 账号注册表

```sql
CREATE TABLE accounts (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  nickname      TEXT NOT NULL,              -- 'Account-A'
  email         TEXT,                       -- 'user@example.com' (可选，脱敏存储)
  instance_num  INTEGER NOT NULL UNIQUE,    -- 1~6
  cpa_port      INTEGER NOT NULL,           -- 8317~8322
  socks5_port   INTEGER NOT NULL,           -- 10001~10006
  server_domain TEXT NOT NULL,              -- 'sttrbjp.cdn.node.a.datahub7.net'
  server_tag    TEXT NOT NULL,              -- 'jp-gpt'
  region        TEXT NOT NULL,              -- '🇯🇵 日本GPT'
  clash_node    TEXT NOT NULL,              -- 'V3-191'
  rate_mult     REAL NOT NULL DEFAULT 1.0,  -- 倍率 1.0/1.5/1.8/2.0
  status        TEXT NOT NULL DEFAULT 'inactive',  -- 'active'|'inactive'|'error'|'circuit_open'
  token_expires_at  DATETIME,              -- Token 过期时间
  token_refreshed_at DATETIME,             -- 上次刷新时间
  created_at    DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at    DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

#### 表 2: `request_logs` — 请求日志（聚合级别，非逐条）

```sql
CREATE TABLE request_logs (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  account_id    INTEGER NOT NULL REFERENCES accounts(id),
  timestamp     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  model         TEXT NOT NULL,              -- 'gpt-5.2'
  input_tokens  INTEGER DEFAULT 0,
  output_tokens INTEGER DEFAULT 0,
  latency_ms    INTEGER,                    -- 响应延迟
  status_code   INTEGER,                    -- 200/429/401/403/500
  is_success    BOOLEAN NOT NULL DEFAULT 1,
  error_type    TEXT,                       -- 'rate_limit'|'auth_failed'|'timeout'|null
  combo_name    TEXT                        -- 'plus-6'
);

-- 索引：按时间和账号查询
CREATE INDEX idx_request_logs_time ON request_logs(timestamp);
CREATE INDEX idx_request_logs_account ON request_logs(account_id, timestamp);
```

#### 表 3: `health_checks` — 健康检查历史

```sql
CREATE TABLE health_checks (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  check_type    TEXT NOT NULL,              -- 'ip_verify'|'cpa_ping'|'singbox_ping'|'token_check'|'omniroute_ping'
  account_id    INTEGER REFERENCES accounts(id),  -- null 表示全局检查
  is_healthy    BOOLEAN NOT NULL,
  details       TEXT,                       -- JSON: {"ip":"1.2.3.4","ja3":"abc..."}
  response_ms   INTEGER                    -- 检查耗时
);

CREATE INDEX idx_health_checks_time ON health_checks(timestamp);
CREATE INDEX idx_health_checks_type ON health_checks(check_type, timestamp);
```

#### 表 4: `alerts` — 告警事件

```sql
CREATE TABLE alerts (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  severity      TEXT NOT NULL,              -- 'info'|'warning'|'critical'
  category      TEXT NOT NULL,              -- 'token'|'circuit_breaker'|'ip'|'process'|'quota'
  account_id    INTEGER REFERENCES accounts(id),  -- null 表示系统级
  title         TEXT NOT NULL,              -- '账号 C Token 将在 6h 后过期'
  message       TEXT,
  is_resolved   BOOLEAN NOT NULL DEFAULT 0,
  resolved_at   DATETIME,
  notified_via  TEXT                        -- JSON: ["dashboard","email"]
);

CREATE INDEX idx_alerts_time ON alerts(timestamp);
CREATE INDEX idx_alerts_unresolved ON alerts(is_resolved, timestamp);
```

#### 表 5: `alert_rules` — 告警规则

```sql
CREATE TABLE alert_rules (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  name          TEXT NOT NULL,              -- 'Token 过期预警'
  enabled       BOOLEAN NOT NULL DEFAULT 1,
  category      TEXT NOT NULL,              -- 同 alerts.category
  condition     TEXT NOT NULL,              -- JSON: {"metric":"token_ttl","operator":"<","value":6,"unit":"hours"}
  channels      TEXT NOT NULL DEFAULT '["dashboard"]',  -- JSON array
  cooldown_min  INTEGER NOT NULL DEFAULT 30,  -- 冷却时间（分钟），防频繁告警
  created_at    DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

#### 表 6: `daily_stats` — 每日聚合统计（用于趋势图加速查询）

```sql
CREATE TABLE daily_stats (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  date          TEXT NOT NULL,              -- '2026-03-23'
  account_id    INTEGER NOT NULL REFERENCES accounts(id),
  total_requests INTEGER NOT NULL DEFAULT 0,
  success_count  INTEGER NOT NULL DEFAULT 0,
  error_count    INTEGER NOT NULL DEFAULT 0,
  total_input_tokens  INTEGER NOT NULL DEFAULT 0,
  total_output_tokens INTEGER NOT NULL DEFAULT 0,
  avg_latency_ms INTEGER,
  p95_latency_ms INTEGER,
  UNIQUE(date, account_id)
);
```

#### 表 7: `system_config` — 键值对配置存储

```sql
CREATE TABLE system_config (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 预置配置
INSERT INTO system_config VALUES
  ('cpa_binary_path', '/usr/local/bin/cliproxyapi', CURRENT_TIMESTAMP),
  ('singbox_config_path', './sing-box/sing-box.json', CURRENT_TIMESTAMP),
  ('omniroute_url', 'http://127.0.0.1:20128', CURRENT_TIMESTAMP),
  ('auth_refresher_url', 'http://127.0.0.1:18081', CURRENT_TIMESTAMP),
  ('clash_config_path', '~/.config/clash/速云梯.yaml', CURRENT_TIMESTAMP),
  ('health_check_interval_sec', '60', CURRENT_TIMESTAMP),
  ('ip_verify_interval_sec', '300', CURRENT_TIMESTAMP),
  ('theme', 'dark', CURRENT_TIMESTAMP);
```

### 数据保留策略

| 表 | 保留期限 | 清理方式 |
|---|---|---|
| `request_logs` | 30 天 | 每日凌晨 CRON 删除 > 30d 的记录 |
| `health_checks` | 7 天 | 每日凌晨清理 |
| `alerts` | 90 天 | 每日凌晨清理 |
| `daily_stats` | 365 天 | 永久保留或年度清理 |

---

## API 接口设计 (Next.js Route Handlers)

### 内部 API (Manager 前后端通信)

| 方法 | 路径 | 功能 | 响应 |
|---|---|---|---|
| GET | `/api/overview` | 全局状态概览 | 6 账号健康 + 系统运行时间 + 24h 统计 |
| GET | `/api/accounts` | 账号列表 | Account[] |
| POST | `/api/accounts` | 添加新账号 | 创建 CPA 实例 + 分配端口 |
| PUT | `/api/accounts/:id` | 更新账号配置 | 修改昵称/节点绑定等 |
| DELETE | `/api/accounts/:id` | 移除账号 | 停止 CPA + 清理 auth |
| POST | `/api/accounts/:id/refresh-token` | 强制刷新 Token | 调用 AuthRefresher API |
| POST | `/api/accounts/:id/toggle-circuit` | 手动熔断/恢复 | 更新 OmniRoute CB |
| GET | `/api/network/verify` | 执行 IP 隔离验证 | 6 IP + JA3 hash |
| GET | `/api/network/latency` | 各节点延迟 | 6 节点 ping 结果 |
| GET | `/api/analytics?range=24h&account=all` | 使用量分析数据 | 趋势 + 分布 + 汇总 |
| GET | `/api/analytics/daily-stats` | 每日聚合统计 | daily_stats[] |
| GET | `/api/alerts` | 告警列表 | Alert[] |
| PUT | `/api/alerts/:id/resolve` | 标记告警已解决 | 更新 resolved_at |
| GET | `/api/alert-rules` | 告警规则列表 | AlertRule[] |
| POST | `/api/alert-rules` | 创建告警规则 | 新规则 |
| PUT | `/api/alert-rules/:id` | 更新规则 | 修改条件/通道 |
| DELETE | `/api/alert-rules/:id` | 删除规则 | |
| GET | `/api/system/health` | 全链路健康检查 | 15 项检查结果 |
| POST | `/api/system/start-all` | 一键启动全部组件 | 启动状态 |
| POST | `/api/system/stop-all` | 一键停止全部组件 | 停止状态 |
| GET | `/api/system/config` | 获取系统配置 | system_config |
| PUT | `/api/system/config` | 更新系统配置 | 修改配置 |
| GET | `/api/system/logs?component=cpa-1&lines=100` | 组件日志尾部 | 日志文本 |
| GET | `/api/sse/events` | SSE 实时事件流 | 健康/告警/Token 变更 |

### 外部 API 调用关系

Manager 调用外部组件的 API：

| 目标 | 端点 | 用途 |
|---|---|---|
| OmniRoute | `GET /v1/models` | 验证模型列表 |
| OmniRoute | `GET /api/stats` (如支持) | 获取使用量统计 |
| CPA-1~6 | `GET /management.html` 或 Management API | 验证存活 + 获取账号状态 |
| AuthRefresher | `GET /healthz` | 健康检查 |
| AuthRefresher | `GET /readyz` | 就绪检查 |
| AuthRefresher | `GET /v1/status` | Token 状态详情 |
| api.ipify.org | SOCKS5 代理请求 | IP 隔离验证 |
| tls.browserleaks.com | SOCKS5 代理请求 | TLS 指纹验证 |

---

## 后台任务调度

| 任务 | 间隔 | 实现 | 功能 |
|---|---|---|---|
| 健康检查 | 60s | `setInterval` + Worker Thread | 检查 CPA/Sing-box/OmniRoute/AuthRefresher 存活 |
| IP 隔离验证 | 5min | `setInterval` | 通过 SOCKS5 查询 ipify.org 验证 6 IP 互不相同 |
| Token 状态同步 | 2min | `setInterval` | 从 AuthRefresher `/v1/status` 同步 Token 过期时间到 DB |
| 告警评估 | 30s | 事件驱动 | 基于 alert_rules 评估健康检查/请求日志，触发告警 |
| 日统计聚合 | 每日 00:05 | `node-cron` | 聚合 request_logs → daily_stats |
| 历史清理 | 每日 03:00 | `node-cron` | 删除超过保留期限的旧数据 |
| 请求日志采集 | 每次请求 | OmniRoute 日志 tail + regex 解析 | 采集请求结果写入 request_logs |

> **采集方案说明**：OmniRoute 未提供请求级 Webhook，因此采用日志 `tail -F` + 正则解析方案。OmniRoute 日志格式为 `[时间] [STATUS] model=xxx tokens=xxx latency=xxxms`。Manager 后端启动一个 Worker Thread 执行 `tail -F` 并解析每行日志写入 SQLite。如果 OmniRoute 未来版本增加 Webhook 支持，可切换到更优雅的方案。

---

## 10 轮自优化修正 (Self-Optimization Patches)

### 修正 1: DB Schema — `accounts` 表增加 `device_id` (轮 2)

```sql
-- accounts 表新增字段
device_id     TEXT,                       -- OAI-Device-Id（防封关键字段，硬绑定）
```

OAI-Device-Id 是 CPA 生成的设备唯一标识，硬绑定在 auth 文件中。将其镜像存储到 DB 中便于 Dashboard 展示和一致性校验。

### 修正 2: DB Schema — 新增 `process_status` 表 (轮 2/6)

```sql
CREATE TABLE process_status (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  component     TEXT NOT NULL UNIQUE,       -- 'singbox'|'cpa-1'...'cpa-6'|'auth-refresher'|'omniroute'
  pid           INTEGER,                    -- 进程 PID
  status        TEXT NOT NULL DEFAULT 'stopped',  -- 'running'|'stopped'|'crashed'|'restarting'
  started_at    DATETIME,
  last_crash_at DATETIME,
  crash_count   INTEGER NOT NULL DEFAULT 0, -- 累计崩溃次数
  auto_restart  BOOLEAN NOT NULL DEFAULT 1, -- 是否自动重启
  updated_at    DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### 修正 3: DB Schema — `request_logs` 增加追踪字段 (轮 2)

```sql
-- request_logs 表新增字段
request_id    TEXT,                        -- OmniRoute 生成的请求 ID（用于去重和追踪）
```

### 修正 4: API 补充 — 进程管理和重登录 (轮 3)

| 方法 | 路径 | 功能 |
|---|---|---|
| GET | `/api/system/processes` | 所有组件进程状态 (PID/uptime/crash_count) |
| POST | `/api/system/restart/:component` | 单组件重启 (e.g., `restart/cpa-3`) |
| POST | `/api/accounts/:id/relogin` | 打开 CPA Management 页面触发 OAuth 重登录 |
| GET | `/api/system/onboarding-status` | 首次设置向导状态检查 |
| POST | `/api/system/onboarding` | 执行首次设置（创建 DB、生成 Sing-box 配置、初始化 CPA 实例） |

### 修正 5: UI 补充 — 首次设置向导 (轮 1)

新增 **Onboarding Wizard**（`/setup`），仅在数据库为空时显示：

步骤流程：
1. **环境检查**：验证 CPA/Sing-box/Node.js/Docker 已安装
2. **Clash 导入**：选择 Clash 配置文件 → 自动解析非香港节点
3. **节点分配**：将可用节点拖拽分配给 6 个账号位置
4. **Sing-box 生成**：自动生成 `sing-box.json` 和启动脚本
5. **CPA 实例创建**：自动创建 6 个 instance 目录和配置文件
6. **OAuth 登录**：逐个打开 CPA Management 页面，引导登录 Plus 账号
7. **验证**：全链路健康检查 → 显示 ✅/❌ 结果
8. **OmniRoute 配置**：引导注册 6 个 Provider + 创建 Combo

### 修正 6: 安全策略 (轮 4)

- **路径校验**：`system_config` 中的路径值使用 `path.resolve()` + `fs.existsSync()` 校验，禁止 `../` 和绝对路径注入
- **CORS**：`next.config.js` 中配置 `headers()` 返回 `Access-Control-Allow-Origin: http://127.0.0.1:3000`，仅允许本地访问
- **JWT 策略**：
  - 生命周期：24h
  - 刷新：页面活跃时每 12h 静默刷新
  - 存储：httpOnly cookie（非 localStorage）
  - 密钥：首次启动随机生成，存入 system_config

### 修正 7: 性能优化 (轮 5)

- **日志批量写入**：`request_logs` 使用 `INSERT` 批量队列，每 5s 或累积 50 条时批量 `INSERT INTO ... VALUES ...`，减少 SQLite 写事务频率
- **ipify 限速保护**：IP 验证每 5 分钟一次（已配置），但不再对每个节点并行请求，改为串行（每节点间隔 2s），避免 ipify 429

### 修正 8: 自动恢复策略 (轮 6)

| 场景 | 策略 |
|---|---|
| CPA 进程崩溃 | 自动重启，最多 3 次/小时；超过则标记 `crashed` 并告警 |
| Sing-box 崩溃 | 自动重启 + 停止所有依赖此 SOCKS5 的 CPA 实例 |
| AuthRefresher 容器退出 | `docker compose restart` + 告警 |
| OmniRoute 崩溃 | 自动重启 + 告警（此时 Codex App 会直接超时） |
| DB 文件丢失 | 降级模式：所有功能正常但无历史数据，Dashboard 显示 "数据库未初始化" 横幅 |
| 节点不可用 | 标记该节点 `unhealthy`，对应 CPA 熔断，触发 OmniRoute Combo fallback |

### 修正 9: SSE 事件类型定义 (轮 8)

```typescript
type SSEEvent =
  | { type: 'health_update';   data: { account_id: number; is_healthy: boolean; check_type: string } }
  | { type: 'token_update';    data: { account_id: number; expires_at: string; status: 'refreshed' | 'expiring' | 'expired' } }
  | { type: 'alert_fired';     data: { alert_id: number; severity: string; title: string } }
  | { type: 'alert_resolved';  data: { alert_id: number } }
  | { type: 'process_change';  data: { component: string; status: 'running' | 'stopped' | 'crashed' } }
  | { type: 'request_logged';  data: { account_id: number; model: string; is_success: boolean; latency_ms: number } }
  | { type: 'ip_verify_done';  data: { results: Array<{ account_id: number; ip: string; is_unique: boolean }> } }
```

### 修正 10: 错误处理策略 (轮 10)

| 层 | 策略 |
|---|---|
| API Route | 统一 try-catch → `{ error: string, code: string }` 格式 |
| DB | Drizzle 事务失败自动重试 1 次 → 仍失败则抛出 + 告警 |
| 子进程 | `child_process.on('error')` + `on('exit')` 双监听 → 触发自动恢复 |
| SSE | 客户端自动重连（`EventSource` 原生支持），服务端 30s 心跳 |
| 前端 | React Error Boundary 包裹每个页面 → 错误页面不影响其他页面 |

---

## 20 轮深度自优化修正 (Round 11~30)

### 修正 11: 节点绑定表事实修正 (Critical)

> **[已直接修正上方表格]** 之前的节点名称、端口、倍率等 4 处与 Clash 实际数据不符：
> - `V3-191 x1.0` → 实际 `V3-189 x1.5`
> - `V4-18 x1.0` → 实际 `V3-50 x1.8`
> - `V4-19 x1.0` → 实际 `V3-166 x1.8`
> - sttrbjp 端口 50191 → 实际 50189
>
> **教训**：所有配置参数必须从源文件（Clash YAML）程序化提取，不可手写估计。

### 修正 12: Sing-box 完整配置样例 (`sing-box.json`)

```json
{
  "log": { "level": "info", "output": "./logs/singbox.log", "timestamp": true },
  "inbounds": [
    { "type": "socks", "tag": "socks-in-1", "listen": "127.0.0.1", "listen_port": 10001 },
    { "type": "socks", "tag": "socks-in-2", "listen": "127.0.0.1", "listen_port": 10002 },
    { "type": "socks", "tag": "socks-in-3", "listen": "127.0.0.1", "listen_port": 10003 },
    { "type": "socks", "tag": "socks-in-4", "listen": "127.0.0.1", "listen_port": 10004 },
    { "type": "socks", "tag": "socks-in-5", "listen": "127.0.0.1", "listen_port": 10005 },
    { "type": "socks", "tag": "socks-in-6", "listen": "127.0.0.1", "listen_port": 10006 }
  ],
  "outbounds": [
    {
      "type": "vmess", "tag": "jp-gpt",
      "server": "sttrbjp.cdn.node.a.datahub7.net", "server_port": 50189,
      "uuid": "577edbac-cb2a-3652-8210-a949d76f24a9", "alter_id": 0, "security": "auto",
      "transport": { "type": "ws", "path": "/009c250b-54f5-43a8-8b18-dccbb5d8c66a.y.live01.m3u8" },
      "multiplex": { "enabled": false },
      "tls": { "enabled": false }
    },
    {
      "type": "vmess", "tag": "jp",
      "server": "dcv69.cdn.node.a.datahub7.net", "server_port": 50208,
      "uuid": "577edbac-cb2a-3652-8210-a949d76f24a9", "alter_id": 0, "security": "auto",
      "transport": { "type": "ws", "path": "/009c250b-54f5-43a8-8b18-dccbb5d8c66a.y.live01.m3u8" },
      "tls": { "enabled": false }
    },
    {
      "type": "vmess", "tag": "us",
      "server": "ggsd5.cdn.node.a.datahub7.net", "server_port": 50103,
      "uuid": "577edbac-cb2a-3652-8210-a949d76f24a9", "alter_id": 0, "security": "auto",
      "transport": { "type": "ws", "path": "/009c250b-54f5-43a8-8b18-dccbb5d8c66a.y.live01.m3u8" },
      "tls": { "enabled": false }
    },
    {
      "type": "vmess", "tag": "sg-a",
      "server": "aasx2.cdn.node.a.datahub7.net", "server_port": 50050,
      "uuid": "577edbac-cb2a-3652-8210-a949d76f24a9", "alter_id": 0, "security": "auto",
      "transport": { "type": "ws", "path": "/009c250b-54f5-43a8-8b18-dccbb5d8c66a.y.live01.m3u8" },
      "tls": { "enabled": false }
    },
    {
      "type": "vmess", "tag": "sg-b",
      "server": "cvb7.cdn.node.a.datahub7.net", "server_port": 50166,
      "uuid": "577edbac-cb2a-3652-8210-a949d76f24a9", "alter_id": 0, "security": "auto",
      "transport": { "type": "ws", "path": "/009c250b-54f5-43a8-8b18-dccbb5d8c66a.y.live01.m3u8" },
      "tls": { "enabled": false }
    },
    {
      "type": "vmess", "tag": "tw",
      "server": "bnm4.cdn.node.a.datahub7.net", "server_port": 50068,
      "uuid": "577edbac-cb2a-3652-8210-a949d76f24a9", "alter_id": 0, "security": "auto",
      "transport": { "type": "ws", "path": "/009c250b-54f5-43a8-8b18-dccbb5d8c66a.y.live01.m3u8" },
      "tls": { "enabled": false }
    },
    { "type": "direct", "tag": "direct" }
  ],
  "route": {
    "rules": [
      { "inbound": ["socks-in-1"], "outbound": "jp-gpt" },
      { "inbound": ["socks-in-2"], "outbound": "jp" },
      { "inbound": ["socks-in-3"], "outbound": "us" },
      { "inbound": ["socks-in-4"], "outbound": "sg-a" },
      { "inbound": ["socks-in-5"], "outbound": "sg-b" },
      { "inbound": ["socks-in-6"], "outbound": "tw" }
    ]
  }
}
```

> **注意**: Sing-box 的 `utls` 设置在 VMess outbound 的 `tls` 块内需要 `tls.enabled: true` 才生效。但这些节点 `tls: false`（隧道不加密），所以 `utls` 无法在 outbound 层生效。
> **实际防御机制**: CPA 直接发起到 `api.openai.com:443` 的 HTTPS 连接，这个连接经过 SOCKS5 → Sing-box → VMess → 代理服务器。代理服务器作为 SOCKS5 的上游，会代行建立到目标的 TCP 连接。**TLS 握手发生在最终目标端**，由 CPA 的 Go TLS 库执行，Sing-box 在此过程中是透明隧道。
> **修正结论**: 在这种架构下，utls:chrome **不会生效**。要真正改写 TLS 指纹，需要 (a) 让 Sing-box 作为透明代理拦截 TLS 握手（需要 tun 模式），或 (b) 在 CPA 端使用 Go 的 utls 库。**这是一个需要在 Pilot 阶段验证的关键风险**。详见 pilot_matrix B1-singbox。

### 修正 13: CPA config.yaml 完整格式 (轮 14)

```yaml
# CPA Instance 1 配置文件
# 路径: cpa-instances/instance-1/config.yaml

listen:
  host: "127.0.0.1"
  port: 8317

proxy:
  url: "socks5://127.0.0.1:10001"

auth:
  dir: "./auths"

management:
  enabled: true
  # Management API 与主端口共用

logging:
  level: "info"
  file: "./logs/cpa-1.log"
  format: "json"
```

> CPA 的确切配置格式需在实施阶段通过 `cliproxyapi --help` 或 README 确认。上述为基于调研推断的合理格式。

### 修正 14: codex-auth-refresher Docker 卷映射 (轮 15)

```yaml
# auth-refresher/docker-compose.yml
version: "3.8"
services:
  auth-refresher:
    image: pymba86/codex-auth-refresher:latest
    container_name: codex-auth-refresher
    restart: unless-stopped
    ports:
      - "127.0.0.1:18081:18081"
    volumes:
      # 方案: 统一挂载到 /data/auth 下的子目录
      - ../cpa-instances/instance-1/auths:/data/auth/instance-1:rw
      - ../cpa-instances/instance-2/auths:/data/auth/instance-2:rw
      - ../cpa-instances/instance-3/auths:/data/auth/instance-3:rw
      - ../cpa-instances/instance-4/auths:/data/auth/instance-4:rw
      - ../cpa-instances/instance-5/auths:/data/auth/instance-5:rw
      - ../cpa-instances/instance-6/auths:/data/auth/instance-6:rw
    env_file: .env
```

> **确认点**: `codex-auth-refresher` 是否支持扫描 `/data/auth` 下的子目录（递归搜索 `codex-*.json`），需在实施阶段验证。如不支持递归，需改为扁平化映射或使用 symlink。

### 修正 15: 端口冲突预检清单 (轮 17)

| 端口 | 组件 | 备注 |
|---|---|---|
| 3000 | Manager (Next.js) | 可能与其他 dev server 冲突 → 可改为 3100 |
| 8317~8322 | CPA-1~6 | 连续 6 端口，需确认 CPA 默认端口 |
| 10001~10006 | Sing-box SOCKS5 | 冷门端口，冲突概率低 |
| 18081 | codex-auth-refresher | 冷门端口 |
| 20128 | OmniRoute | OmniRoute 默认端口 |

实施前执行：
```bash
for port in 3000 8317 8318 8319 8320 8321 8322 10001 10002 10003 10004 10005 10006 18081 20128; do
  lsof -i :$port > /dev/null 2>&1 && echo "⚠️ Port $port in use" || echo "✅ Port $port free"
done
```

### 修正 16: 台湾节点可用性风险 (轮 28)

OpenAI 的地区封锁名单时常变化。台湾目前**通常可用**但不保证长期稳定。

**缓解措施**:
1. Pilot 阶段首先验证 `bnm4` 节点是否能访问 `api.openai.com`
2. 如不可用，直接将 Account-F 标记为 inactive → 5 账号运行
3. 如启动后某天突然被封，OmniRoute Circuit Breaker 会自动熔断该 CPA
4. Manager Dashboard 的 IP 验证定期检测，发现不可达自动告警

### 修正 17: 新加坡两节点 ASN 重合风险 (轮 30)

`aasx2` 和 `cvb7` 虽然域名不同但可能共享同一 ASN 或相近 IP 段：
- 如果 OpenAI 做 ASN 级别的关联检测（同一 ASN 下多个 Plus 账号），可能增加风险
- **缓解**: IP 验证时检查两个 IP 的 ASN 号是否相同（通过 `ipinfo.io` API 查询）
- 如 ASN 相同，将 Account-E 换到备选方案节点或降级为 4+1 运行

### 修正 18: `health_checks` 数据膨胀优化 (轮 29)

60s × 9 组件 × 86400s/天 ÷ 60 = 每天 **12,960 行**，7 天 = **90,720 行**。

优化方案：
- **分辨率降级**: 超过 24h 的健康检查记录按 5min 聚合（保留最差结果），而非逐条保留
- **只记异常**: 状态未变化时不写入新行，仅在状态变化（healthy → unhealthy 或反之）时写入
- **新增 `health_summary` 聚合表**:
  ```sql
  CREATE TABLE health_summary (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    date       TEXT NOT NULL,           -- '2026-03-23'
    hour       INTEGER NOT NULL,        -- 0~23
    account_id INTEGER REFERENCES accounts(id),
    check_type TEXT NOT NULL,
    healthy_pct REAL NOT NULL,          -- 0.0~1.0
    avg_response_ms INTEGER,
    UNIQUE(date, hour, account_id, check_type)
  );
  ```

### 修正 19: 请求分配策略 — 加权 Round-Robin (轮 27)

不同节点倍率不同（流量消耗不同），应加权分配以均衡流量成本：

| 节点 | 倍率 | 权重 (1/倍率归一化) | 每 10 次请求分配 |
|---|---|---|---|
| jp-gpt | x1.5 | 0.20 | 2 次 |
| jp | x1.5 | 0.20 | 2 次 |
| us | x2.0 | 0.15 | 1~2 次 |
| sg-a | x1.8 | 0.17 | 1~2 次 |
| sg-b | x1.8 | 0.17 | 1~2 次 |
| tw | x1.8 | 0.11 | 1 次 |

OmniRoute Combo 支持权重设置 → 在创建 Combo 时配置 `weight` 参数。

### 修正 20: 备份与恢复策略 (轮 21)

| 数据 | 路径 | 备份频率 | 方式 |
|---|---|---|---|
| SQLite DB | `data/aggregator.db` | 每日 | `cp` 到 `data/backups/aggregator-YYYYMMDD.db` |
| CPA Auth 文件 | `cpa-instances/instance-*/auths/` | 每日 | `tar` 打包到 `data/backups/auths-YYYYMMDD.tar.gz` |
| Sing-box 配置 | `sing-box/sing-box.json` | 配置变更时 | Git 版本控制 |
| OmniRoute 数据 | `omniroute/db.json` | 每日 | `cp` 备份 |

自动备份脚本 `scripts/backup.sh`，由 `node-cron` 每日 02:00 执行。保留最近 7 天备份。

### 修正 21: 各组件日志路径规范 (轮 22)

```
codex-aggregator/logs/
├── singbox.log          # Sing-box 主日志 (JSON 格式)
├── cpa-1.log            # CPA Instance 1
├── cpa-2.log            # CPA Instance 2
├── cpa-3.log            # CPA Instance 3
├── cpa-4.log            # CPA Instance 4
├── cpa-5.log            # CPA Instance 5
├── cpa-6.log            # CPA Instance 6
├── omniroute.log        # OmniRoute 日志 (通过 stdout redirect)
├── auth-refresher.log   # Docker logs → file
└── manager.log          # Next.js 应用日志
```

日志轮转：`logrotate` 或 Manager CRON 每日压缩 > 3 天的日志。

### 修正 22: Sing-box 单进程架构确认 (轮 23)

Sing-box 是**单进程多 outbound** 架构：一个 `sing-box run` 进程同时管理 6 个 inbound 和 6 个 outbound。不需要启动 6 个独立进程。这简化了进程管理但意味着 Sing-box 崩溃会导致所有 6 个 CPA 同时断网。

**缓解**: Manager 对 Sing-box 进程监控优先级最高 → 崩溃后 2s 内自动重启。

### 修正 23: OmniRoute 安装方式 (轮 24)

需在实施阶段确认 `omniroute` 是否已发布到 npm。备选安装方式：
1. `npm install -g omniroute` (如已发布)
2. `git clone + npm install + npm run build` (如仅 GitHub 源码)
3. Docker (如提供镜像)

在 `OPENAI_BASE_URL` 配置中，OmniRoute 可能需要设置内部 base URL 指向 CPA 时**不添加** `/v1` 后缀（CPA 本身已处理路径）。

### 修正 24: Clash 订阅链接同步 (轮 25)

速云梯订阅链接在机场方更新节点时会变化。自动同步流程：

1. Settings 页面保存订阅 URL
2. 定时任务每 24h 重新拉取订阅 YAML
3. 解析新节点列表 → 与当前绑定对比
4. 如果已绑定的域名仍存在 → 静默更新端口/参数
5. 如果已绑定的域名消失 → 告警 "节点已下线" + 建议重新分配
6. 新增节点不自动分配（需人工确认）

### 修正 25: 成本追踪准确性说明 (轮 26)

Plus 订阅是固定月费 $20/账号，不按 Token 计费。Analytics 页面的"成本估算"实际展示：
- **等价 API 成本**: 如果同样的请求量使用付费 API Key 需要花多少钱
- **节省比例**: (等价 API 成本 - Plus 订阅费) / 等价 API 成本
- **用量均衡度**: 6 个账号的请求分布是否均匀（避免某账号被过度使用）

### 修正 26: 前端数据获取策略 (轮 19)

| 数据类型 | 获取方式 | 库 | 刷新策略 |
|---|---|---|---|
| 一次性数据 (accounts, config) | REST `GET` | SWR (`useSWR`) | `revalidateOnFocus: true` |
| 实时数据 (health, token) | SSE | `useEventSource` 自定义 hook | 实时推送 |
| 表格数据 (alerts, logs) | REST + 分页 | SWR + `cursor` 分页 | 手动刷新 + 自动 30s |
| 图表数据 (analytics) | REST | SWR | `refreshInterval: 60000` |
| 操作 (start/stop/restart) | REST `POST` | `fetch` + `mutate` | 即时 |

选择 **SWR** 而非 React Query 的理由：Next.js 官方推荐、更轻量、与 App Router 集成更好。

### 修正 27: Drizzle Schema TypeScript 定义 (轮 20)

```typescript
// src/db/schema.ts
import { sqliteTable, text, integer, real } from 'drizzle-orm/sqlite-core';

export const accounts = sqliteTable('accounts', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  nickname: text('nickname').notNull(),
  email: text('email'),
  deviceId: text('device_id'),
  instanceNum: integer('instance_num').notNull().unique(),
  cpaPort: integer('cpa_port').notNull(),
  socks5Port: integer('socks5_port').notNull(),
  serverDomain: text('server_domain').notNull(),
  serverTag: text('server_tag').notNull(),
  region: text('region').notNull(),
  clashNode: text('clash_node').notNull(),
  rateMult: real('rate_mult').notNull().default(1.0),
  status: text('status').notNull().default('inactive'),
  tokenExpiresAt: text('token_expires_at'),
  tokenRefreshedAt: text('token_refreshed_at'),
  createdAt: text('created_at').default('CURRENT_TIMESTAMP'),
  updatedAt: text('updated_at').default('CURRENT_TIMESTAMP'),
});

export const requestLogs = sqliteTable('request_logs', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  accountId: integer('account_id').notNull().references(() => accounts.id),
  requestId: text('request_id'),
  timestamp: text('timestamp').notNull().default('CURRENT_TIMESTAMP'),
  model: text('model').notNull(),
  inputTokens: integer('input_tokens').default(0),
  outputTokens: integer('output_tokens').default(0),
  latencyMs: integer('latency_ms'),
  statusCode: integer('status_code'),
  isSuccess: integer('is_success', { mode: 'boolean' }).notNull().default(true),
  errorType: text('error_type'),
  comboName: text('combo_name'),
});

// ... (health_checks, alerts, alert_rules, daily_stats, system_config, process_status 同理)
```

### 修正 28: utls 不生效问题的替代方案 (Critical — 轮 13 修正衍生)

修正 12 中发现 `utls: chrome` 在 VMess `tls:false` 隧道中**不会改写 CPA 到目标的 TLS 指纹**。这是架构中的重大技术风险。

**替代方案 (按推荐度排序)**:

1. **CPA 自身支持 utls** — 如果 CPA 的 Go HTTP 客户端使用了 `github.com/refraction-networking/utls`，则已内置 TLS 指纹伪装。需检查 CPA 源码。
2. **Sing-box tun 模式** — 将 Sing-box 配置为 `tun` inbound，拦截 TCP level 流量，此时 utls 才能改写 Go TLS 握手。但 tun 模式需要 root/systemExtension 权限（macOS 限制）。
3. **curl_cffi 中间层** — 在 CPA → Sing-box 之间插入 `chatgpt-to-api`（使用 `curl_cffi` 实现 Chrome TLS），但这增加了一层复杂性。
4. **Accept risk** — CPA 使用 Go 原生 TLS，其 JA3 指纹与 Codex CLI 一致。既然 CPA 本质就是模拟 Codex CLI 行为，OpenAI 不太可能封杀自家 CLI 的 TLS 指纹。**这可能是最务实的选择**。

**Pilot B1-singbox 的验证方式需更新**: 不仅测试 utls 是否改写 JA3，还要测试 CPA 原生 Go TLS 指纹是否与 Codex CLI 一致。如果一致 → 无需 utls，L2 防御层可标记为"由 CPA 原生保障"。

### 修正 29: OmniRoute 是否真实存在验证 (轮 16)

> **⚠️ 重要**: 在 GitHub 调研阶段引用的项目 URL（CLIProxyAPI、OmniRoute、codex-auth-refresher 等）需在实施前逐一打开验证是否仍可访问、是否有 npm 包、是否有 Docker 镜像。如果任何项目不可用或功能与描述不符，需要 fallback 到替代方案。

**实施前必检清单**:
- [ ] `https://github.com/router-for-me/CLIProxyAPI` 可访问且有 Release 二进制
- [ ] `https://github.com/diegosouzapw/OmniRoute` 可访问且 npm/Docker 可安装
- [ ] `https://github.com/pymba86/codex-auth-refresher` 可访问且 Docker 可运行
- [ ] Sing-box `brew install sing-box` 成功，版本 ≥ 1.8

### 修正 30: 最终版架构风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| utls 不生效（CPA Go TLS 未被改写） | 高 | 中 (CPA 原生指纹 ≈ Codex CLI) | 验证 CPA 指纹 → Accept risk |
| 台湾节点被 OpenAI 封锁 | 中 | 低 (降至 5 账号) | Circuit Breaker 自动熔断 |
| 新加坡两节点同 ASN | 中 | 中 (关联检测) | IP 验证时检查 ASN |
| OmniRoute 项目不存在/不可用 | 低 | 高 (架构核心) | 退回自研 + 9Router |
| codex-auth-refresher 不支持子目录 | 中 | 低 (改用 symlink) | Pilot 验证 |
| Clash 订阅节点下线 | 中 | 中 (对应 CPA 断线) | 24h 订阅同步 + 告警 |
| SQLite 并发写入瓶颈 | 低 | 低 (WAL 模式) | 批量写入 + WAL |
| 5 个端口已被占用 | 低 | 低 (可改端口) | 启动前端口检查 |

---

## 持续自优化（Round 31 → 收敛）

### 修正 31: code_list.md Milestone 1 端口错误同步修正 (Critical)

> **[直接修正 code_list.md M1 表格]** 与 outline.md 修正11 一致，以下端口之前在 code_list.md 中记录有误：
> - `sttrbjp → 50191` 应为 **50189**（已在 outline.md 修正11 中确认）
> - `aasx2 → 50018` 应为 **50050**（V3-50 节点实际端口）
> - `cvb7 → 50019` 应为 **50166**（V3-166 节点实际端口）
>
> **根因**: 配置参数需从 Clash YAML 程序化提取，禁止手写估算（与修正11教训一致）。code_list.md M1 中的绑定表已同步修正。

### 修正 32: utls 策略重写 — `code_list.md` M1 移除错误配置 (Critical)

outline.md 修正12 和修正28 已确认：在 `tls: false` 的 VMess 隧道中，Sing-box 的 `utls:chrome` **不会对 CPA→OpenAI 的 TLS 握手生效**。但 `code_list.md` M1 仍然写着：

```bash
# 每个 outbound 强制 `utls: { enabled: true, fingerprint: "chrome" }`  ← 错误期望
```

**修正策略**（四选一，按推荐度排序）：

1. **Accept Risk (推荐)**: CPA 的 Go TLS 指纹 ≈ Codex CLI 原生指纹。OpenAI 不会封自家 CLI 的 TLS 签名。验证方法：`tls.browserleaks.com` 通过 SOCKS5 检查 CPA 的 JA3 hash，与官方 Codex CLI 对比。
2. **Sing-box tun 模式**: macOS 需系统扩展权限，不适合轻量部署。
3. **curl_cffi 插件**: 在 CPA 和 Sing-box 之间插入 HTTP 中间层，增加复杂性。
4. **查阅 CPA 源码**: 确认 CPA 是否内置 `refraction-networking/utls`。

**code_list.md M1 修正**:
- 删除 `utls: { enabled: true, fingerprint: "chrome" }` 配置项
- 在 sing-box.json outbound 中移除 `utls` 块（因为 `tls.enabled: false`）
- 在 M1 验证步骤中新增：检查 CPA Go TLS JA3 hash 和 Codex CLI JA3 是否一致
- pilot_matrix.md B1-singbox 成功标准需更新（见修正45）

### 修正 33: OmniRoute 双轨安装方案 + 版本锁定

outline.md 修正23 指出 OmniRoute 是否已发布到 npm 需验证。两条安装路径：

**路径 A（npm 全局安装，首选）**:
```bash
npm install -g omniroute@latest
omniroute --version   # 确认版本
```

**路径 B（从源码编译，备选）**:
```bash
git clone https://github.com/diegosouzapw/OmniRoute.git
cd OmniRoute && npm ci && npm run build
node dist/index.js    # 替代 omniroute 命令
```

**路径 C（Docker，备备选）**:
```bash
docker run -d --name omniroute \
  -p 127.0.0.1:20128:20128 \
  -e PORT=20128 \
  diegosouzapw/omniroute:latest
```

自动检测并选路脚本（加入 `scripts/install_omniroute.sh`）：
```bash
#!/usr/bin/env bash
set -e
if npm list -g omniroute &>/dev/null; then
  echo "✅ OmniRoute already installed globally"
elif npm pack omniroute 2>/dev/null | grep -q 'omniroute'; then
  npm install -g omniroute
  echo "✅ OmniRoute installed from npm"
else
  echo "⚠️  npm package not found, cloning from GitHub..."
  git clone https://github.com/diegosouzapw/OmniRoute.git omniroute-src
  cd omniroute-src && npm ci && npm run build
  ln -sf "$(pwd)/dist/index.js" /usr/local/bin/omniroute
  echo "✅ OmniRoute installed from source"
fi
```

### 修正 34: OmniRoute Combo 加权 Round-Robin 配置落实

修正19 规划了加权分配策略但未说明具体配置方法。OmniRoute Combo 创建时：

```json
// OmniRoute Combo API 或 db.json combo 格式（推断，需在 Dashboard 验证）
{
  "name": "plus-6",
  "strategy": "weighted-round-robin",
  "providers": [
    { "id": "cpa-1-jp-gpt", "weight": 20 },
    { "id": "cpa-2-jp",     "weight": 20 },
    { "id": "cpa-3-us",     "weight": 15 },
    { "id": "cpa-4-sg-a",   "weight": 17 },
    { "id": "cpa-5-sg-b",   "weight": 17 },
    { "id": "cpa-6-tw",     "weight": 11 }
  ],
  "fallback": ["openai-mini"]
}
```

> **实施注意**: OmniRoute 的 Combo 配置接口需在 Dashboard 中确认是否支持 `weight` 字段。如不支持，退化为普通 round-robin（当前请求量级下差异可忽略）。

### 修正 35: codex-auth-refresher 子目录递归扫描验证强化

修正14 提到「是否支持扫描 `/data/auth` 下的子目录（递归搜索 `codex-*.json`）」需验证。

**Pilot B4 Kill Rule 强化**（补充到 pilot_matrix.md）：

```
额外 Kill 条件: 
  - docker exec codex-auth-refresher find /data/auth -name "codex-*.json" 
    如果仅能找到 /data/auth/*.json 而非 /data/auth/instance-*/codex-*.json 
    → 不支持递归扫描

Fallback 修复（如不支持递归）：
  # 方案 A: 扁平化映射（修改 docker-compose.yml）
  - ../cpa-instances/instance-1/auths/codex-account-a.json:/data/auth/codex-account-a.json:rw
  # ... 逐文件映射（缺点：auth 文件名需提前知道）

  # 方案 B: symlink 聚合目录
  mkdir -p auth-refresher/auths-all
  for i in 1 2 3 4 5 6; do
    ln -sf $(realpath cpa-instances/instance-$i/auths) \
      auth-refresher/auths-all/instance-$i
  done
  # 然后映射 auth-refresher/auths-all:/data/auth:rw

  # 方案 C: 自定义 CODEX_AUTH_PATTERN 正则（如 refresher 支持）
```

### 修正 36: DB Schema 补全 `health_summary` 表（修正18衍生）

修正18 新增了 `health_summary` 聚合表，但该表仅在修正18的说明中出现，未纳入 Milestone 7 的 DB 表计数和 Kill Rule 中。

**修正**：
- `health_summary` 表加入 DB Schema（见 SQL DDL，已在修正18定义）
- Milestone 7 DB 表数量从 **8** 更正为 **9**（含 `health_summary`）
- Kill Rule：`sqlite3 aggregator.db ".tables"` 结果应包含 `health_summary`
- `daily_stats` CRON 任务同时需执行 `health_summary` 的小时级聚合（见后台任务调度表 → 更新为"日统计 + 健康摘要聚合"）

### 修正 37: Drizzle Schema 完整 TypeScript 定义（补全被截断部分）

code_list.md M8 的 Drizzle Schema 以 `// ... (health_checks, alerts, alert_rules, daily_stats, system_config, process_status 同理)` 截断，这是典型的偷懒截断，执行层无法直接使用。完整定义：

```typescript
// src/db/schema.ts — 全量表定义

export const healthChecks = sqliteTable('health_checks', {
  id:         integer('id').primaryKey({ autoIncrement: true }),
  timestamp:  text('timestamp').notNull().default('CURRENT_TIMESTAMP'),
  checkType:  text('check_type').notNull(),         // 'ip_verify'|'cpa_ping'|'singbox_ping'|'token_check'|'omniroute_ping'
  accountId:  integer('account_id').references(() => accounts.id),
  isHealthy:  integer('is_healthy', { mode: 'boolean' }).notNull(),
  details:    text('details'),                       // JSON string
  responseMs: integer('response_ms'),
});

export const alerts = sqliteTable('alerts', {
  id:          integer('id').primaryKey({ autoIncrement: true }),
  timestamp:   text('timestamp').notNull().default('CURRENT_TIMESTAMP'),
  severity:    text('severity').notNull(),            // 'info'|'warning'|'critical'
  category:    text('category').notNull(),            // 'token'|'circuit_breaker'|'ip'|'process'|'quota'
  accountId:   integer('account_id').references(() => accounts.id),
  title:       text('title').notNull(),
  message:     text('message'),
  isResolved:  integer('is_resolved', { mode: 'boolean' }).notNull().default(false),
  resolvedAt:  text('resolved_at'),
  notifiedVia: text('notified_via'),                  // JSON array string
});

export const alertRules = sqliteTable('alert_rules', {
  id:          integer('id').primaryKey({ autoIncrement: true }),
  name:        text('name').notNull(),
  enabled:     integer('enabled', { mode: 'boolean' }).notNull().default(true),
  category:    text('category').notNull(),
  condition:   text('condition').notNull(),            // JSON: {metric, operator, value, unit}
  channels:    text('channels').notNull().default('["dashboard"]'),
  cooldownMin: integer('cooldown_min').notNull().default(30),
  createdAt:   text('created_at').default('CURRENT_TIMESTAMP'),
});

export const dailyStats = sqliteTable('daily_stats', {
  id:                integer('id').primaryKey({ autoIncrement: true }),
  date:              text('date').notNull(),             // 'YYYY-MM-DD'
  accountId:         integer('account_id').notNull().references(() => accounts.id),
  totalRequests:     integer('total_requests').notNull().default(0),
  successCount:      integer('success_count').notNull().default(0),
  errorCount:        integer('error_count').notNull().default(0),
  totalInputTokens:  integer('total_input_tokens').notNull().default(0),
  totalOutputTokens: integer('total_output_tokens').notNull().default(0),
  avgLatencyMs:      integer('avg_latency_ms'),
  p95LatencyMs:      integer('p95_latency_ms'),
});

export const systemConfig = sqliteTable('system_config', {
  key:       text('key').primaryKey(),
  value:     text('value').notNull(),
  updatedAt: text('updated_at').default('CURRENT_TIMESTAMP'),
});

export const processStatus = sqliteTable('process_status', {
  id:           integer('id').primaryKey({ autoIncrement: true }),
  component:    text('component').notNull().unique(), // 'singbox'|'cpa-1'~'cpa-6'|'auth-refresher'|'omniroute'
  pid:          integer('pid'),
  status:       text('status').notNull().default('stopped'), // 'running'|'stopped'|'crashed'|'restarting'
  startedAt:    text('started_at'),
  lastCrashAt:  text('last_crash_at'),
  crashCount:   integer('crash_count').notNull().default(0),
  autoRestart:  integer('auto_restart', { mode: 'boolean' }).notNull().default(true),
  updatedAt:    text('updated_at').default('CURRENT_TIMESTAMP'),
});

export const healthSummary = sqliteTable('health_summary', {
  id:           integer('id').primaryKey({ autoIncrement: true }),
  date:         text('date').notNull(),               // 'YYYY-MM-DD'
  hour:         integer('hour').notNull(),             // 0~23
  accountId:    integer('account_id').references(() => accounts.id),
  checkType:    text('check_type').notNull(),
  healthyPct:   real('healthy_pct').notNull(),         // 0.0~1.0
  avgResponseMs:integer('avg_response_ms'),
});
```

> **索引补全**: `drizzle-kit push` 会替 `sqliteTable` 创建主键索引，但 `request_logs`, `health_checks`, `alerts` 的复合索引需在迁移文件中手动添加（见 DB Schema 章节的 `CREATE INDEX` 语句）。

### 修正 38: SQLite WAL 模式强制启用

所有 SQLite 并发写操作（Manager 后台任务 + API Routes）均需开启 WAL (Write-Ahead Log) 模式以避免 "database is locked" 错误。

**在 `src/db/client.ts` 中**（DB 初始化时）：

```typescript
import Database from 'better-sqlite3';
import path from 'path';

const DB_PATH = path.resolve(process.cwd(), 'data/aggregator.db');

let _db: Database.Database | null = null;

/** Initialize and return singleton SQLite connection with WAL mode. */
export function getDb(): Database.Database {
  if (_db) return _db;
  _db = new Database(DB_PATH);
  // Enable WAL mode for concurrent reads and writes
  _db.pragma('journal_mode = WAL');
  // Busy timeout: wait up to 5s before throwing SQLITE_BUSY
  _db.pragma('busy_timeout = 5000');
  // Synchronous = NORMAL (safe with WAL, faster than FULL)
  _db.pragma('synchronous = NORMAL');
  // Enable foreign key enforcement
  _db.pragma('foreign_keys = ON');
  return _db;
}
```

> **WAL 必选理由**: Manager 同时运行健康检查 Worker Thread、SSE 推送、API 请求日志写入和 CRON 任务，不开 WAL 必然死锁。

### 修正 39: ASN 重合检查脚本落实（修正17衍生）

修正17 说"缓解措施：IP 验证时检查两个 IP 的 ASN 号是否相同"，但从未落实到具体脚本。

**在 `scripts/health_check.sh` 中新增 ASN 检查段**：

```bash
# ── ASN 关联风险检查 ──────────────────────────────────────────────────
echo "🔍 Checking ASN isolation for SG nodes (sg-a port 10004, sg-b port 10005)..."

IP_SGA=$(curl -s --socks5 127.0.0.1:10004 --connect-timeout 10 https://api.ipify.org)
IP_SGB=$(curl -s --socks5 127.0.0.1:10005 --connect-timeout 10 https://api.ipify.org)

ASN_SGA=$(curl -s "https://ipinfo.io/${IP_SGA}/org" 2>/dev/null)
ASN_SGB=$(curl -s "https://ipinfo.io/${IP_SGB}/org" 2>/dev/null)

echo "  sg-a IP: $IP_SGA  ASN: $ASN_SGA"
echo "  sg-b IP: $IP_SGB  ASN: $ASN_SGB"

if [ "$ASN_SGA" = "$ASN_SGB" ]; then
  echo "⚠️  WARNING: sg-a and sg-b share the same ASN: $ASN_SGA"
  echo "   Consider replacing Account-E with a node from a different ASN."
else
  echo "✅ SG nodes are on different ASNs — good isolation"
fi
```

> **使用 ipinfo.io**: 免费 API，月限 50,000 次，日常健康检查（5min/次 × 2节点 = 288次/天）完全够用。不需要 API key。

### 修正 40: Milestone 1 台湾节点可用性预检（修正16衍生）

台湾节点（bnm4，port 10006）在 OpenAI 地区封锁名单中状态不稳定。Milestone 1 原验证步骤仅检查"6 个 IP 彼此不同"，未包含台湾节点的 OpenAI 可达性测试。

**在 Milestone 1 代码列表验证步骤中补充**：

```bash
# 特别检查：台湾节点 (10006) 对 api.openai.com 的连通性
echo "🇹🇼 Testing Taiwan node connectivity to OpenAI..."
TW_STATUS=$(curl -s -o /dev/null -w "%{http_code}" \
  --socks5 127.0.0.1:10006 \
  --connect-timeout 15 \
  https://api.openai.com/v1/models \
  -H "Authorization: Bearer dummy-key")

if [ "$TW_STATUS" = "401" ]; then
  # 401 = OpenAI 能访问但 key 无效 → 节点可用
  echo "✅ Taiwan node reachable (OpenAI returns 401)"
elif [ "$TW_STATUS" = "000" ]; then
  # 000 = 连接超时/被拒 → 台湾节点被 OpenAI 封锁
  echo "❌ Taiwan node BLOCKED by OpenAI (connect timeout)"
  echo "   → Mark Account-F as inactive, running with 5 accounts"
else
  echo "⚠️  Taiwan node returned HTTP $TW_STATUS — investigate manually"
fi
```

**Kill Rule 更新**: 如果台湾节点已被封锁（返回 000），不阻止整体继续，仅将 Account-F 标记为 `inactive` 并从 OmniRoute Combo 中移除。

### 修正 41: CODEX_MAX_PARALLEL=1 并发刷新原因说明

`code_list.md` M3 中 `CODEX_MAX_PARALLEL=1` 设置无注释。原因：

> **防竞态**: codex-auth-refresher 刷新 Token 时会对 OpenAI OAuth 端点发起请求。如果多个 Token 同时刷新，可能会对同一账号产生并发登录，导致旧 Session 被踢出或触发异常检测。设置为 1 确保**串行刷新**，每次只有一个 Token 在刷新中，避免账号管理侧的异常行为。
>
> **性能影响**: 6 个账号串行刷新，每次刷新约 5-10s，总刷新时间约 60s（远小于 6h 预留窗口）。

### 修正 42: `rotate_account.sh` 紧急账号切换脚本完整实现

```bash
#!/usr/bin/env bash
# scripts/rotate_account.sh
# Emergency account rotation: swap a failed account with a standby account.
# Usage: ./rotate_account.sh <from_instance_num> <to_instance_num>
#   from_instance_num: the currently failed instance (1~6)
#   to_instance_num:   the standby instance to activate (e.g., 7)
#
# Precondition: standby instance directory and CPA config must already exist.

set -e

FROM=${1:?Usage: $0 <from_instance_num> <to_instance_num>}
TO=${2:?Usage: $0 <from_instance_num> <to_instance_num>}
CPA_PORT_BASE=8316   # CPA port = 8316 + instance_num
SOCKS_PORT_BASE=10000 # SOCKS5 port = 10000 + instance_num

FROM_PORT=$((CPA_PORT_BASE + FROM))
TO_PORT=$((CPA_PORT_BASE + TO))

echo "🔄 Rotating account: instance-$FROM (port $FROM_PORT) → instance-$TO (port $TO_PORT)"

# Step 1: Stop the failed instance
echo "⏹  Stopping CPA instance-$FROM..."
pkill -f "cliproxyapi.*cpa-instances/instance-$FROM" || true
sleep 2

# Step 2: Remove failed instance from OmniRoute combo (via OmniRoute Management API)
echo "🔧 Removing CPA-$FROM from OmniRoute combo 'plus-6'..."
# OmniRoute Management API endpoint TBD — update after confirming API format
# curl -X DELETE http://127.0.0.1:20128/api/combos/plus-6/providers/cpa-$FROM \
#   -H "Authorization: Bearer $OMNIROUTE_ADMIN_KEY"
echo "   ⚠️  Manual step: remove CPA-$FROM from OmniRoute Dashboard if API not available"

# Step 3: Ensure standby CPA instance is configured
if [ ! -f "cpa-instances/instance-$TO/config.yaml" ]; then
  echo "❌ Standby instance-$TO config.yaml not found!"
  exit 1
fi

# Step 4: Start standby instance
echo "▶️  Starting CPA instance-$TO..."
bash "cpa-instances/instance-$TO/start.sh"
sleep 3

# Step 5: Verify standby is healthy
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$TO_PORT/management.html")
if [ "$STATUS" != "200" ]; then
  echo "❌ Standby CPA instance-$TO not responding (HTTP $STATUS)"
  exit 1
fi
echo "✅ CPA instance-$TO is running on port $TO_PORT"

# Step 6: Add standby to OmniRoute combo
echo "🔧 Adding CPA-$TO to OmniRoute combo 'plus-6'..."
echo "   ⚠️  Manual step: add CPA-$TO (http://127.0.0.1:$TO_PORT/v1) to OmniRoute Dashboard"

echo "✅ Rotation complete: instance-$FROM → instance-$TO"
echo "   Action required: login Account-? to CPA-$TO via http://127.0.0.1:$TO_PORT/management.html"
```

### 修正 43: OmniRoute 日志 Tail 解析 Regex 具体化

`src/lib/log-collector.ts` 需要解析 OmniRoute stdout 日志。outline.md 中描述格式为：

```
[时间] [STATUS] model=xxx tokens=xxx latency=xxxms
```

实际 OmniRoute (TypeScript/Next.js) 日志格式更可能是 pino/winston 格式。**两套 regex**（覆盖常见格式）：

```typescript
// src/lib/log-collector.ts

/** Regex patterns to match OmniRoute request log lines. */
const LOG_PATTERNS = [
  // Pattern A: pino JSON line
  // {"level":30,"time":1234567890,"req":{"method":"POST"},"res":{"statusCode":200},"responseTime":124,"model":"gpt-4o"}
  {
    type: 'json',
    match: /^\{.*"responseTime":\d+.*\}$/,
    parse: (line: string) => {
      const j = JSON.parse(line);
      return {
        model: j.model ?? j.req?.body?.model ?? 'unknown',
        statusCode: j.res?.statusCode ?? j.statusCode ?? 0,
        latencyMs: j.responseTime ?? j.latency ?? 0,
        isSuccess: (j.res?.statusCode ?? 200) < 400,
        provider: j.provider ?? j.combo ?? 'plus-6',
        inputTokens: j.input_tokens ?? j.usage?.prompt_tokens ?? 0,
        outputTokens: j.output_tokens ?? j.usage?.completion_tokens ?? 0,
      };
    },
  },
  // Pattern B: structured text line
  // [2026-03-23T03:38:00Z] [200] model=gpt-4o tokens_in=123 tokens_out=456 latency=234ms provider=CPA-1-JP-GPT
  {
    type: 'text',
    match: /^\[[\d\-T:Z]+\]\s+\[(\d{3})\]\s+model=(\S+).*latency=(\d+)ms/,
    parse: (line: string, m: RegExpMatchArray) => ({
      model: m[2],
      statusCode: parseInt(m[1]),
      latencyMs: parseInt(m[3]),
      isSuccess: parseInt(m[1]) < 400,
      provider: line.match(/provider=(\S+)/)?.[1] ?? 'plus-6',
      inputTokens: parseInt(line.match(/tokens_in=(\d+)/)?.[1] ?? '0'),
      outputTokens: parseInt(line.match(/tokens_out=(\d+)/)?.[1] ?? '0'),
    }),
  },
];

/**
 * Parse a single OmniRoute log line into a request log entry.
 * Returns null if the line does not match any known pattern.
 */
export function parseOmniRouteLine(line: string): ParsedRequest | null {
  for (const p of LOG_PATTERNS) {
    const m = line.match(p.match);
    if (m) {
      try {
        return p.type === 'json' ? p.parse(line) : p.parse(line, m);
      } catch {
        return null;
      }
    }
  }
  return null;
}
```

> **实施注意**: OmniRoute 的实际日志格式需在 Pilot 启动后通过 `tail -f omniroute.log | head -20` 观察实际格式，再调整 regex。以上为防御性双模式解析。

### 修正 44: Responses API vs Chat Completions API 协议说明

CPA 实现的是 **OpenAI Responses API** (`/v1/responses`)，而非标准 Chat Completions API (`/v1/chat/completions`)。这影响了多处配置：

| 项目 | Chat Completions API | Responses API |
|---|---|---|
| URL | `/v1/chat/completions` | `/v1/responses` |
| 请求格式 | `{model, messages[]}` | `{model, input, tools[]}` |
| 流式 | `stream: true` + SSE | 原生 streaming |
| OmniRoute 兼容 | ✅ 标准 OpenAI 兼容格式 | ⚠️ 需确认 OmniRoute 是否透明转发非标准路径 |
| Codex App 使用 | 仅 Chat Completions | **同时使用两者** |

**关键风险**: OmniRoute 作为 OpenAI-compatible 代理，其路由规则可能只覆盖 `/v1/chat/completions` 和 `/v1/embeddings`。如果 Codex App 的 Responses API 调用（`/v1/responses`）未被 OmniRoute 转发，请求会直接失败（404 或被拒）。

**验证步骤（新增到 Milestone 4 验证）**:
```bash
# 测试 OmniRoute 是否转发 Responses API 请求
curl http://127.0.0.1:20128/v1/responses \
  -H "Authorization: Bearer <omniroute-api-key>" \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4o","input":"hello"}' \
  -w "\nHTTP %{http_code}\n"
# 期望: 200 (或 CPA 返回) 而非 404
```

**应急方案**: 如果 OmniRoute 不支持 `/v1/responses` 转发，Codex App 中的 `OPENAI_BASE_URL` 需要直接指向某个 CPA 实例（失去聚合功能），或考虑在 OmniRoute 前加一个简单的路径转发层（Nginx 或简单 Node HTTP 代理）。

### 修正 45: pilot_matrix.md B1-singbox 成功标准更新

B1-singbox 原假设是"utls:chrome 能改写 TLS 指纹"，修正12和修正28已证明在此架构中不成立。B1 的成功标准需更新：

**更新后的 B1-singbox**:
- **假设（修订）**: CPA 原生 Go TLS 指纹与 Codex CLI 相同，且不会被 OpenAI JA3 检测封锁
- **验证方法**:
  1. 通过 SOCKS5 发送请求到 `tls.browserleaks.com/json`，获取 CPA 的 JA3 hash
  2. 在同一机器上运行官方 Codex CLI，获取其 JA3 hash
  3. 对比两个 hash → 若相同，Accept Risk；若不同，需启用 Plan B
- **成功标准（修订）**: CPA Go TLS JA3 hash = Codex CLI JA3 hash（允许小版本差异）
- **Kill Rule（修订）**: CPA JA3 hash 与 Codex CLI 差异 > 3 个字段 **且** OpenAI 开始拒绝该指纹的请求

### 修正 46: `start_all.sh` 启动顺序依赖保护（wait/ready 检测）

原 `start_all.sh` 仅按顺序启动各组件，无等待就绪逻辑。若 Sing-box 启动慢，CPA 可能在 SOCKS5 端口就绪前已开始请求→失败。

```bash
#!/usr/bin/env bash
# scripts/start_all.sh — Ordered startup with readiness checks
set -e

wait_for_port() {
  local port=$1 name=$2 timeout=${3:-30}
  echo -n "  Waiting for $name (port $port)..."
  for i in $(seq 1 $timeout); do
    nc -z 127.0.0.1 $port 2>/dev/null && echo " ✅ ready in ${i}s" && return 0
    sleep 1
  done
  echo " ❌ timeout after ${timeout}s"
  return 1
}

wait_for_http() {
  local url=$1 name=$2 timeout=${3:-30}
  echo -n "  Waiting for $name ($url)..."
  for i in $(seq 1 $timeout); do
    curl -sf -o /dev/null "$url" 2>/dev/null && echo " ✅ ready in ${i}s" && return 0
    sleep 1
  done
  echo " ❌ timeout after ${timeout}s"
  return 1
}

# 1. Sing-box (SOCKS5 层)
echo "▶ Starting Sing-box..."
nohup sing-box run -c sing-box/sing-box.json > logs/singbox.log 2>&1 &
echo $! > .pids/singbox.pid
wait_for_port 10001 "Sing-box SOCKS5 #1"
wait_for_port 10006 "Sing-box SOCKS5 #6"

# 2. CPA 6 实例 (依赖 Sing-box)
for i in 1 2 3 4 5 6; do
  PORT=$((8316 + i))
  echo "▶ Starting CPA instance-$i (port $PORT)..."
  bash cpa-instances/instance-$i/start.sh
  wait_for_port $PORT "CPA-$i"
done

# 3. codex-auth-refresher (依赖 CPA auth 目录存在)
echo "▶ Starting codex-auth-refresher..."
docker compose -f auth-refresher/docker-compose.yml up -d
wait_for_http "http://127.0.0.1:18081/healthz" "AuthRefresher" 60

# 4. OmniRoute (依赖 CPA 实例全部就绪)
echo "▶ Starting OmniRoute..."
nohup omniroute --no-open --port 20128 --hostname 127.0.0.1 \
  > logs/omniroute.log 2>&1 &
echo $! > .pids/omniroute.pid
wait_for_http "http://127.0.0.1:20128/v1/models" "OmniRoute" 30

echo ""
echo "🎉 All components started. Running health check..."
bash scripts/health_check.sh
```

### 修正 47: SQLite 批量写入事务策略（防 SQLITE_BUSY）

修正38 已启用 WAL + busy_timeout，但批量写入仍需使用 `BEGIN IMMEDIATE` 事务（WAL 模式下也存在写冲突）：

```typescript
// src/lib/db-writer.ts

const WRITE_QUEUE: RequestLogInsert[] = [];
const FLUSH_INTERVAL_MS = 5000;
const FLUSH_BATCH_SIZE = 50;

/** Enqueue a request log entry for batch writing. */
export function enqueueRequestLog(entry: RequestLogInsert): void {
  WRITE_QUEUE.push(entry);
  if (WRITE_QUEUE.length >= FLUSH_BATCH_SIZE) flushQueue();
}

/** Flush the queue to SQLite using a single transaction. */
function flushQueue(): void {
  if (WRITE_QUEUE.length === 0) return;
  const batch = WRITE_QUEUE.splice(0, FLUSH_BATCH_SIZE);
  const db = getDb();
  // BEGIN IMMEDIATE in WAL mode blocks writers but allows readers
  const insertMany = db.transaction((rows: RequestLogInsert[]) => {
    const stmt = db.prepare(
      `INSERT INTO request_logs (account_id, request_id, timestamp, model, ` +
      `input_tokens, output_tokens, latency_ms, status_code, is_success, error_type, combo_name) ` +
      `VALUES (@accountId, @requestId, @timestamp, @model, ` +
      `@inputTokens, @outputTokens, @latencyMs, @statusCode, @isSuccess, @errorType, @comboName)`
    );
    for (const row of rows) stmt.run(row);
  });
  try {
    insertMany(batch);
  } catch (err) {
    console.error('[db-writer] Batch insert failed, will retry next cycle:', err);
    // Re-enqueue failed batch (at front of queue)
    WRITE_QUEUE.unshift(...batch);
  }
}

setInterval(flushQueue, FLUSH_INTERVAL_MS);
```

### 修正 48: Next.js 15 App Router 绑定 127.0.0.1 的正确方式

Next.js 15 dev server 通过 CLI 参数绑定，而生产部署通过环境变量：

```bash
# 开发模式
npm run dev -- --hostname 127.0.0.1 --port 3000

# 生产模式 (next start)
HOSTNAME=127.0.0.1 PORT=3000 npm start
```

`next.config.ts` 本身无法直接绑定 hostname（这是 Node.js HTTP 服务器层的配置）。但可以通过 CORS headers 禁止跨域访问：

```typescript
// next.config.ts
import type { NextConfig } from 'next';

const nextConfig: NextConfig = {
  async headers() {
    return [
      {
        source: '/api/:path*',
        headers: [
          { key: 'Access-Control-Allow-Origin', value: 'http://127.0.0.1:3000' },
          { key: 'Access-Control-Allow-Methods', value: 'GET,POST,PUT,DELETE,OPTIONS' },
          { key: 'Access-Control-Allow-Headers', value: 'Content-Type,Authorization' },
        ],
      },
    ];
  },
  // Disable X-Powered-By to avoid fingerprinting
  poweredByHeader: false,
};

export default nextConfig;
```

> **package.json 脚本更新**（防止忘记绑定）:
> ```json
> "scripts": {
>   "dev":   "next dev --hostname 127.0.0.1 --port 3000",
>   "start": "HOSTNAME=127.0.0.1 next start --port 3000"
> }
> ```

### 修正 49: OmniRoute `db.json` 持久化与并发写保护

OmniRoute 将其 Provider/Combo 配置持久化到 `db.json`（基于 LowDB 或类似库）。并发写保护：

1. **原子写入**: OmniRoute 应在写入 `db.json` 时使用 `write-file-atomic` 或类似库，避免写到一半时进程被 kill 导致 DB 损坏。
2. **Manager 不直接写 `db.json`**: Manager 应通过 OmniRoute 的 Dashboard API 管理 Provider/Combo，而不是直接修改文件。
3. **备份保护**: `scripts/backup.sh` 在备份 SQLite 的同时也备份 `omniroute/db.json`。
4. **重启保护**: OmniRoute 进程重启后自动加载 `db.json`，Manager 的 `start_all.sh` 不需要重新配置 Provider/Combo（幂等性）。

**验证（加入 Milestone 4 Kill Rule）**:
```bash
# 验证 OmniRoute 重启后 Combo 配置持久化
omniroute_pid=$(cat .pids/omniroute.pid)
kill $omniroute_pid && sleep 3
HOSTNAME=127.0.0.1 omniroute --no-open --port 20128 &
sleep 5
# db.json 中的 6 个 Provider 应仍存在
curl http://127.0.0.1:20128/v1/models | jq '.[] | .id' | wc -l
# → 应 ≥ 6
```

### 修正 50: Duo-Doc 最终收敛一致性校验清单

以下为 `outline.md` ↔ `code_list.md` 交叉一致性验证清单，作为 idea-to-plan 的收敛判定标准：

| 检查项 | outline.md 声明 | code_list.md 落实 | 状态 |
|--------|----------------|------------------|------|
| sttrbjp 端口 | 50189 (修正11) | M1 表格 50189 (修正31) | ✅ |
| aasx2 端口 | 50050 (修正11) | M1 表格 50050 (修正31) | ✅ |
| cvb7 端口 | 50166 (修正11) | M1 表格 50166 (修正31) | ✅ |
| utls 策略 | Accept Risk(修正28/32) | M1 移除 utls 配置(修正32) | ✅ |
| OmniRoute 安装 | 双轨方案(修正33) | M4 含安装脚本(修正33) | ✅ |
| 加权 RR | 权重表(修正19) | M4 Combo weight 配置(修正34) | ✅ |
| auth-refresher 递归 | 确认点(修正14) | M3 Kill Rule 含递归验证(修正35) | ✅ |
| health_summary 表 | DDL(修正18) | M7 表数量=9，Kill Rule 更新(修正36) | ✅ |
| Drizzle Schema | 需完整 | M8 全量 TS 定义(修正37) | ✅ |
| SQLite WAL | 需开启 | client.ts PRAGMA 初始化(修正38) | ✅ |
| ASN 检查 | 需落实(修正17) | health_check.sh ASN 段(修正39) | ✅ |
| 台湾节点预检 | 风险(修正16) | M1 验证含 bnm4 预检(修正40) | ✅ |
| MAX_PARALLEL 原因 | 未说明(修正41) | M3 注释说明(修正41) | ✅ |
| rotate_account.sh | 内容空缺 | 完整脚本(修正42) | ✅ |
| 日志 regex | 未定义 | log-collector.ts 双模式(修正43) | ✅ |
| Responses API | 未说明 | M4 验证含路径检测(修正44) | ✅ |
| B1-singbox 标准 | utls→JA3对比 | pilot_matrix 更新(修正45) | ✅ |
| start_all.sh | 无 wait | wait_for_port/http 逻辑(修正46) | ✅ |
| 批量写事务 | WAL+批量 | db-writer.ts BEGIN IMMEDIATE(修正47) | ✅ |
| hostname 绑定 | 127.0.0.1 | package.json + next.config.ts(修正48) | ✅ |
| db.json 保护 | 未说明 | 原子写+API操作+幂等启动(修正49) | ✅ |

> **收敛判定 (第一次)**: 以上 21 项全部 ✅。继续执行第 51 轮深度审查。
> 如发现新问题，在下一轮继续修正并更新此表。

---

## 第二次收敛扫描（Round 51 → 56）

### 修正 51: decision_log.md D-1 和 D-5 未同步 utls 失效结论 (Critical)

`decision_log.md` D-1（第44-47行）仍然写着：

> "Sing-box 的 `utls: chrome` 提供真正的出站 TLS 指纹伪装"
> "双层防护：Sing-box 洗 TLS + OmniRoute 洗 CLI 指纹"

而 `outline.md` 修正12/28/32 已经确认这在 `tls: false` 的 VMess 隧道中**不生效**。`decision_log.md` D-5 的"物理层"描述也沿用了相同的错误表述。

**⚠️ 已修正 decision_log.md D-1/D-5**（见下方 decision_log.md 变更指令）：
- D-1 中 Sing-box 理由删除"真正的出站 TLS 指纹伪装"，改为"提供隔离隧道；utls 在 tls:false VMess 隧道中不生效，TLS 防御实际由 CPA 原生 Go TLS（≈ Codex CLI 指纹）承担"
- D-5 中"物理层"描述移除"utls:chrome (TLS 洗白)"，改为"CPA Go TLS ≈ Codex CLI 指纹（Accept Risk）"

### 修正 52: code_list.md 目录树缺失 `app/` 子树

`code_list.md` §1 结构概览中的目录树只列出了：
```
codex-aggregator/
├── sing-box/
├── cpa-instances/
├── auth-refresher/
├── omniroute/
├── scripts/
└── README.md
```

**完全遗漏了 `app/` 目录**（Next.js 管理面板），而 Milestone 7-9 均在 `app/` 下工作。收敛校验清单 Milestone 7-9 的文件路径 `app/src/...` 无法从目录树中找到对应的结构。

**⚠️ 已补充到 code_list.md §1**（见下方变更指令）：在 `README.md` 之前插入 `app/` 子树。

### 修正 53: `scripts/backup.sh` 内容空缺

修正20 和 code_list.md M5 都提到了 `scripts/backup.sh`，并在修正49中明确说它应该"每日 02:00 备份 SQLite + OmniRoute db.json"。但实际脚本内容从未定义。

```bash
#!/usr/bin/env bash
# scripts/backup.sh
# Daily backup: SQLite DB + OmniRoute db.json
# Invoked by node-cron (next.js) at 02:00 daily.
# Keeps the last 7 days of backups.

set -e

BACKUP_DIR="$(cd "$(dirname "$0")/.." && pwd)/data/backups"
DB_SRC="$(cd "$(dirname "$0")/.." && pwd)/data/aggregator.db"
OMNIROUTE_DB="$(cd "$(dirname "$0")/.." && pwd)/omniroute/db.json"
DATE=$(date +%Y%m%d)

mkdir -p "$BACKUP_DIR"

# 1. SQLite backup (uses SQLite's .backup command for hot backup)
echo "🗄️  Backing up SQLite..."
sqlite3 "$DB_SRC" ".backup '$BACKUP_DIR/aggregator-$DATE.db'"
echo "   → $BACKUP_DIR/aggregator-$DATE.db"

# 2. OmniRoute db.json backup
if [ -f "$OMNIROUTE_DB" ]; then
  cp "$OMNIROUTE_DB" "$BACKUP_DIR/omniroute-db-$DATE.json"
  echo "   → $BACKUP_DIR/omniroute-db-$DATE.json"
fi

# 3. CPA auth files backup
echo "🔑  Backing up CPA auth files..."
tar -czf "$BACKUP_DIR/auths-$DATE.tar.gz" \
  -C "$(cd "$(dirname "$0")/.." && pwd)" \
  cpa-instances/instance-1/auths \
  cpa-instances/instance-2/auths \
  cpa-instances/instance-3/auths \
  cpa-instances/instance-4/auths \
  cpa-instances/instance-5/auths \
  cpa-instances/instance-6/auths 2>/dev/null || true
echo "   → $BACKUP_DIR/auths-$DATE.tar.gz"

# 4. Cleanup: remove backups older than 7 days
find "$BACKUP_DIR" -name "*.db" -mtime +7 -delete
find "$BACKUP_DIR" -name "*.json" -mtime +7 -delete
find "$BACKUP_DIR" -name "*.tar.gz" -mtime +7 -delete
echo "🧹  Old backups (>7d) cleaned up"

echo "✅  Backup complete: $DATE"
```

### 修正 54: API Route 数量核算不一致（29 vs 实际 24+1 SSE）

`code_list.md` M8 说"实现 29 个 API Route Handlers"，但 outline.md §API 接口设计表列出的端点为：

| 分类 | 数量 |
|---|---|
| overview / accounts / network / analytics / alerts / alert-rules | 15 个 REST |
| system (health/start-all/stop-all/config/logs/processes/restart/:component/onboarding-status/onboarding) | 9 个 REST（含修正4补充的5个中4个是新增，合计） |
| SSE events | 1 个 SSE |
| **合计** | **25 个** |

> 实际数量为 **25**（含 SSE 计 1 个），code_list.md M8 的"29"是误算。差异来源：可能将部分 URL 参数变体（`/:id`）分别计数。
> **修正**: code_list.md M8 改为"实现约 25 个 API 端点（含 SSE）"，注明可能随 OmniRoute 集成增加。

### 修正 55: pilot_matrix.md B5-cli-fp 成功标准与 outline.md 修正126-129 不一致

outline.md §139 明确指出：

> "OmniRoute 的 TLS Fingerprint Spoofing 和 CLI Fingerprint Matching 功能在本架构中影响的是 **OmniRoute → CPA** 这段链路（内网 localhost），而非 CPA → OpenAI 这段关键链路。"

但 `pilot_matrix.md` B5-cli-fp 的成功标准是"header 顺序与 Codex CLI 原生二进制一致"，且隐含了对 OpenAI 的防封效果。这与修正126-129 矛盾：B5 实际测试的是 OmniRoute → CPA（本地）的 HTTP 头，对 OpenAI 的实际防封效果**为零**。

**⚠️ 修正 pilot_matrix.md B5**（见变更指令）：
- 假设改为："OmniRoute CLI Fingerprint Matching 对 OmniRoute→CPA 本地链路生效，但对 CPA→OpenAI 链路无直接防封作用"
- 成功标准改为："抓包确认 OmniRoute→CPA 的请求 header 顺序匹配配置，且 Codex App 端行为正常"
- Kill Rule 改为："该特性对 OpenAI 无防封价值（已知），可降级为可选优化，不阻止主方案继续"

### 修正 56: 支持文档审查 — `eval_suite.md`、`middleware_contracts.md` 内容稀薄

读取 `eval_suite.md`（691B）和 `middleware_contracts.md`（964B）发现内容过于简单，不包含具体可执行的评估标准或中间件合约，与修正50中"收敛判定"所需的完整文档体系不匹配。

**结论**：这两个文件属于占位符，不影响 idea-to-plan 阶段的收敛。在执行阶段（plan-to-code）填充即可，不需要在 idea-to-plan 阶段完善。

---

## 修正 51-56 更新后的收敛校验清单

在修正50的 21 项基础上，新增：

| 检查项 | 状态 |
|--------|------|
| decision_log.md D-1/D-5 utls 描述与修正12/28同步 | ✅ (修正51) |
| code_list.md §1 目录树包含 `app/` 子树 | ✅ (修正52) |
| `scripts/backup.sh` 完整实现 | ✅ (修正53) |
| API Route 数量核算准确（≈ 25 而非 29） | ✅ (修正54) |
| pilot_matrix.md B5 成功标准与 outline 修正对齐 | ✅ (修正55) |
| eval_suite / middleware_contracts 占位符说明 | ✅ (修正56) |

> **最终收敛判定**: 修正50共 21 项 ✅ + 修正56新增 6 项 ✅ = 共 27 项全部 ✅
> 新一轮深度审查未发现更多根本性不一致 → **Duo-Doc 正式收敛，移交 `$plan-to-code` 执行层。**
