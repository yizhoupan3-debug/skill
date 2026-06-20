use std::collections::HashMap;

use crate::latex::error::ParseResult;
use crate::latex::functions::{define_function_full, FunctionContext, FunctionSpec};
use crate::latex::parse_node::ParseNode;

pub fn register(map: &mut HashMap<&'static str, FunctionSpec>) {
    define_function_full(
        map,
        &["\\relax"],
        "internal",
        0,
        0,
        None,
        true, // allowed_in_argument
        true, // allowed_in_text
        true,
        false,
        false,
        handle_relax,
    );
}

fn handle_relax(
    ctx: &mut FunctionContext,
    _args: Vec<ParseNode>,
    _opt_args: Vec<Option<ParseNode>>,
) -> ParseResult<ParseNode> {
    Ok(ParseNode::Internal {
        mode: ctx.parser.mode,
        loc: None,
    })
}
