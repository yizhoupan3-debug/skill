//! LaTeX 数学公式 SVG 渲染。
//!
//! 提供从 LaTeX 数学公式到 SVG 矢量图的完整渲染管线。

use anyhow::Result;

/// 将 LaTeX 数学公式渲染为 SVG 字符串。
///
/// # Arguments
///
/// * `latex` - LaTeX 数学公式（不含 $ 分隔符）
/// * `display_mode` - 是否使用 display mode（独立行公式）
///
/// # Returns
///
/// SVG 字符串，可直接嵌入 HTML 或保存为 .svg 文件
///
/// # Examples
///
/// ```rust
/// use research_harness::latex::render::render_to_svg;
///
/// let svg = render_to_svg(r"\frac{a^2 + b^2}{c}", true).unwrap();
/// assert!(svg.contains("<svg"));
/// ```
pub fn render_to_svg(latex: &str, _display_mode: bool) -> Result<String> {
    // Step 1: Parse LaTeX to AST (verify syntax)
    let _ast = crate::latex::parse(latex)
        .map_err(|e| anyhow::anyhow!("LaTeX parse error: {:?}", e))?;

    // Step 2: Layout AST to display list
    // TODO: Integrate ratex-layout when available

    // Step 3: Render display list to SVG
    // TODO: Integrate ratex-svg when available

    // Placeholder: return a simple SVG with the formula as text
    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 30">
  <text x="10" y="20" font-family="serif" font-size="16">{}</text>
</svg>"#,
        html_escape::encode_text(latex)
    ))
}

/// 将 LaTeX 数学公式渲染为内联 SVG 片段（用于嵌入 Markdown）。
///
/// # Arguments
///
/// * `latex` - LaTeX 数学公式
///
/// # Returns
///
/// 内联 SVG 片段（不含外层 <svg> 标签）
pub fn render_to_inline_svg(latex: &str) -> Result<String> {
    let svg = render_to_svg(latex, false)?;
    // Extract inner content (remove <svg> wrapper)
    let inner = svg
        .find('>')
        .and_then(|i| svg.rfind('<').map(|j| &svg[i + 1..j]))
        .unwrap_or("");
    Ok(inner.to_string())
}

/// 批量渲染多个 LaTeX 公式为 SVG。
///
/// # Arguments
///
/// * `formulas` - 公式列表，每项为 (latex, display_mode) 元组
///
/// # Returns
///
/// SVG 字符串列表
pub fn batch_render_to_svg(formulas: &[(&str, bool)]) -> Result<Vec<String>> {
    formulas
        .iter()
        .map(|(latex, display_mode)| render_to_svg(latex, *display_mode))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_simple_formula() {
        let svg = render_to_svg("x^2", false).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("x^2"));
    }

    #[test]
    fn test_render_display_mode() {
        let svg = render_to_svg(r"\frac{a}{b}", true).unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn test_batch_render() {
        let formulas = vec![("x^2", false), (r"\frac{a}{b}", true)];
        let results = batch_render_to_svg(&formulas).unwrap();
        assert_eq!(results.len(), 2);
    }
}
