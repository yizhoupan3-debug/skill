---
last_verified: "2026-06-02"
depends_on:
  - components.md
  - host-integration.md
---

# 数据流

本文档覆盖框架的四条核心数据流：用户请求全链路、skill 路由、goal drive、证据采集。

## 1. 一次完整的用户请求

```
用户输入 -> 宿主捕获 -> shell launcher -> router-rs hook
  -> SessionStart: 注入轻量 Repo: 行
  -> UserPromptSubmit: session key 检查、pre-goal nudge
  -> [agent 执行，调用工具]
  -> PostToolUse: 证据采集到 EVIDENCE_INDEX
  -> [agent 继续执行]
  -> Stop: review gate 检查 -> closeout 检查 -> SESSION_CLOSE_STYLE 提示
```

## 2. Skill 路由流

```
用户意图
  -> router-rs route::routing 路由引擎
  -> 读取 SKILL_ROUTING_RUNTIME.json
  -> 匹配 trigger_hints + scoring
  -> 返回 skill_path
  -> 宿主读取对应 SKILL.md
```

## 3. Goal drive 流

```
用户调用 /implementx
  -> implementx SKILL.md 读取 WAVE_STATE.json
  -> 逐 wave 执行
  -> 每个 wave 产出写入 artifacts/current/<task_id>/
  -> 验证后 /verifyx 清理
```

## 4. 证据流

```
L1 验证命令输出
  -> router-rs PostToolUse 采样/追加
  -> artifacts/current/<task_id>/EVIDENCE_INDEX.json
  -> closeout / review gate 消费
```
