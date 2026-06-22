//! hook-layer: 函数指针注册表、hook 分发路由、hook 观察类型。
//!
//! 从 host-projection 提取的 Hook Layer，负责：
//! - 全部 49+ 个 OnceLock 函数指针 slot 的注册
//! - 跨宿主 hook 事件的路由分发
//! - Review gate / closeout 的 hook 端观察

pub mod hooks;
pub mod hook_dispatch;
