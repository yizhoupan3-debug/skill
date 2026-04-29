# Code List: OpenAI Plus 聚合系统执行计划 v2

## 0. 语言计划 (Language Policy)

- `primary_language`: **Go** (CPA + codex-auth-refresher)
- `secondary_languages`: **TypeScript** (OmniRoute)、**YAML/JSON** (配置文件)、**Shell** (启动脚本)
- `language_selection_reason`: CPA 和 codex-auth-refresher 均为 Go 项目，OmniRoute 为 TypeScript/Next.js 项目。我们不自研核心代理引擎，而是组合成熟开源方案，因此实际编写的代码以配置文件 (YAML/JSON) 和编排脚本 (Shell) 为主。
- `module_language_map`:
  - 代理实例层：Go (CPA binary, 直接使用)
  - Token 续期层：Go (codex-auth-refresher, Docker 部署)
  - 聚合网关层：TypeScript (OmniRoute, npm 安装)
  - TLS 洗白层：Go (Sing-box binary, 直接使用)
  - 编排脚本层：Shell (启动/监控/切换脚本)
- `runtime_or_toolchain_notes`:
  - Node.js ≥ 18 LTS (OmniRoute)
  - Go ≥ 1.21 (可选，仅在需要源码编译 codex-auth-refresher 时)
  - Docker (codex-auth-refresher 推荐 Docker 部署)
  - Homebrew (Sing-box: `brew install sing-box`)
- `verification_commands`:
  - `cliproxyapi --version` (CPA)
  - `npm list -g omniroute` (OmniRoute)
  - `docker ps | grep codex-auth-refresher`
  - `sing-box version`
  - `curl http://127.0.0.1:20128/v1/models` (OmniRoute API)
  - `curl http://127.0.0.1:8317/v1/models` (CPA-A)
  - `curl http://127.0.0.1:18081/healthz` (codex-auth-refresher)

## 1. 结构概览 (Code Structure)

```
codex-aggregator/
├── sing-box/
│   ├── sing-box.json              # Sing-box 配置 (6 outbound + 6 inbound)
│   └── start_singbox.sh           # Sing-box 启动脚本
├── cpa-instances/
│   ├── instance-1/                # 🇯🇵 sttrbjp (日本GPT)
│   │   ├── config.yaml            # port:8317, proxy:10001
│   │   ├── auths/                 # codex-*.json
│   │   └── start.sh
│   ├── instance-2/                # 🇯🇵 dcv69 (日本)
│   │   ├── config.yaml            # port:8318, proxy:10002
│   │   ├── auths/
│   │   └── start.sh
│   ├── instance-3/                # 🇺🇸 ggsd5 (美国)
│   │   ├── config.yaml            # port:8319, proxy:10003
│   │   ├── auths/
│   │   └── start.sh
│   ├── instance-4/                # 🇸🇬 aasx2 (新加坡)
│   │   ├── config.yaml            # port:8320, proxy:10004
│   │   ├── auths/
│   │   └── start.sh
│   ├── instance-5/                # 🇸🇬 cvb7 (新加坡)
│   │   ├── config.yaml            # port:8321, proxy:10005
│   │   ├── auths/
│   │   └── start.sh
│   ├── instance-6/                # 🇹🇼 bnm4 (台湾)
│   │   ├── config.yaml            # port:8322, proxy:10006
│   │   ├── auths/
│   │   └── start.sh
│   └── add_instance.sh            # 快速添加新实例的模板脚本
├── auth-refresher/
│   ├── docker-compose.yml         # codex-auth-refresher Docker 编排
│   └── .env                       # 环境变量 (刷新策略/邮件告警)
├── omniroute/
│   ├── .env                       # OmniRoute 环境变量
│   └── db.json                    # OmniRoute 持久化数据 (Providers/Combos)
├── scripts/
│   ├── start_all.sh               # 一键启动 (Sing-box→CPA×6→AuthRefresher→OmniRoute)
│   ├── stop_all.sh                # 逆序停止
│   ├── health_check.sh            # 全链路健康检查 (6 IP隔离验证 + ASN重合检查)
│   ├── rotate_account.sh          # 紧急账号切换脚本
│   ├── install_omniroute.sh       # OmniRoute 双轨安装（npm/GitHub/Docker）
│   └── backup.sh                  # 每日备份 SQLite + OmniRoute db.json + auth files
├── app/                           # ← 修正52: Next.js 管理面板 (Milestone 7-9)
│   ├── src/
│   │   ├── app/
│   │   │   ├── (dashboard)/       # 6 页面布局
│   │   │   │   ├── layout.tsx     # Sidebar 导航布局
│   │   │   │   ├── page.tsx       # Overview Dashboard
│   │   │   │   ├── accounts/page.tsx
│   │   │   │   ├── network/page.tsx
│   │   │   │   ├── analytics/page.tsx
│   │   │   │   ├── alerts/page.tsx
│   │   │   │   └── settings/page.tsx
│   │   │   ├── setup/page.tsx     # Onboarding Wizard (首次设置)
│   │   │   └── api/               # ~25 API 端点 (含 1 SSE)
│   │   ├── components/            # 自定义组件 (StatusCard, Charts等)
│   │   ├── db/
│   │   │   ├── schema.ts          # Drizzle 全量 Schema (9 张表)
│   │   │   └── client.ts          # SQLite 单例 + WAL + busy_timeout
│   │   └── lib/                   # 后端模块
│   │       ├── process-manager.ts
│   │       ├── health-checker.ts
│   │       ├── token-syncer.ts
│   │       ├── log-collector.ts
│   │       ├── db-writer.ts
│   │       ├── alert-engine.ts
│   │       └── cron-tasks.ts
│   ├── data/
│   │   ├── aggregator.db          # SQLite 数据库
│   │   └── backups/               # 每日备份目录
│   ├── drizzle.config.ts
│   ├── next.config.ts
│   └── package.json
└── README.md                      # 部署手册
```

## 2. 里程碑切片 (Milestone Slices)

### Milestone 1: 网络隧道层 — Sing-box 6 节点配置
- **修改/创建文件**: `sing-box/sing-box.json`, `sing-box/start_singbox.sh`
- **实现内容**:
  - 从 `~/.config/clash/速云梯.yaml` 提取 6 个非香港节点的 VMess 参数（程序化提取，禁止手写）
  - 转换为 Sing-box outbound 格式，配置 6 个 outbound：
    | outbound tag | 服务器 | 端口 | 地区 | 绑定 SOCKS5 |
    |---|---|---|---|---|
    | jp-gpt | sttrbjp.cdn.node.a.datahub7.net | **50189** | 🇯🇵 | 10001 |
    | jp | dcv69.cdn.node.a.datahub7.net | 50208 | 🇯🇵 | 10002 |
    | us | ggsd5.cdn.node.a.datahub7.net | 50103 | 🇺🇸 | 10003 |
    | sg-a | aasx2.cdn.node.a.datahub7.net | **50050** | 🇸🇬 | 10004 |
    | sg-b | cvb7.cdn.node.a.datahub7.net | **50166** | 🇸🇬 | 10005 |
    | tw | bnm4.cdn.node.a.datahub7.net | 50068 | 🇹🇼 | 10006 |
  > ⚠️ 端口已根据 Clash YAML 实际数据修正（修正31）。勿手写，以 YAML 程序化提取为准。
  - **utls 策略（修正32）**: `tls: false` 隧道中 utls 不生效，**不配置 utls 块**。TLS 指纹由 CPA 原生 Go TLS 保障（≈ Codex CLI 指纹），Accept Risk。
  - 6 个 SOCKS5 inbound (10001~10006)，通过 routing rules 一对一绑定
  - VMess 公共参数：`uuid: 577edbac-...`，`network: ws`，`path: /009c250b-...`
- **验证方式**:
  ```bash
  sing-box run -c sing-box/sing-box.json
  
  # 验证 6 个出口 IP 彼此不同
  for port in 10001 10002 10003 10004 10005 10006; do
    echo "Port $port: $(curl -s --socks5 127.0.0.1:$port https://api.ipify.org)"
  done
  # → 应输出 6 个不同的 IP

  # 验证 CPA Go TLS JA3 指纹与 Codex CLI 一致 (修正32/45)
  echo "CPA JA3:"
  curl -s --socks5 127.0.0.1:10001 https://tls.browserleaks.com/json | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('ja3_hash','N/A'))"
  echo "(手动对比官方 Codex CLI 在同机器上访问的 JA3：应一致)"

  # 台湾节点 OpenAI 可达性预检 (修正40)
  TW_STATUS=$(curl -s -o /dev/null -w "%{http_code}" \
    --socks5 127.0.0.1:10006 --connect-timeout 15 \
    https://api.openai.com/v1/models -H "Authorization: Bearer dummy-key")
  [ "$TW_STATUS" = "401" ] && echo "✅ TW reachable" || echo "⚠️ TW returned HTTP $TW_STATUS"
  ```
- **Kill Rule**: 任意两个端口返回相同 IP → 配置有误，不得继续
- **Kill Rule²**: 台湾节点 HTTP 000（连接超时）→ 标记 Account-F inactive，改为 5 账号运行（不阻止整体继续）

### Milestone 2: 代理实例层 — CPA 6 实例隔离部署
- **修改/创建文件**: `cpa-instances/instance-{1..6}/config.yaml`, `start.sh`
- **实现内容**:
  - 6 个 CPA 实例，每个三位一体绑定：
    | 实例 | 端口 | proxy-url | auth-dir |
    |---|---|---|---|
    | CPA-1 | 8317 | socks5://127.0.0.1:10001 | ./auths |
    | CPA-2 | 8318 | socks5://127.0.0.1:10002 | ./auths |
    | CPA-3 | 8319 | socks5://127.0.0.1:10003 | ./auths |
    | CPA-4 | 8320 | socks5://127.0.0.1:10004 | ./auths |
    | CPA-5 | 8321 | socks5://127.0.0.1:10005 | ./auths |
    | CPA-6 | 8322 | socks5://127.0.0.1:10006 | ./auths |
  - 每个实例开启 Management API（`management.enabled: true`）
  - 所有实例仅绑定 `127.0.0.1`
- **验证方式**:
  ```bash
  # 启动 6 个 CPA 实例
  for i in 1 2 3 4 5 6; do
    ./cpa-instances/instance-$i/start.sh
  done
  
  # 验证 6 个 Management API
  for port in 8317 8318 8319 8320 8321 8322; do
    echo "CPA :$port → $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$port/management.html)"
  done
  # → 全部返回 200
  
  # 分别在每个 CPA 管理页登录 Plus 账号 A~F
  # 验证各自 auths/ 目录生成了 codex-*.json
  for i in 1 2 3 4 5 6; do
    echo "Instance-$i: $(ls cpa-instances/instance-$i/auths/ 2>/dev/null || echo 'empty')"
  done
  ```
- **Kill Rule**: 如果任意两个 CPA 实例的 auth 文件出现交叉 → 隔离失败

### Milestone 3: Token 续期层 — codex-auth-refresher Sidecar
- **修改/创建文件**: `auth-refresher/docker-compose.yml`, `auth-refresher/.env`
- **实现内容**:
  - Docker Compose 配置，bind-mount 6 个 CPA 的 auth 目录到同一 `/data/auth` 下
  - 卷映射示意：`./cpa-instances/instance-{1..6}/auths:/data/auth/instance-{1..6}:rw`
  - 环境变量配置：
    ```
    CODEX_AUTH_DIR=/data/auth
    CODEX_REFRESH_BEFORE=6h
    CODEX_REFRESH_MAX_AGE=20h
    CODEX_SCAN_INTERVAL=5m
    CODEX_MAX_PARALLEL=1   # 串行刷新，防竞态（修正41）：并发刷新会触发 OAuth 多端登录异常
    CODEX_HTTP_TIMEOUT=30s
    CODEX_AUTH_ENABLE_CODEX=true
    CODEX_WEB_ENABLE=true
    CODEX_LOG_FORMAT=json
    ```
  - 可选 Email 告警配置
- **验证方式**:
  ```bash
  docker compose -f auth-refresher/docker-compose.yml up -d
  
  # 检查健康
  curl http://127.0.0.1:18081/healthz   # → OK
  curl http://127.0.0.1:18081/readyz    # → ready=true
  
  # 验证子目录递归扫描支持 (修正35)
  docker exec codex-auth-refresher find /data/auth -name "codex-*.json" | head -10
  # 若仅找到 /data/auth/*.json 而非 /data/auth/instance-*/codex-*.json
  # → 不支持递归，需改为 symlink 聚合方案（见修正35）
  
  # 查看 Token 状态
  curl http://127.0.0.1:18081/v1/status
  # → 应显示 6 个 tracked files + refresh schedule
  ```
- **Kill Rule**: 如果 readyz 返回 ready=false 且 10 分钟内未恢复 → 检查 auth 文件权限
- **Kill Rule²（修正35）**: 递归扫描不支持 → 停止继续，先执行 symlink 聚合方案再重启

### Milestone 4: 聚合网关层 — OmniRoute 部署与配置
- **修改/创建文件**: `omniroute/.env`, `scripts/install_omniroute.sh`, OmniRoute Dashboard 配置
- **实现内容**:
  - 安装 OmniRoute（双轨方案，修正33）：
    ```bash
    bash scripts/install_omniroute.sh   # 自动探测 npm/GitHub/Docker 三条路径
    omniroute --version
    ```
  - 配置环境变量：
    ```
    PORT=20128
    HOSTNAME=127.0.0.1
    JWT_SECRET=<random-64-chars>
    INITIAL_PASSWORD=<strong-password>
    REQUIRE_API_KEY=true
    ```
  - 在 Dashboard 中添加 6 个 Custom OpenAI-Compatible Provider：
    | Provider 名称 | Base URL |
    |---|---|
    | CPA-1-JP-GPT | http://127.0.0.1:8317/v1 |
    | CPA-2-JP | http://127.0.0.1:8318/v1 |
    | CPA-3-US | http://127.0.0.1:8319/v1 |
    | CPA-4-SG-A | http://127.0.0.1:8320/v1 |
    | CPA-5-SG-B | http://127.0.0.1:8321/v1 |
    | CPA-6-TW | http://127.0.0.1:8322/v1 |
  - 创建 Combo "plus-6"：加权 Round-Robin（修正19/34），fallback → iFlow/Kiro:
    ```json
    { "strategy": "weighted-round-robin",
      "providers": [
        {"id":"cpa-1-jp-gpt","weight":20}, {"id":"cpa-2-jp","weight":20},
        {"id":"cpa-3-us","weight":15},    {"id":"cpa-4-sg-a","weight":17},
        {"id":"cpa-5-sg-b","weight":17},  {"id":"cpa-6-tw","weight":11}
      ] }
    ```
    > 若 Dashboard 不支持 weight 字段，退化为普通 round-robin（可接受）
  - 配置 Rate Limiter (RPM/TPM)
- **验证方式**:
  ```bash
  HOSTNAME=127.0.0.1 omniroute --no-open --port 20128
  
  curl http://127.0.0.1:20128/v1/models
  # → 应列出 6 个 CPA 的模型
  
  # 测试 Chat Completions
  curl http://127.0.0.1:20128/v1/chat/completions \
    -H "Authorization: Bearer <omniroute-api-key>" \
    -H "Content-Type: application/json" \
    -d '{"model":"gpt-4o","messages":[{"role":"user","content":"test"}]}'

  # 测试 Responses API 透传（修正44）
  curl http://127.0.0.1:20128/v1/responses \
    -H "Authorization: Bearer <key>" -H "Content-Type: application/json" \
    -d '{"model":"gpt-4o","input":"hello"}' -w "\nHTTP %{http_code}\n"
  # 期望 200，若返回 404 → OmniRoute 不透传 /v1/responses，需应急方案

  # 发送 12 次请求，验证 6 个 CPA 实例日志均有命中
  for i in $(seq 1 12); do
    curl -s http://127.0.0.1:20128/v1/chat/completions \
      -H "Authorization: Bearer <key>" -H "Content-Type: application/json" \
      -d '{"model":"gpt-4o","messages":[{"role":"user","content":"ping"}]}' > /dev/null
  done

  # 验证重启后 Combo 配置持久化（修正49）
  kill $(cat .pids/omniroute.pid) && sleep 3
  HOSTNAME=127.0.0.1 omniroute --no-open --port 20128 &
  sleep 5 && curl http://127.0.0.1:20128/v1/models | jq 'length'
  # → 应 ≥ 6
  ```
- **Kill Rule**: OmniRoute 无法访问任何 CPA 实例 → 检查端口/防火墙
- **Kill Rule²（修正44）**: `/v1/responses` 返回 404 → Codex App 无法使用聚合，执行应急方案（Nginx 路径转发或直连 CPA）

### Milestone 5: 编排脚本 — 一键启停 + 健康检查
- **修改/创建文件**: `scripts/start_all.sh`, `scripts/stop_all.sh`, `scripts/health_check.sh`, `scripts/rotate_account.sh`, `scripts/install_omniroute.sh`, `scripts/backup.sh`
- **实现内容**:
  - `start_all.sh`: 按顺序启动 Sing-box → CPA×6 → codex-auth-refresher → OmniRoute，**含 wait_for_port/wait_for_http 就绪检测**（修正46）
  - `stop_all.sh`: 逆序停止所有 9 个进程/容器
  - `health_check.sh`: 检查全链路 + ASN 重合检查（修正39）：
    1. Sing-box 6 个 SOCKS5 端口 (×6)
    2. CPA 6 个 Management API (×6)
    3. codex-auth-refresher /readyz (×1)
    4. OmniRoute /v1/models (×1)
    5. 6 个出口 IP 互不相同 (×1)
    6. **sg-a / sg-b ASN 不重合检查（修正39）**
  - `rotate_account.sh`: 紧急账号切换脚本（完整实现，修正42）
  - `install_omniroute.sh`: npm/GitHub/Docker 三路自动选择（修正33）
  - `backup.sh`: SQLite + OmniRoute db.json 每日备份（修正49）
- **验证方式**:
  ```bash
  ./scripts/start_all.sh     # 应无错误启动全部 9 个服务，含 wait 就绪输出
  ./scripts/health_check.sh  # 应全绿（含 ASN 检查）
  ./scripts/stop_all.sh      # 应干净停止所有进程
  # 测试紧急切换（若有备用 instance-7）
  ./scripts/rotate_account.sh 6 7
  ```

### Milestone 6: 防泄密配置分发 — 安全 API 入口
- **修改/创建文件**: 复用 OmniRoute 的 API Key Management
- **实现内容**:
  - 在 OmniRoute Dashboard → Endpoints 生成受限 API Key
  - 配置 Codex App：
    ```
    OPENAI_BASE_URL=http://127.0.0.1:20128
    OPENAI_API_KEY=<omniroute-generated-key>
    ```
  - 确保 Codex App 的所有请求都通过 OmniRoute 中转，永远看不到真实的 Plus Session Token
- **验证方式**:
  - 用 Codex App 发起请求，检查 OmniRoute 日志中只有 OmniRoute Key
  - 在 CPA 日志中验证是 Plus Token 在实际调用
  - Codex App 的任何网络抓包都不应泄露 Plus Token

### Milestone 7: App 骨架 — Next.js 项目搭建
- **修改/创建文件**: `app/` 目录（Next.js 15 项目根目录）
- **实现内容**:
  - `npx -y create-next-app@latest ./app --ts --tailwind --eslint --app --src-dir --import-alias "@/*"` 初始化
  - 安装核心依赖：`shadcn@latest` + `better-sqlite3` + `drizzle-orm` + `recharts` + `react-leaflet` + `next-themes` + `node-cron` + `sonner` + `@types/better-sqlite3`
  - 初始化 Drizzle：`drizzle.config.ts` + `src/db/schema.ts`（**9 张表**，含 health_summary — 修正36）
  - 创建 `src/db/client.ts`：开启 WAL 模式 + busy_timeout + foreign_keys（修正38）
  - 运行 `drizzle-kit push` 创建 SQLite DB
  - 配置 `next.config.ts`：CORS headers（修正48）
  - `package.json` scripts 更新：`"dev": "next dev --hostname 127.0.0.1 --port 3000"`（修正48）
  - shadcn init + 安装组件：`button card table badge progress dialog sheet sidebar separator`
- **验证方式**:
  ```bash
  cd app && npm run dev
  # → 浏览器打开 http://127.0.0.1:3000 看到默认页面
  
  # 验证 DB 创建（9 张表）
  ls data/aggregator.db  # → 文件存在
  sqlite3 data/aggregator.db ".tables"
  # → accounts request_logs health_checks alerts alert_rules daily_stats system_config process_status health_summary

  # 验证 WAL 模式已启用
  sqlite3 data/aggregator.db "PRAGMA journal_mode;"
  # → wal
  ```
- **Kill Rule**: DB 表数量 ≠ 9 → Schema 不完整（修正36：新增了 health_summary 表）
- **Kill Rule²**: `PRAGMA journal_mode` 不是 `wal` → WAL 初始化失败，检查 client.ts

### Milestone 8: App 后端 — API Routes + 后台任务
- **修改/创建文件**: `app/src/app/api/**/*.ts`, `app/src/lib/**/*.ts`
- **实现内容**:
  - 实现约 25 个 API 端点（含 1 个 SSE）（见 outline.md API 接口设计章节；修正54：原"29"为误算，实际≈25，随 OmniRoute 集成可能微调）
  - 实现进程管理模块 `src/lib/process-manager.ts`：
    - `startComponent(name)` / `stopComponent(name)` / `restartComponent(name)`
    - 自动恢复：崩溃检测 + 重启最多 3 次/小时
  - 实现健康检查 Worker `src/lib/health-checker.ts`：
    - 60s 间隔检查所有组件存活
    - 5min 间隔 IP 隔离验证 + ASN 重合检查（修正39）
  - 实现 Token 同步模块 `src/lib/token-syncer.ts`
  - 实现 SSE 端点 `src/app/api/sse/events/route.ts`
  - 实现 OmniRoute 日志 tail 采集器 `src/lib/log-collector.ts`（双模式 regex，修正43）
  - 实现批量写入队列 `src/lib/db-writer.ts`（BEGIN IMMEDIATE 事务，修正47）
  - 实现告警评估引擎 `src/lib/alert-engine.ts`
  - 实现 CRON 任务 `src/lib/cron-tasks.ts`：
    - 日统计聚合 + health_summary 小时级聚合（修正36）
    - 历史清理
    - SQLite DB 每日备份 + `omniroute/db.json` 备份（修正49）
  - Drizzle Schema 全量定义（`src/db/schema.ts`）：9 张表完整 TS 类型（修正37，见 outline.md 修正37 完整代码）
- **验证方式**:
  ```bash
  # API 冒烟测试
  curl http://127.0.0.1:3000/api/overview          # → 200 + JSON
  curl http://127.0.0.1:3000/api/accounts           # → 200 + []
  curl http://127.0.0.1:3000/api/system/health      # → 200 + 检查结果
  curl http://127.0.0.1:3000/api/system/processes    # → 200 + 进程列表
  
  # SSE 测试
  curl -N http://127.0.0.1:3000/api/sse/events
  # → 应收到心跳事件流

  # 验证批量写入（注入模拟日志行，等待 5s 后查询）
  sqlite3 data/aggregator.db "SELECT count(*) FROM request_logs;"
  ```

### Milestone 9: App 前端 — 6 个页面 + Onboarding
- **修改/创建文件**: `app/src/app/(dashboard)/**/*.tsx`, `app/src/components/**/*.tsx`
- **实现内容**:
  - 布局：`(dashboard)/layout.tsx` — shadcn Sidebar + 响应式外壳
  - Overview Dashboard (`page.tsx`) — 6 StatusCard + 趋势图 + 成功率 + 告警流
  - Accounts Detail (`accounts/page.tsx`) — DataTable + 操作按钮
  - Network (`network/page.tsx`) — IP 展示 + Leaflet 地图 + TLS 验证
  - Analytics (`analytics/page.tsx`) — Recharts 趋势图 + 饼图 + 直方图
  - Alerts (`alerts/page.tsx`) — 告警列表 + 规则 CRUD + 时间线
  - Settings (`settings/page.tsx`) — 配置表单 + 启停操作 + DB 维护
  - Onboarding Wizard (`setup/page.tsx`) — 8 步向导（仅首次显示）
  - 全局：深色模式 + next-themes + Framer Motion 过渡
- **验证方式**:
  - 浏览器打开 `http://127.0.0.1:3000`
  - 逐页检查 6 个页面是否正确渲染
  - 验证深色/浅色主题切换
  - 验证 SSE 实时更新（在 Network 页面触发 IP 验证）

### Milestone 10: 端到端集成测试
- **修改/创建文件**: `app/tests/**/*.test.ts`, 手动测试清单
- **实现内容**:
  - 全系统联调：Manager → OmniRoute → CPA×6 → Sing-box → OpenAI
  - 测试场景：
    1. 一键启动全部组件 → 健康检查全绿
    2. 通过 OmniRoute 发送 12 次请求 → 6 个 CPA 均有命中
    3. 手动停止 1 个 CPA → 自动恢复 → Dashboard 显示 crashed → restarting → running
    4. 等待 codex-auth-refresher 刷新 Token → Dashboard 显示刷新事件
    5. 一键停止全部组件 → 所有进程干净退出
  - API 单元测试（可选）：`vitest` 测试关键 API 端点
- **验证方式**:
  ```bash
  # 自动化冒烟测试脚本
  ./scripts/e2e_smoke_test.sh
  # → 应输出 10/10 测试通过
  ```

## 3. 终极执行顺序流 (Anti-Fragile Pipeline)

> **[执行约束核查完毕]** 交由 `plan-to-code` 按当前 execution protocol 落地

1. **[基础设施层]** 安装 Sing-box → 配置 6 节点 → 验证 IP 隔离 + TLS 指纹
2. **[代理实例层]** 创建 6 个 CPA 实例 → 登录 Plus 账号 → 验证隔离
3. **[Token 续期层]** 部署 codex-auth-refresher → 验证自动刷新 → 配置告警
4. **[聚合网关层]** 安装 OmniRoute → 注册 6 个 Provider → 创建 Combo
5. **[编排层]** 编写启停脚本 → 编写健康检查 → 端到端联调
6. **[防泄密层]** 生成 OmniRoute API Key → 配置 Codex App → 验证 Token 隐藏
7. **[App 骨架层]** 初始化 Next.js → DB Schema → shadcn 组件
8. **[App 后端层]** API Routes → 进程管理 → 健康检查 → SSE → 日志采集
9. **[App 前端层]** 6 页面 + Onboarding Wizard → 深色模式 → 动效
10. **[集成测试层]** 全系统联调 → 故障恢复 → 24h 稳定性验证

## 4. 验证期望 (Verification Expectations)

### 安全性
- ✅ Codex App 配置中无任何真实 Plus Token (仅 OmniRoute Key)
- ✅ 网络抓包无法获取 Plus Session Token
- ✅ OmniRoute / Manager 仅监听 `127.0.0.1`
- ✅ Manager Dashboard 需要 JWT 认证才能访问

### 可用性
- ✅ 通过 OmniRoute 发起对 GPT-5/5.1/5.2 的请求成功
- ✅ 负载均衡生效（6 个 CPA 均有命中）
- ✅ 单个 CPA 故障时自动 fallback 到其余 5 个
- ✅ Manager Dashboard 6 个页面均正常渲染

### 网络隔离
- ✅ 6 个 CPA 的出口 IP 彼此完全不同
- ✅ 无任何香港节点参与（已排除）
- ✅ TLS 指纹 (JA3/JA4) 与 Chrome 浏览器一致
- ✅ Network 页面地图正确标注 6 个 IP 地理位置

### 稳定性
- ✅ Token 在过期前 6h 自动刷新
- ✅ Circuit Breaker 在连续报错后正确熔断
- ✅ 持续运行 24h 无中断
- ✅ 进程崩溃后自动重启（≤ 3 次/小时）
- ✅ Manager Dashboard 实时展示全部 6 个 Token 和进程状态

### App 功能
- ✅ Onboarding Wizard 首次设置成功
- ✅ Overview Dashboard StatusCard 实时更新
- ✅ Analytics 页面 24h 趋势图正确渲染
- ✅ Alerts 页面告警规则 CRUD 正常
- ✅ Settings 页面一键启停全部组件

## 5. 当前容量与扩展说明

### 当前配置
- **6 个 Plus 账号**，绑定速云梯 6 个非香港独立服务器
- **月成本**：$120 (6 × $20)
- **冗余**：N+5 冗余，任意 5 个账号同时故障仍可继续工作

### 扩展到第 7~8 个账号

> **套餐升级说明（2026-03-23 核查）**：速云梯套餐升级后，非 HK **独立域名数量仍为 6 个**，未新增新的独立 IP 出口。升级带来的是每个域名下新增了 V4 系列节点（更多端口可选），可作为各域名的热备端口。若 V3 节点不稳定，直接在 `sing-box.json` 中换用同域名的 V4 端口即可，不改变 IP 隔离格局。

当前 6 个独立域名出口已全部分配给 6 个账号。扩展到第 7~8 个账号的方案：
1. 购买第二个机场订阅，获取新的**独立 IP 出口**节点
2. 或购买独立 VPS（推荐美国/日本/新加坡），自建代理
3. 扩展后重复 Milestone 1~4 添加新实例
4. Manager Onboarding Wizard 支持动态添加节点
