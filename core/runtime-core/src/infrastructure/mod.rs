//! 基础设施层 (Infrastructure Layer)
//!
//! Transport 层、配置加载、遥测、存储等支撑功能。
//!
//! ## 模块
//! - `stdio_transport`: MCP stdio 传输协议
//! - `kernel_bootstrap`: 内核引导
//! - `framework_skills`: 技能发现
//! - `telemetry_emit`: 遥测事件发射
//! - `router_env_flags`: 环境变量标志
//! - `router_rs_obs`: 路由器观察
//! - `session_call`: 会话调用跟踪
pub mod stdio_transport;
pub mod kernel_bootstrap;
pub mod framework_skills;
pub mod telemetry_emit;
pub mod router_env_flags;
pub mod router_rs_obs;
pub mod session_call;
