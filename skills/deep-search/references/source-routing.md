# Source Routing — Agent Reach 多平台检索参考

> 本文档是 `deep-search` skill 的 Phase 2/3 配套参考。
> 提供按主题类型选择数据源、命令模板和降级策略。

## 能力检测

```bash
# 检查 Agent Reach 可用渠道
agent-reach doctor --json

# Exa AI 搜索（若已通过 mcporter 配置）
mcporter config list 2>/dev/null
```

## 主题 → 数据源映射

### 1. 技术/代码调研

| 源 | 命令 | 适用场景 |
|----|------|---------|
| Exa AI 搜索 | `mcporter call 'exa.web_search_exa(query: "Rust async 框架对比 2025", numResults: 5)'` | 技术文章、博客、新兴框架 |
| GitHub 搜索 | `gh search repos "ml-inference rust" --sort stars --limit 10` | 开源项目发现 |
| GitHub 代码 | `gh search code "tokio spawn" --limit 5` | 代码模式搜索 |
| V2EX | `curl -s "https://www.v2ex.com/api/topics/hot.json" -H "User-Agent: agent-reach/1.0"` | 中文技术社区讨论 |
| 降级 | `WebSearch` + `web_fetch` | — |

### 2. 产品评测/口碑

| 源 | 命令 | 适用场景 |
|----|------|---------|
| 小红书 | `opencli xiaohongshu search "AirPods Pro 2 评测" -f yaml`（桌面） | 中文真实用户口碑 |
| B站 | `bili search "AI 编程工具" --type video -n 5` | 视频评测 |
| V2EX | `curl -s "https://www.v2ex.com/api/topics/hot.json" -H "User-Agent: agent-reach/1.0"` | 技术产品讨论 |
| 降级 | `WebSearch` + `web_fetch` | — |

### 3. 学术/论文调研

| 源 | 命令 | 适用场景 |
|----|------|---------|
| Semantic Scholar | `search_research(query: "transformer attention mechanism", limit: 5)` | 论文发现与引用关系 |
| arXiv | `research_literature_search(query: "LLM reasoning", source: "arxiv", limit: 10)` | 最新预印本 |
| Exa | `mcporter call 'exa.web_search_exa(query: "deep learning survey 2025", numResults: 5)'` | 博客/技术报告 |
| 降级 | `WebSearch` → `web_fetch` | — |

### 4. 全球热点

| 源 | 命令 | 适用场景 |
|----|------|---------|
| Twitter/X | `twitter search "AI regulation" -n 20` | 实时讨论与专家观点 |
| Reddit | `rdt search "LLM benchmarks" --limit 10` | 社区深度讨论 |
| YouTube | `yt-dlp --write-sub --skip-download -o "/tmp/%(id)s" "URL"` | 技术演讲/教程 |
| 降级 | `WebSearch` + `web_fetch` | — |

### 5. 国内话题

| 源 | 命令 | 适用场景 |
|----|------|---------|
| B站 | `bili search "大模型" --type video -n 10` | 中文视频内容 |
| V2EX | `curl -s "https://www.v2ex.com/api/topics/latest.json" -H "User-Agent: agent-reach/1.0"` | 中文技术社区 |
| 雪球 | `opencli xueqiu search "新能源" -f yaml` | 财经/股票讨论 |
| 降级 | `WebSearch` + `web_fetch` | — |

### 6. 视频内容

| 源 | 命令 | 适用场景 |
|----|------|---------|
| YouTube | `yt-dlp --write-sub --skip-download -o "/tmp/%(id)s" "URL"` | 技术教程、演讲字幕 |
| B站 | `bili search "教程" --type video -n 5` | 中文视频搜索 |
| 小宇宙 | `opencli xiaoyuzhou search "AI" -f yaml` | 中文播客内容 |
| 降级 | `WebSearch` | — |

### 7. 通用网页

| 源 | 命令 | 适用场景 |
|----|------|---------|
| Jina Reader | `curl -s "https://r.jina.ai/URL"` | 任意网页 → Markdown |
| web_fetch | `web_fetch(url: "URL")` | 降级方案 |
| RSS | `feedparser.parse("feed_url")` | 订阅源定期阅读 |

## 多源并行模式

对于宽泛研究话题，可同时运行 2-3 个源：

```bash
# 技术话题：Exa + GitHub + V2EX
mcporter call 'exa.web_search_exa(query: "..." numResults: 5)' &
gh search repos "..." --sort stars --limit 5 &
curl -s "https://www.v2ex.com/api/topics/hot.json" &
wait
```

## 部分可用（Partial Availability）降级

Agent Reach 的渠道可能部分可用（如 Exa 配好了但小红书没配）。按以下策略处理：

| 检测结果 | 行为 |
|---------|------|
| `agent-reach doctor --json` 返回成功 | 按各渠道 `active_backend` 字段选择可用源 |
| 某渠道 `status == "ok"` | 正常使用该源 |
| 某渠道 `status == "warn"` | 使用但有已知限制（如缺 JS runtime），记录在 recovery trace |
| 某渠道 `status == "off"` | 跳过该源，使用备选源 |
| 某渠道 `status == "error"` | 跳过该源，记录错误 |
| `mcporter` 存在但 Exa 未配置 | 退回到 `WebSearch` |
| `gh` CLI 未安装 | 跳过 GitHub 搜索，退回到 `WebSearch` |
| `agent-reach doctor` 命令不存在 | Agent Reach 整体不可用 → 全部降级到 `WebSearch` + `web_fetch` |
| 部分渠道可用 + 部分不可用 | 用可用的，不可用的记录在 recovery trace 中 |

## 失败处理

| 失败原因 | 处理方式 |
|---------|---------|
| mcporter 不可用 | 降级到 `WebSearch` |
| Jina Reader 超时 | 降级到 `web_fetch` |
| Agent Reach 未安装 | 全部降级到 `WebSearch` + `web_fetch` |
| 特定平台 (Twitter/Reddit) 未配置 | 跳过该源，记录在 recovery trace 中 |
| yt-dlp 缺少 JS runtime | 仅使用已有的字幕文件，不转写 |
| curl 超时 | 3 次重试，每次 30s，失败后标记为 unreachable |
