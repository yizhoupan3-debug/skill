---
allowed_tools:
- shell
- browser
approval_required_tools: []
description: '通用联网工具参考卡 — web_fetch、browser、文献调研工具速查。工具选择已由路由引擎自动处理。'
metadata:
  platforms:
  - supported
  tags:
  - web
  - browser
  - automation
  - screenshot
  - scraping
  - literature
  version: '1.2.0'
name: web-tools
network_access: required
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P2
risk: low
scene: general
session_start: n/a
short_description: 通用联网工具 — web_fetch 抓取、浏览器操作、文献调研速查卡
tags:
- web
- browser
- automation
- web-scraping
- screenshot
- literature
trigger_hints:
- 抓取
- 联网
- 爬虫
- web
- fetch
- browser
- automation
- screenshot
- 浏览器
- 截图
- 截屏
- 打开网页
- 点击
- 表单填写
- 输入
- 网络请求
- network
- 页面状态
- 页面元素
- 页面文本
- 浏览器会话
- 等待
- 自动填充
- 网页抓取
- web scrape
- 网络监听
- 网页操作
- 网页内容
- 获取网页
- 爬取
- 网络调研
- 查资料
- 查文献
- DOI
- 学术搜索
- 文献搜索
- 论文搜索
- 外部调研
- 外部项目调研
- 调研外部
when_to_use: 需要联网获取数据、打开网页、操作浏览器、抓取内容、截图、文献调研、DOI 验证时
do_not_use: 视觉审查截图内容（走 visual-review）；将截图转为 TikZ 配图（走 tikz-paper-figure）
---

# 通用联网工具

> **工具选择已由路由引擎自动处理。** 当前技能仅作为工具速查参考卡。如需手动指定工具，在查询中明确工具名称即可（如"用 web_fetch 抓"、"打开浏览器截图"）。

## Persona

Act as a **web connectivity specialist** with expertise in choosing the right tool for any network task: simple HTTP fetch, interactive browser automation, or academic literature research.

> 路由引擎（`web_task.rs`）会自动分析查询意图，选择最合适的工具。以下决策树为教育参考，非强制执行逻辑。

## 工具速查表

| 任务 | 工具 |
|------|------|
| 简单 HTTP GET 获取内容（无需 JS） | `web_fetch` |
| 学术论文搜索 | `search_research` / `research_literature_search` |
| 按标题/DOI 查论文详情 | `find_paper_by_title` / `fetch_paper` |
| 打开 URL（需 JS 渲染） | `browser_open` |
| 点击元素 | `browser_click` |
| 填表单 | `browser_fill` |
| 按键 | `browser_press` |
| 截图 | `browser_screenshot` |
| 获取页面文本 | `browser_get_text` |
| 获取页面状态/元素 | `browser_get_state` / `browser_get_elements` |
| 检查网络请求 | `browser_get_network` |
| 等待条件 | `browser_wait_for` |
| 管理标签页 | `browser_tabs` / `browser_close` |
| 保存/恢复会话 | `browser_save_session` / `browser_restore_session` |
| 诊断 | `browser_diagnostics` |

## Do not use

- 视觉审查截图内容（已有截图后需要分析图片 → 用 `visual-review`）
- 将截图转为 TikZ 配图 → 用 `tikz-paper-figure`
- 只需要自然语言搜索结果（用 `WebSearch` 内置搜索）
- 需要深度多源事实核查报告（用 `deep-search`）
