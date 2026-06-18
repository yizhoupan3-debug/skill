---
last_verified: "2026-06-19"
depends_on:
  - ../spec.md
  - ../spec-core-crates.md
---

# ADR-004: 错误处理策略 — `Result<T, String>` vs `thiserror`

## Status

Accepted (2026-06-14).

## Context

核心 crate（`core-state`, `framework-kernel`, `core-policy`, `runtime-core`, `host-projection`, `router-rs`）统一使用 `Result<T, String>` 作为错误类型。工具 crate（`codegraph-rs`, `autoresearch-rs`, `evolution-rs`）使用 `anyhow::Error`。无 `thiserror` 派生错误枚举。

## Decision

1. **核心 crate 保持 `Result<T, String>`**：v7 不引入 `thiserror`。当前模式的简洁性对内部项目有利，且实践中未出现错误类型可匹配性不足的问题。
2. **公共 API 签名不变**：`Result<T, String>` 在核心 crate 间传递时足够表达 IO/Config/Registry/Hook/MCP/Session 六大类错误（错误消息前缀标识类别）。
3. **工具 crate 保持 `anyhow`**：工具 crate 对框架错误处理的耦合度低，`anyhow` 的 context 链对工具场景有利。
4. **评估时机**：如在 v7.1+ 发现频繁出现 match-on-error-string 模式，再引入 `thiserror`。

## Consequences

- **优势**：零迁移成本，保持 v6.5 基线的一致性。
- **代价**：错误处理缺乏类型安全，调用方无法通过类型区分错误类别。
- **风险**：若未来引入 thiserror，跨 crate 签名变更是 breaking change。

## Related

- `artifacts/current/roadmap-v7.md` §11.2 — thiserror 评估
- `docs/spec/core-crates.md` — crate 职责与依赖
