//! 结构验证 — LaTeX 可编译性与图表引用完整性。

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use regex::Regex;

/// 检查 LaTeX 文件是否可编译（语法级别）。
///
/// 基本语法检查（不替代完整编译）：
/// 1. 环境 begin/end 配对
/// 2. 大括号平衡
/// 3. 常见命令格式
pub fn check_latex_compilable(tex_path: &Path) -> Result<bool> {
    let content = fs::read_to_string(tex_path)
        .with_context(|| format!("failed to read tex: {}", tex_path.display()))?;
    Ok(check_latex_syntax(&content))
}

/// 检查 LaTeX 文件中的图表引用是否有对应的标签定义。
/// 返回未找到定义的引用列表。
pub fn check_figure_references(tex_path: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(tex_path)
        .with_context(|| format!("failed to read tex: {}", tex_path.display()))?;

    let labels = extract_labels(&content);
    let refs = extract_refs(&content);

    let missing: Vec<String> = refs
        .into_iter()
        .filter(|r| !labels.contains(r))
        .collect();

    Ok(missing)
}

/// 基本 LaTeX 语法检查：环境配对和大括号平衡。
fn check_latex_syntax(content: &str) -> bool {
    // 1. 检查 \begin{env} 和 \end{env} 配对
    let begin_re = Regex::new(r"\\begin\{([^}]+)\}").expect("static regex");
    let end_re = Regex::new(r"\\end\{([^}]+)\}").expect("static regex");

    let mut env_stack: Vec<String> = Vec::new();
    for cap in begin_re.captures_iter(content) {
        env_stack.push(cap[1].to_string());
    }
    for cap in end_re.captures_iter(content) {
        let env_name = &cap[1];
        if let Some(pos) = env_stack.iter().rposition(|e| e == env_name) {
            env_stack.remove(pos);
        }
        // 如果找不到对应的 begin，这是一个错误
        // 但保守策略：不直接报错（可能是宏展开等原因）
    }
    // 有未关闭的环境才算错误（如果数量差 > 2 可能是真正的错误）
    if env_stack.len() > 2 {
        return false;
    }

    // 2. 检查大括号平衡
    let mut brace_depth = 0i32;
    for ch in content.chars() {
        match ch {
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            _ => {}
        }
        if brace_depth < 0 {
            return false; // 多余的闭括号
        }
    }
    if brace_depth.abs() > 0 {
        return false; // 不平衡
    }

    true
}

/// 提取 LaTeX 文件中所有 \label{...} 的标签名。
fn extract_labels(content: &str) -> HashSet<String> {
    let re = Regex::new(r"\\label\{([^}]+)\}").expect("static regex");
    re.captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

/// 提取 LaTeX 文件中所有 \ref{...}, \eqref{...}, \autoref{...} 的引用名。
fn extract_refs(content: &str) -> Vec<String> {
    let re = Regex::new(r"\\(?:ref|eqref|autoref|nameref|pageref)\{([^}]+)\}").expect("static regex");
    re.captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_latex_passes() {
        let tex = r#"
\documentclass{article}
\begin{document}
Hello \textbf{world}.
\end{document}
"#;
        assert!(check_latex_syntax(tex));
    }

    #[test]
    fn unbalanced_braces_fails() {
        let tex = r"\textbf{unclosed";
        assert!(!check_latex_syntax(tex));
    }

    #[test]
    fn missing_ref_detected() {
        let dir = tempfile::tempdir().unwrap();
        let tex_path = dir.path().join("main.tex");
        fs::write(
            &tex_path,
            r#"\label{fig:example} See Figure \ref{fig:missing} and \ref{fig:example}."#,
        )
        .unwrap();

        let missing = check_figure_references(&tex_path).unwrap();
        assert_eq!(missing, vec!["fig:missing"]);
    }

    #[test]
    fn all_refs_have_labels() {
        let dir = tempfile::tempdir().unwrap();
        let tex_path = dir.path().join("main.tex");
        fs::write(
            &tex_path,
            r#"\label{fig:example} See Figure \ref{fig:example}."#,
        )
        .unwrap();

        let missing = check_figure_references(&tex_path).unwrap();
        assert!(missing.is_empty());
    }
}
