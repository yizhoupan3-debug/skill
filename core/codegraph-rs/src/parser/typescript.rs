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
        "function_declaration"
        | "method_definition"
        | "class_declaration"
        | "interface_declaration"
        | "type_alias_declaration" => {
            if let Some(name) = node.child_by_field_name("name")
                && let Ok(text) = name.utf8_text(source) {
                    symbols.push(ParsedSymbol {
                        symbol: text.to_string(),
                        kind: node
                            .kind()
                            .replace("_declaration", "")
                            .replace("_definition", ""),
                        line: node.start_position().row as u32 + 1,
                        start_col: node.start_position().column as u32 + 1,
                        end_line: node.end_position().row as u32 + 1,
                        end_col: node.end_position().column as u32 + 1,
                    });
                }
        }
        "lexical_declaration" | "variable_declarator" => {
            if let Some(name) = node.child_by_field_name("name")
                && name.kind() == "identifier"
                    && let Ok(text) = name.utf8_text(source) {
                        symbols.push(ParsedSymbol {
                            symbol: text.to_string(),
                            kind: "const".to_string(),
                            line: node.start_position().row as u32 + 1,
                            start_col: node.start_position().column as u32 + 1,
                            end_line: node.end_position().row as u32 + 1,
                            end_col: node.end_position().column as u32 + 1,
                        });
                    }
        }
        _ => {}
    }
    // Collect call edges at this node
    if node.kind() == "call_expression"
        && let Some(func) = node.child_by_field_name("function")
            && let (Some(caller), Some(callee)) =
                (enclosing_symbol(node, source), callee_name(func, source))
            {
                edges.push(ParsedEdge {
                    caller_symbol: caller,
                    callee_symbol: callee,
                    line: node.start_position().row as u32 + 1,
                });
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
        "identifier" | "property_identifier" => node.utf8_text(source).ok().map(|s| s.to_string()),
        "member_expression" => node
            .child_by_field_name("property")
            .and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string())),
        _ => None,
    }
}

/// Walk up AST to find nearest enclosing named scope.
/// Supports arrow functions (via lexical_declaration parent) and class methods.
fn enclosing_symbol(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        match ancestor.kind() {
            "function_declaration" | "method_definition" | "class_declaration" => {
                if let Some(name) = ancestor.child_by_field_name("name")
                    && let Ok(text) = name.utf8_text(source) {
                        return Some(text.to_string());
                    }
            }
            // Arrow functions: const foo = () => { ... }
            "lexical_declaration" => {
                // Look for the variable_declarator child to get the name
                for i in 0..ancestor.named_child_count() {
                    if let Some(child) = ancestor.named_child(i)
                        && child.kind() == "variable_declarator"
                            && let Some(name) = child.child_by_field_name("name")
                                && let Ok(text) = name.utf8_text(source) {
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

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn arrow_function_is_const_symbol() {
        let src = r#"
const greet = (name: string) => { console.log(name); };
"#;
        let out = parse(src, false);
        let symbols: Vec<_> = out.symbols.iter().map(|s| (s.symbol.as_str(), s.kind.as_str())).collect();
        assert!(symbols.contains(&("greet", "const")), "arrow fn should be const: {:?}", symbols);
    }

    #[test]
    fn arrow_function_call_edge() {
        let src = r#"
const compute = (x: number) => { return x * 2; };
function run() {
    compute(5);
}
"#;
        let out = parse(src, false);
        let edge = out.edges.iter().find(|e| e.callee_symbol == "compute").expect("compute edge");
        assert_eq!(edge.caller_symbol, "run");
    }

    #[test]
    fn interface_declaration() {
        let src = r#"
interface Config {
    name: string;
    debug: boolean;
}
"#;
        let out = parse(src, false);
        let symbols: Vec<_> = out.symbols.iter().map(|s| (s.symbol.as_str(), s.kind.as_str())).collect();
        assert!(symbols.contains(&("Config", "interface")), "should find interface: {:?}", symbols);
    }

    #[test]
    fn interface_methods_not_extracted() {
        let src = r#"
interface Logger {
    log(msg: string): void;
    error(msg: string): void;
}
"#;
        let out = parse(src, false);
        let method_symbols: Vec<_> = out.symbols.iter()
            .filter(|s| s.kind == "method")
            .map(|s| s.symbol.as_str())
            .collect();
        assert!(method_symbols.is_empty(), "interface methods should not be extracted as symbols: {:?}", method_symbols);
    }
}
