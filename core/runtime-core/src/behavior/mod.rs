//! 行为层 (Behavior Layer)
//!
//! 上下文工程、loop engine、goal 驱动。管理 Quality Gate 多轮闭环、
//! 目标状态跟踪与验证聚合。
//!
//! ## 模块
//! - `quality_gate`: RFV 多轮闭环引擎，支持 start/upsert/append_round
pub mod quality_gate;
