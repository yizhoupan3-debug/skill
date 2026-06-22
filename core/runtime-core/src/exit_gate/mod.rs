//! 退出门 (Exit Gate Layer)
//!
//! Quality Gate 与 Closeout 检查点。管理操作者 nudges、
//! schema drift 检测与退出条件评估。
//!
//! ## 模块
//! - `harness_ops`: Harness 操作者 nudges 系统
//! - `quality_gate`: Quality Gate 多轮闭环引擎
//! - `schema_drift`: Schema 漂移检测
pub mod harness_ops;
pub mod quality_gate;
pub mod schema_drift;
