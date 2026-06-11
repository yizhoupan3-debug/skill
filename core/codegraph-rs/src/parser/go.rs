use super::{ParsedEdge, ParsedSymbol};
use tree_sitter::Node;

pub(crate) struct ParseOutput {
    pub symbols: Vec<ParsedSymbol>,
    pub edges: Vec<ParsedEdge>,
}

pub(crate) fn parse(source: &str) -> ParseOutput {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .ok();
    let Some(tree) = parser.parse(source, None) else {
        return ParseOutput {
            symbols: Vec::new(),
            edges: Vec::new(),
        };
    };
    let root = tree.root_node();
    let bytes = source.as_bytes();
    let mut symbols = Vec::new();
    let mut edges = Vec::new();
    // Single-pass: collect both symbols and edges in one AST traversal
    collect_all(root, bytes, &mut symbols, &mut edges);
    ParseOutput { symbols, edges }
}

fn collect_all(
    node: Node<'_>,
    source: &[u8],
    symbols: &mut Vec<ParsedSymbol>,
    edges: &mut Vec<ParsedEdge>,
) {
    // Collect symbols at this node
    match node.kind() {
        "function_declaration" | "method_declaration" | "type_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                if let Ok(text) = name.utf8_text(source) {
                    symbols.push(ParsedSymbol {
                        symbol: text.to_string(),
                        kind: node.kind().trim_end_matches("_declaration").to_string(),
                        line: node.start_position().row as u32 + 1,
                    });
                }
            }
        }
        _ => {}
    }
    // Collect call edges at this node
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if let (Some(caller), Some(callee)) =
                (enclosing_symbol(node, source), callee_name(func, source))
            {
                edges.push(ParsedEdge {
                    caller_symbol: caller,
                    callee_symbol: callee,
                    line: node.start_position().row as u32 + 1,
                });
            }
        }
    }
    // Recurse into children
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            collect_all(child, source, symbols, edges);
        }
    }
}

fn callee_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => node.utf8_text(source).ok().map(|s| s.to_string()),
        "selector_expression" => node
            .child_by_field_name("field")
            .and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string())),
        _ => None,
    }
}

/// Walk up AST to find nearest enclosing function/method declaration.
fn enclosing_symbol(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        match ancestor.kind() {
            "function_declaration" | "method_declaration" => {
                if let Some(name) = ancestor.child_by_field_name("name") {
                    if let Ok(text) = name.utf8_text(source) {
                        return Some(text.to_string());
                    }
                }
            }
            _ => {}
        }
        current = ancestor.parent();
    }
    None
}
