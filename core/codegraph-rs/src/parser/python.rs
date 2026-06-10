use super::{ParsedEdge, ParsedSymbol};
use tree_sitter::Node;

pub(crate) struct ParseOutput {
    pub symbols: Vec<ParsedSymbol>,
    pub edges: Vec<ParsedEdge>,
}

pub(crate) fn parse(source: &str) -> ParseOutput {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
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
    collect_symbols(root, bytes, &mut symbols);
    let mut edges = Vec::new();
    collect_calls(root, bytes, &symbols, &mut edges);
    ParseOutput { symbols, edges }
}

fn collect_symbols(node: Node<'_>, source: &[u8], out: &mut Vec<ParsedSymbol>) {
    match node.kind() {
        "function_definition" | "class_definition" => {
            if let Some(name) = node.child_by_field_name("name") {
                if let Ok(text) = name.utf8_text(source) {
                    out.push(ParsedSymbol {
                        symbol: text.to_string(),
                        kind: node.kind().trim_end_matches("_definition").to_string(),
                        line: node.start_position().row as u32 + 1,
                    });
                }
            }
        }
        _ => {}
    }
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            collect_symbols(child, source, out);
        }
    }
}

fn collect_calls(
    node: Node<'_>,
    source: &[u8],
    _symbols: &[ParsedSymbol],
    edges: &mut Vec<ParsedEdge>,
) {
    if node.kind() == "call" {
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
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            collect_calls(child, source, _symbols, edges);
        }
    }
}

fn callee_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "attribute" => {
            if node.kind() == "attribute" {
                node.child_by_field_name("attribute")
                    .and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string()))
            } else {
                node.utf8_text(source).ok().map(|s| s.to_string())
            }
        }
        _ => None,
    }
}

/// Walk up the AST to find the nearest enclosing function/class definition.
fn enclosing_symbol(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        match ancestor.kind() {
            "function_definition" | "class_definition" => {
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
