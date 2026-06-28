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
        "function_definition" | "class_definition" => {
            if let Some(name) = node.child_by_field_name("name")
                && let Ok(text) = name.utf8_text(source)
            {
                symbols.push(ParsedSymbol {
                    symbol: text.to_string(),
                    kind: node.kind().trim_end_matches("_definition").to_string(),
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
    if node.kind() == "call"
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

/// Walk up AST to find nearest enclosing function/class definition.
fn enclosing_symbol(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        match ancestor.kind() {
            "function_definition" | "class_definition" => {
                if let Some(name) = ancestor.child_by_field_name("name")
                    && let Ok(text) = name.utf8_text(source)
                {
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::parse;

    #[test]
    fn class_methods_are_functions() {
        let src = r#"
class MyClass:
    def method(self):
        pass
    def other(self):
        self.method()
"#;
        let out = parse(src);
        let symbols: Vec<_> = out
            .symbols
            .iter()
            .map(|s| (s.symbol.as_str(), s.kind.as_str()))
            .collect();
        assert!(
            symbols.contains(&("MyClass", "class")),
            "should find class: {:?}",
            symbols
        );
        assert!(
            symbols.contains(&("method", "function")),
            "should find method: {:?}",
            symbols
        );
        assert!(
            symbols.contains(&("other", "function")),
            "should find other: {:?}",
            symbols
        );
    }

    #[test]
    fn method_call_inside_class() {
        let src = r#"
class Service:
    def run(self):
        self.helper()
    def helper(self):
        pass
"#;
        let out = parse(src);
        let edge = out
            .edges
            .iter()
            .find(|e| e.callee_symbol == "helper")
            .expect("helper edge");
        assert_eq!(
            edge.caller_symbol, "run",
            "self.method() attributed to enclosing fn"
        );
    }

    #[test]
    fn decorator_does_not_create_symbol() {
        let src = r#"
class MyClass:
    @staticmethod
    def decorated():
        pass
"#;
        let out = parse(src);
        let decorated_syms: Vec<_> = out
            .symbols
            .iter()
            .filter(|s| s.symbol.contains("decorator") || s.kind == "decorator")
            .collect();
        assert!(
            decorated_syms.is_empty(),
            "decorators should not produce symbols"
        );
        let symbols: Vec<_> = out.symbols.iter().map(|s| s.symbol.as_str()).collect();
        assert!(
            symbols.contains(&"decorated"),
            "decorated fn should still be extracted"
        );
    }

    #[test]
    fn nested_function_call() {
        let src = r#"
def outer():
    def inner():
        pass
    inner()
"#;
        let out = parse(src);
        let symbols: Vec<_> = out.symbols.iter().map(|s| s.symbol.as_str()).collect();
        assert!(symbols.contains(&"outer"));
        assert!(symbols.contains(&"inner"));
        let edge = out
            .edges
            .iter()
            .find(|e| e.callee_symbol == "inner")
            .expect("inner edge");
        assert_eq!(edge.caller_symbol, "outer");
    }
}
