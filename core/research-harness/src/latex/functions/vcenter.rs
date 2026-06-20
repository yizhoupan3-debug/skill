use std::collections::HashMap;

use crate::latex::error::ParseResult;
use crate::latex::functions::{define_function_full, ArgType, FunctionContext, FunctionSpec};
use crate::latex::parse_node::ParseNode;

pub fn register(map: &mut HashMap<&'static str, FunctionSpec>) {
    define_function_full(
        map,
        &["\\vcenter"],
        "vcenter",
        1,
        0,
        Some(vec![ArgType::Original]),
        false,
        false,
        true,
        false,
        false,
        handle_vcenter,
    );
}

fn handle_vcenter(
    ctx: &mut FunctionContext,
    args: Vec<ParseNode>,
    _opt_args: Vec<Option<ParseNode>>,
) -> ParseResult<ParseNode> {
    Ok(ParseNode::VCenter {
        mode: ctx.parser.mode,
        body: Box::new(args.into_iter().next().unwrap()),
        loc: None,
    })
}
