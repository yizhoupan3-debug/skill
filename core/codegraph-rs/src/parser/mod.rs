pub mod common;
mod go;
mod python;
mod rust;
mod typescript;

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSymbol {
    pub symbol: String,
    pub kind: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEdge {
    pub caller_symbol: String,
    pub callee_symbol: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    pub path: String,
    pub language: String,
    pub mtime_ns: i64,
    pub content_hash: String,
    pub symbols: Vec<ParsedSymbol>,
    pub edges: Vec<ParsedEdge>,
}

pub fn parse_file(path: &Path, contents: &str, mtime_ns: i64) -> Option<ParsedFile> {
    let path_str = path.to_string_lossy();
    let language = common::detect_language(&path_str)?;
    let rel_path = path_str.replace('\\', "/");
    let (symbols, edges) = match language {
        "rust" => {
            let parsed = rust::parse(contents);
            (parsed.symbols, parsed.edges)
        }
        "typescript" => {
            let tsx = rel_path.ends_with(".tsx");
            let parsed = typescript::parse(contents, tsx);
            (parsed.symbols, parsed.edges)
        }
        "python" => {
            let parsed = python::parse(contents);
            (parsed.symbols, parsed.edges)
        }
        "go" => {
            let parsed = go::parse(contents);
            (parsed.symbols, parsed.edges)
        }
        _ => return None,
    };
    Some(ParsedFile {
        path: rel_path,
        language: language.to_string(),
        mtime_ns,
        content_hash: String::new(),
        symbols,
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_file;
    use std::path::Path;

    #[test]
    fn parses_rust_function_and_call() {
        let src = r#"
fn helper() {}
fn main() {
    helper();
}
"#;
        let parsed = parse_file(Path::new("main.rs"), src, 1).expect("parse file");
        assert!(parsed.symbols.iter().any(|s| s.symbol == "main"));
        assert!(parsed.edges.iter().any(|e| e.callee_symbol == "helper"));
    }
}
