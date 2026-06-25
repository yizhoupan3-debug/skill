use super::{ParsedEdge, ParsedSymbol};
use tree_sitter::Node;

pub(crate) struct ParseOutput {
    pub symbols: Vec<ParsedSymbol>,
    pub edges: Vec<ParsedEdge>,
}

pub(crate) fn parse(source: &str) -> ParseOutput {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_go::LANGUAGE.into()).ok();
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
            if let Some(name) = node.child_by_field_name("name")
                && let Ok(text) = name.utf8_text(source) {
                    symbols.push(ParsedSymbol {
                        symbol: text.to_string(),
                        kind: node.kind().trim_end_matches("_declaration").to_string(),
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
                if let Some(name) = ancestor.child_by_field_name("name")
                    && let Ok(text) = name.utf8_text(source) {
                        return Some(text.to_string());
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
    fn interface_method_not_extracted_as_symbol() {
        let src = r#"
type Reader interface {
    Read(p []byte) (n int, err error)
}
"#;
        let out = parse(src);
        let symbols: Vec<_> = out.symbols.iter().map(|s| (s.symbol.as_str(), s.kind.as_str())).collect();
        assert!(symbols.is_empty(), "interface method declarations inside type_declaration are not extracted: {:?}", symbols);
    }

    #[test]
    fn method_declaration_on_type() {
        let src = r#"
type Foo struct{}
func (f Foo) Bar() { f.Baz() }
func (f Foo) Baz() {}
"#;
        let out = parse(src);
        let symbols: Vec<_> = out.symbols.iter().map(|s| (s.symbol.as_str(), s.kind.as_str())).collect();
        assert!(symbols.contains(&("Bar", "method")), "should find method: {:?}", symbols);
        assert!(symbols.contains(&("Baz", "method")), "should find method: {:?}", symbols);
    }

    #[test]
    fn method_call_inside_method() {
        let src = r#"
type Svc struct{}
func (s Svc) Run() { s.helper() }
func (s Svc) helper() {}
"#;
        let out = parse(src);
        let edge = out.edges.iter().find(|e| e.callee_symbol == "helper").expect("helper edge");
        assert_eq!(edge.caller_symbol, "Run");
    }

    #[test]
    fn goroutine_call() {
        let src = r#"
func worker() {}
func main() {
    go worker()
}
"#;
        let out = parse(src);
        let symbols: Vec<_> = out.symbols.iter().map(|s| s.symbol.as_str()).collect();
        assert!(symbols.contains(&"worker"));
        assert!(symbols.contains(&"main"));
        let edge = out.edges.iter().find(|e| e.callee_symbol == "worker").expect("worker edge");
        assert_eq!(edge.caller_symbol, "main");
    }
}
