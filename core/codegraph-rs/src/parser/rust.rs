use super::{ParsedEdge, ParsedSymbol};
use tree_sitter::Node;

pub(crate) struct ParseOutput {
    pub symbols: Vec<ParsedSymbol>,
    pub edges: Vec<ParsedEdge>,
}

pub(crate) fn parse(source: &str) -> ParseOutput {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into()).ok();
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
    collect_all(root, bytes, &mut symbols, &mut edges);
    ParseOutput { symbols, edges }
}

fn collect_all(
    node: Node<'_>,
    source: &[u8],
    symbols: &mut Vec<ParsedSymbol>,
    edges: &mut Vec<ParsedEdge>,
) {
    match node.kind() {
        "function_item" | "struct_item" | "enum_item" | "trait_item" | "type_item" => {
            if let Some(name) = node.child_by_field_name("name")
                && let Ok(text) = name.utf8_text(source)
            {
                symbols.push(ParsedSymbol {
                    symbol: text.to_string(),
                    kind: node.kind().trim_end_matches("_item").to_string(),
                    line: node.start_position().row as u32 + 1,
                    start_col: node.start_position().column as u32 + 1,
                    end_line: node.end_position().row as u32 + 1,
                    end_col: node.end_position().column as u32 + 1,
                });
            }
        }
        _ => {}
    }
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
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            collect_all(child, source, symbols, edges);
        }
    }
}

fn callee_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "type_identifier" | "field_identifier" => {
            node.utf8_text(source).ok().map(|s| s.to_string())
        }
        "scoped_identifier" => node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string())),
        "field_expression" => node
            .child_by_field_name("field")
            .and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string())),
        _ => None,
    }
}

fn enclosing_symbol(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        match ancestor.kind() {
            "function_item" | "impl_item" => {
                if let Some(name) = ancestor.child_by_field_name("name")
                    && let Ok(text) = name.utf8_text(source)
                {
                    return Some(text.to_string());
                }
            }
            "closure_expression" | "async_block" => {}
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
    fn closure_call_attributed_to_enclosing_fn() {
        let src = r#"
fn caller() {
    let add = |a, b| a + b;
    add(1, 2);
}
"#;
        let out = parse(src);
        let edge = out
            .edges
            .iter()
            .find(|e| e.callee_symbol == "add")
            .expect("add edge");
        assert_eq!(edge.caller_symbol, "caller");
    }

    #[test]
    fn async_block_call_attributed_to_enclosing_fn() {
        let src = r#"
async fn run() {
    helper();
}
"#;
        let out = parse(src);
        let symbols: Vec<_> = out.symbols.iter().map(|s| s.symbol.as_str()).collect();
        assert!(symbols.contains(&"run"), "should find async fn");
        let edge = out
            .edges
            .iter()
            .find(|e| e.callee_symbol == "helper")
            .expect("helper edge");
        assert_eq!(
            edge.caller_symbol, "run",
            "async fn call attributed to enclosing fn"
        );
    }

    #[test]
    fn impl_block_methods() {
        let src = r#"
struct Foo;
impl Foo {
    fn new() -> Self { Foo }
    fn method(&self) { self.other(); }
    fn other(&self) {}
}
"#;
        let out = parse(src);
        let symbols: Vec<_> = out
            .symbols
            .iter()
            .map(|s| (s.symbol.as_str(), s.kind.as_str()))
            .collect();
        assert!(symbols.contains(&("Foo", "struct")));
        assert!(symbols.contains(&("new", "function")));
        assert!(symbols.contains(&("method", "function")));
        assert!(symbols.contains(&("other", "function")));
    }

    #[test]
    fn method_call_inside_impl() {
        let src = r#"
struct Bar;
impl Bar {
    fn run(&self) {
        self/helper();
    }
    fn helper(&self) {}
}
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
}
