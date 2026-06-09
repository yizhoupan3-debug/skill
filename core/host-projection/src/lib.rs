//! host-projection: 宿主集成层（hosts, host_integration, host_entrypoint_sync, framework_host_targets）。
//!
//! 从 router-rs 抽取，临时依赖 router-rs 获取共享模块。

pub mod hosts;
pub mod host_integration;
pub mod host_entrypoint_sync;
pub mod framework_host_targets;
