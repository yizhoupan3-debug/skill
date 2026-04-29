---
name: linux-server-ops
description: |
  Get services running and staying healthy on a Linux host — systemd units,
  reverse proxies (Nginx/Caddy), log inspection, and port/process diagnosis.
  Delivers restart-safe service configurations, not ad-hoc terminal sessions.
  Use when the user asks about Linux deployment, server logs, reverse proxy
  setup, process management, bad gateway, port conflicts, or phrases like
  "配 systemd", "看服务器日志", "Nginx 反代", "服务起不来".
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - linux
    - server
    - systemd
    - nginx
    - ops
risk: medium
source: local
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 配 systemd
  - 看服务器日志
  - Nginx 反代
  - 服务起不来
  - server logs
  - reverse proxy setup
  - process management
  - bad gateway
  - port conflicts
  - linux

---

# linux-server-ops

This skill owns Linux service runtime and small-to-medium server operations when the problem is deployment behavior, process management, reverse proxies, or service health on a host.

## When to use

- The user wants to deploy, run, restart, or debug an app on a Linux server or VM
- The task involves `systemd`, service units, Nginx/Caddy reverse proxies, logs, ports, environment files, or process/runtime supervision
- The task involves PM2-style Node service runtime management, restart behavior, health checks, or host-level troubleshooting
- The user wants to inspect why a service will not start, stay healthy, or serve traffic correctly
- Best for requests like:
  - "帮我配 systemd service"
  - "这个服务在 Linux 上为什么起不来"
  - "Nginx 反代怎么配"
  - "看服务器日志和端口占用"

## Do not use

- The main task is Docker/container authoring rather than host-level service ops → use `$docker`
- The main task is Cloudflare-specific deployment rather than Linux host runtime → use `$cloudflare-deploy`
- The task is generic shell scripting rather than server runtime operations → use `$shell-cli`
- The task is Kubernetes or large-cluster orchestration beyond host-level ops

## Task ownership and boundaries

This skill owns:
- Linux host service runtime setup and debugging
- systemd unit design and service lifecycle
- reverse proxy setup for app traffic
- log, port, process, and environment troubleshooting on the host
- host-level health/restart and runtime hardening basics
- **Dual-Dimension Audit (Pre: Systemd/Config, Post: Restart-Health/Port Results)** → runtime verification gate

This skill does not own:
- container image authoring
- provider-specific platform deployment details by default
- generic shell-only automation with no server-ops context
- large-scale orchestration platforms

If the task shifts to adjacent skill territory, route to:
- `$docker`
- `$cloudflare-deploy`
- `$shell-cli`
- `$github-actions-authoring`

## Required workflow

1. Confirm the task shape:
   - object: service, unit file, reverse proxy, port/process, host logs
   - action: deploy, configure, debug, restart, harden, inspect
   - constraints: distro, runtime, init system, network topology, TLS, privileges
   - deliverable: config, fix, runbook, or diagnosis
2. Check the actual runtime path: process, logs, ports, unit state, and proxy path.
3. Prefer explicit, restart-safe service configuration over manual shell startup.
4. Keep environment, ports, file permissions, and proxy mappings explicit.
5. Validate that traffic reaches the intended process and the process stays healthy.

## Core workflow

### 1. Intake
- Determine distro, init system, app runtime, and whether a reverse proxy is involved.
- Identify symptoms: fails to start, crashes, bad gateway, wrong env vars, wrong ports, or permission issues.
- Inspect existing service config and runtime assumptions before rewriting them.

### 2. Execution
- Configure `systemd` or the chosen supervisor with explicit working dir, user, env, restart policy, and logs.
- Keep reverse proxy config narrow and aligned with actual app port and headers.
- Verify firewall, listening address, file permissions, and runtime dependencies.
- Prefer repeatable service config over ad hoc terminal sessions.

### 3. Validation / recheck
- Re-check service status, logs, listening ports, and reverse proxy behavior.
- Confirm health after restart/reload, not just after one manual run.
- If privileged/network/TLS details are unknown, state the remaining uncertainty clearly.

## Output defaults

Default output should contain:
- host/service context
- config or diagnosis
- validation steps and remaining host risks

Recommended structure:

````markdown
## Server Ops Summary
- Host/runtime: ...
- Symptom: ...

## Config / Diagnosis
- Service: ...
- Proxy / ports / env: ...

## Validation / Remaining Risk
- Checked: ...
- Still verify: ...
````

## Hard constraints

- Do not present host-level service setup as complete without checking logs and unit/process state.
- Do not rely on manual shell sessions when a managed service definition is appropriate.
- Do not leave ports, bind address, or reverse-proxy targets implicit.
- Do not assume Docker/Kubernetes when the task is plainly host-level Linux ops.
- If the advice depends on distro/init specifics, say so explicitly.
- **Superior Quality Audit**: For server uptime critical nodes, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples

- "Use $linux-server-ops to configure systemd and Nginx for this app."
- "帮我看 Linux 服务器上这个服务为什么起不来。"
- "给我一套可重复的服务部署/反向代理配置。"
- "强制进行 Linux 运维深度审计 / 检查服务重启后的健康状态与端口结果。"
- "Use the runtime verification gate to audit this Linux server setup for restart-safe idealism."
