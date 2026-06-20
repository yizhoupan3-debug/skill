use std::collections::HashMap;

use crate::latex::error::ParseResult;
use crate::latex::functions::{define_function_full, FunctionContext, FunctionSpec};
use crate::latex::parse_node::ParseNode;

static SIZE_FUNCS: &[&str] = &[
    "\\tiny",
    "\\sixptsize",
    "\\scriptsize",
    "\\footnotesize",
    "\\small",
    "\\normalsize",
    "\\large",
    "\\Large",
    "\\LARGE",
    "\\huge",
    "\\Huge",
];

pub fn register(map: &mut HashMap<&'static str, FunctionSpec>) {
    define_function_full(
        map,
        SIZE_FUNCS,
        "sizing",
        0,
        0,
        None,
        false,
        true, // allowed_in_text
        true,
        false,
        false,
        handle_sizing,
    );
}

fn handle_sizing(
    ctx: &mut FunctionContext,
    _args: Vec<ParseNode>,
    _opt_args: Vec<Option<ParseNode>>,
) -> ParseResult<ParseNode> {
    let break_on = ctx.break_on_token_text.clone();
    let body = ctx.parser.parse_expression(false, break_on.as_deref())?;

    let size = SIZE_FUNCS
        .iter()
        .position(|&f| f == ctx.func_name)
        .map(|i| i + 1)
        .unwrap_or(6) as u8;

    Ok(ParseNode::Sizing {
        mode: ctx.parser.mode,
        size,
        body,
        loc: None,
    })
}
