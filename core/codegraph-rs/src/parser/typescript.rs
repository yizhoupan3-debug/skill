use super::{ParsedEdge, ParsedSymbol};
use tree_sitter::Node;

pub(crate) struct ParseOutput {
    pub symbols: Vec<ParsedSymbol>,
    pub edges: Vec<ParsedEdge>,
}

pub(crate) fn parse(source: &str, tsx: bool) -> ParseOutput {
    let mut parser = tree_sitter::Parser::new();
    let language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    };
    parser.set_language(&language.into()).ok();
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
        "function_declaration"
        | "method_definition"
        | "class_declaration"
        | "interface_declaration"
        | "type_alias_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                if let Ok(text) = name.utf8_text(source) {
                    out.push(ParsedSymbol {
                        symbol: text.to_string(),
                        kind: node.kind().replace("_declaration", "").replace("_definition", ""),
                        line: node.start_position().row as u32 + 1,
                    });
                }
            }
        }
        "lexical_declaration" | "variable_declarator" => {
            if let Some(name) = node.child_by_field_name("name") {
                if name.kind() == "identifier" {
                    if let Ok(text) = name.utf8_text(source) {
                        out.push(ParsedSymbol {
                            symbol: text.to_string(),
                            kind: "const".to_string(),
                            line: node.start_position().row as u32 + 1,
                        });
                    }
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
    symbols: &[ParsedSymbol],
    edges: &mut Vec<ParsedEdge>,
) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if let Some(callee) = callee_name(func, source) {
                if let Some(caller) = symbols.first().map(|s| s.symbol.clone()) {
                    edges.push(ParsedEdge {
                        caller_symbol: caller,
                        callee_symbol: callee,
                        line: node.start_position().row as u32 + 1,
                    });
                }
            }
        }
    }
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            collect_calls(child, source, symbols, edges);
        }
    }
}

fn callee_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "property_identifier" => node
            .utf8_text(source)
            .ok()
            .map(|s| s.to_string()),
        "member_expression" => node
            .child_by_field_name("property")
            .and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string())),
        _ => None,
    }
}
