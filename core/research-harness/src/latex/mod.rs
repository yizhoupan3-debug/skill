//! LaTeX 数学公式解析与渲染模块。
//!
//! 基于 [RaTeX](https://github.com/erweixin/RaTeX) 的 LaTeX 数学公式引擎，
//! 提供完整的 parse → layout → SVG 渲染管线。
//!
//! ## 与现有 citation 模块的关系
//!
//! 本模块**不替代**现有的 citation/audit.rs、citation/render.rs、citation/doi.rs。
//! 现有模块处理 BibTeX 文献管理和引用审计，本模块处理数学公式解析和渲染。
//! 两者覆盖完全不同的领域，互不干扰。
//!
//! ## 能力范围
//!
//! - LaTeX 数学公式 AST 解析（54 种节点类型）
//! - 30+ 数学环境支持（equation/align/cases/matrix 等）
//! - 宏展开（\def/\newcommand/\DeclareMathOperator）
//! - 化学方程式（\ce{}）
//! - SVG 矢量渲染

// ── RaTeX Types (from ratex-types crate) ──
pub mod color;
pub mod display_item;
pub mod math_style;
pub mod path_command;
pub mod unicode_scripts;

// ── RaTeX Parser (from ratex-parser crate) ──
pub mod environments;
pub mod error;
pub mod functions;
pub mod macro_expander;
pub mod mhchem;
pub mod parse_node;
pub mod parser;
pub mod unicode_sup_sub;

// ── Our SVG render module ──
pub mod render;

// ── Re-exports ──
pub use color::Color;
pub use display_item::{DisplayItem, DisplayList};
pub use error::{ParseError, ParseResult};
pub use math_style::MathStyle;
pub use parse_node::{Mode, ParseNode};
pub use parser::{parse, Parser};
pub use path_command::PathCommand;
pub use unicode_scripts::{script_from_codepoint, supported_codepoint, UnicodeScript};

/// 模块版本
pub const MODULE_VERSION: &str = "0.1.0";

/// 模块描述
pub const DESCRIPTION: &str = "LaTeX math formula parser and SVG renderer (based on RaTeX)";
