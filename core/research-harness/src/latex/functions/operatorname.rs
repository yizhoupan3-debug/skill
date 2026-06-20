use std::collections::HashMap;

use crate::latex::error::ParseResult;
use crate::latex::functions::{define_function_full, FunctionContext, FunctionSpec};
use crate::latex::parse_node::ParseNode;

pub fn register(map: &mut HashMap<&'static str, FunctionSpec>) {
    define_function_full(
        map,
        &["\\operatorname@", "\\operatornamewithlimits"],
        "operatorname",
        1,
        0,
        None,
        false,
        false,
        true,
        false,
        false,
        handle_operatorname,
    );
}

fn handle_operatorname(
    ctx: &mut FunctionContext,
    args: Vec<ParseNode>,
    _opt_args: Vec<Option<ParseNode>>,
) -> ParseResult<ParseNode> {
    let body = ParseNode::ord_argument(args.into_iter().next().unwrap());

    let always_handle_sup_sub = ctx.func_name == "\\operatornamewithlimits";

    Ok(ParseNode::OperatorName {
        mode: ctx.parser.mode,
        body,
        always_handle_sup_sub,
        limits: false,
        parent_is_sup_sub: false,
        loc: None,
    })
}
