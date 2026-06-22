//! 编排层 (Orchestration Layer)
//!
//! 多 Agent 编排、论文调用的 hook 处理、研究活动日志。
//!
//! ## 模块
//! - `paper_adversarial`: 论文对抗性验证 hook
//! - `paper_prose`: 论文 prose 上下文注入 hook
//! - `research`: 研究活动日志记录
pub mod paper_adversarial;
pub mod paper_prose;
pub mod research;
