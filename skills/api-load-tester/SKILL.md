---
name: api-load-tester
description: |
  Design and run API load, stress, soak, and spike tests with k6, wrk, or
  autocannon. Use when benchmarking endpoints, estimating RPS and throughput,
  validating p95 and p99 latency targets, finding bottlenecks, testing
  authenticated flows, or comparing performance before and after changes.
  适用于“压测 API”“load test”“stress test”“soak test”“spike test”“RPS”
  “并发”“延迟阈值”“找瓶颈”“k6 / wrk / autocannon”这类请求。
risk: medium
source: community-adapted
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 压测 API
  - load test
  - stress test
  - soak test
  - spike test
  - RPS
  - 并发
  - 延迟阈值
  - 找瓶颈
  - k6
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - api
    - load
    - tester
---

- **Dual-Dimension Audit (Pre: Load-Profile/Logic, Post: Latency-P99/Throughput Results)** → `$execution-audit` [Overlay]
# api-load-tester

## Overview

这个 skill 用来把“接口能用”升级成“接口能承压、能量化验证”。

核心目标：
- 设计合理压测场景
- 生成可运行脚本
- 记录阈值和破坏点
- 输出可解释的性能结论

## When to use

- 用户要求压测 API
- 用户要求 benchmark、stress test、soak test、spike test
- 用户要求测 RPS、延迟、错误率、并发能力
- 用户要求验证登录后流程、注册流程、多步接口链路

## Do not use

- 前端渲染性能问题
- 单纯做函数微基准，不是 API 压测
- 未经确认直接对生产环境压测

## Safety First

**默认不要压生产。**

开始前必须明确：
- 目标环境
- 是否允许压测
- 是否有速率限制或防火墙
- 是否会影响真实用户

## Workflow

### 1. Choose the Right Tool

- `k6`
  适合多步骤流程、阈值、认证、复杂场景

- `wrk`
  适合简单单端点高吞吐基准

- `autocannon`
  适合 Node.js 环境下快速测 HTTP 接口

默认优先 `k6`。

### 2. Gather API Facts

从以下来源收集：
- OpenAPI / Swagger
- route 定义
- handler 代码
- 用户提供的接口描述

重点拿到：
- endpoint
- method
- auth 方式
- request / response shape
- 依赖资源

### 3. Pick a Scenario

常见测试类型：
- ramp-up：找破坏点
- soak：看长期稳定性
- spike：看突发流量恢复能力
- steady benchmark：对比版本前后变化

### 4. Add Realistic Traffic

不要只发假数据。

应尽量模拟：
- 真实 payload 分布
- 登录态或 token
- 多类请求混合
- think time
- 合理 headers

### 5. Add Thresholds

至少定义：
- p95 latency
- p99 latency
- error rate
- 最低成功率

没有阈值，压测结果就很难判断成败。

### 6. Report Clearly

报告至少应包含：
- 环境
- 并发或 VUs
- 持续时间
- p50 / p95 / p99
- 错误率
- 实际吞吐
- 首次超阈值点
- 瓶颈假设

## Good Defaults

- 从低压开始，再逐步升高
- 对多步业务流加入认证和状态依赖
- 把限流命中单独标注，不要误判成系统崩溃
- 记录数据库规模、实例数等背景信息

## Codex Notes

- 若用户要求直接实施，优先生成最小可运行脚本
- 如果依赖登录态或 token，明确获取流程
- 如果测试工具本机未安装，先说明依赖
- 结果总结要用工程语言，不只贴原始数字

## Quick Checklist

- 目标环境是否确认
- 工具是否选对
- 流量模型是否真实
- 阈值是否定义
- 是否区分限流与故障
- **Superior Quality Audit**: For production-scale load tests, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).
- 是否输出了明确结论

## Trigger examples
- "强制进行压测深度审计 / 检查负载曲线与 P99 延迟结果。"
- "Use $execution-audit to audit this load test for p99-latency idealism."

