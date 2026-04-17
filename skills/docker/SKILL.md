---
name: docker
description: |
  Produce minimal, secure Docker images with correct layer caching,
  multi-stage builds, health checks, and Compose orchestration. Delivers
  Dockerfiles that build reproducibly, run unprivileged, and stay small.
  Use when the user asks about Docker, containers, Dockerfiles, Compose, image
  optimization, or phrases like "容器化", "镜像构建", "Docker 部署",
  "多阶段构建", "镜像太大了".
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  model: inherit
risk: medium
source: community
---
# docker

This skill owns Docker containerization: image building, Compose orchestration, container debugging, security hardening, and CI/CD integration patterns.
- **Dual-Dimension Audit (Pre: Layers/Config, Post: Build-Size/Runtime Results)** → `$execution-audit-codex` [Overlay]

## When to use

- The user wants to create, debug, or optimize Dockerfiles or docker-compose setups
- The task involves container builds, multi-stage builds, image optimization, or Docker networking
- The user says "Docker", "容器化", "docker-compose", "Dockerfile", "镜像优化"
- The user wants to containerize an application or fix container-related issues

## Do not use

- The task is Kubernetes cluster management → use `$linux-server-ops`
- The task is CI/CD pipeline configuration using Docker → use `$github-actions-authoring`
- The task is Cloudflare Workers deployment → use `$cloudflare-deploy`

## Hard Constraints
- **Superior Quality Audit**: For production images, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).
- Do not run containers as root.
- Use multi-stage builds for minimal production footprint.

## Trigger examples
- "强制进行 Docker 深度审计 / 检查镜像构建结果与运行时隔离状态。"
- "Use $execution-audit-codex to audit this Dockerfile for layer-caching idealism."
