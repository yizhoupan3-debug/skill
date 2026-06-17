---
module: framework-kernel
lines: ~1400
layer: B0
last_verified: "2026-06-16"
---

# framework-kernel（B0 层）

框架基础设施层，无业务逻辑依赖，被所有上层 crate 依赖。

## 职责

提供框架运行时的底层基础设施：宿主目标解析、注册表加载、遥测、分词、profile 管理。

## 核心功能

| 模块 | 行数 | 功能 | 关键 API |
|------|------|------|----------|
| `framework_host_targets` | 385 | 宿主目标解析 | `installable_hosts()`、`skills_install_tool()` |
| `framework_profile` | 1,692 | 生命周期配置与 host profile | `FrameworkProfile`、`LifecycleConfig` |
| `runtime_registry` | 279 | `RUNTIME_REGISTRY.json` 加载与校验 | `load_runtime_registry()`、`HOST_ADAPTER_CONTRACT_PATH` |
| `router_self` | 602 | router-rs 自身安装/验证/路径解析 | `router_self_binary_path()` |
| `telemetry` | 320 | 遥测管道 | `emit_telemetry()`、`LogAggregator` |
| `tokenizer` | 84 | 分词提供者 trait | `TokenizerProvider`、`install_tokenizer_provider()` |
| `repo_roots` | 201 | 框架根路径检测 | `framework_root_from_executable_path()` |
| `stdio_payload_types` | 309 | stdio 协议载荷类型 | `StdioRequest`、`StdioResponse` |
| `skill_repo` | 34 | skill 仓库发现 | `discover_skill_policy_repo_root()` |

## 依赖关系

- **依赖**: `serde`、`serde_json`、`sha2`（workspace）
- **被依赖**: `runtime-core`、`host-projection`、`core-policy`

## 近期变更

- v6.5: `framework_host_targets` 从 `runtime-core` 迁移至此，通过 `pub use` 保持兼容
- v6.5: `HOST_ADAPTER_CONTRACT_PATH` 指向更新为 `docs/spec.md`

## 已知技术债

- `framework_profile`（1,692 行）是 B0 层最大模块，可考虑拆分
- `HOST_ADAPTER_CONTRACT_PATH` 常量用于运行时检查文档存在性，但指向的文档内容已变
