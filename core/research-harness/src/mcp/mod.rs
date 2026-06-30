//! MCP tool definitions and dispatch for research-harness.

use crate::mcp_tools::handle_research_tool;
use anyhow::Result;
use serde_json::{Value, json};

/// MCP tool definitions exposed by this server.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool_def(
            "research_aigc_check",
            "检测文本是否 AI 生成",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "待检测文本"},
                    "language": {"type": "string", "enum": ["en", "zh"], "description": "语言：en=英语, zh=中文"}
                },
                "required": ["text"]
            }),
        ),
        tool_def(
            "research_review_dimensions",
            "获取审稿维度 prompt",
            json!({
                "type": "object",
                "properties": {
                    "round": {"type": "integer", "description": "审稿轮次"},
                    "manuscript_summary": {"type": "string", "description": "论文摘要（可选）"}
                },
                "required": ["round"]
            }),
        ),
        tool_def(
            "research_claim_drift",
            "检测声明漂移和证据变化",
            json!({
                "type": "object",
                "properties": {
                    "original_claims": {"type": "array", "items": {"type": "object"}, "description": "原始声明列表（含 id/text/ceiling/evidence）"},
                    "current_claims": {"type": "array", "items": {"type": "object"}, "description": "当前声明列表"}
                },
                "required": ["original_claims", "current_claims"]
            }),
        ),
        tool_def(
            "research_review_loop",
            "运行对抗审稿循环",
            json!({
                "type": "object",
                "properties": {
                    "operation": {"type": "string", "description": "操作：start（默认）/submit_round/status"},
                    "max_rounds": {"type": "integer", "description": "最大轮次"},
                    "min_rounds": {"type": "integer", "description": "最小轮次"},
                    "consecutive_stable_required": {"type": "integer", "description": "连续稳定轮次要求"},
                    "round": {"type": "integer", "description": "当前轮次号"},
                    "findings": {"type": "array", "items": {"type": "object"}, "description": "本轮审稿发现"}
                }
            }),
        ),
        tool_def(
            "math_asymptotic_estimate",
            "估计数学表达式的渐近量级",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "数学表达式"},
                    "variable": {"type": "string", "description": "变量名（默认 x）"},
                    "regime": {"type": "string", "description": "极限 regime: oo=无穷大, 0=趋于零（默认 oo）"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_proof_dag_init",
            "初始化证明 DAG",
            json!({
                "type": "object",
                "properties": {
                    "goal": {"type": "string", "description": "证明目标陈述"},
                    "name": {"type": "string", "description": "证明名称（可选）"}
                },
                "required": ["goal"]
            }),
        ),
        tool_def(
            "math_proof_dag_decompose",
            "分解证明节点为子目标",
            json!({
                "type": "object",
                "properties": {
                    "parent_id": {"type": "string", "description": "父节点 ID"},
                    "children": {"type": "array", "items": {"type": "object"}, "description": "子目标节点列表"},
                    "and": {"type": "boolean", "description": "AND 分解或 OR 分解（默认 false=OR）"}
                },
                "required": ["parent_id", "children"]
            }),
        ),
        tool_def(
            "math_proof_dag_verify",
            "验证证明 DAG 结构完备性",
            json!({ "type": "object", "properties": {} }),
        ),
        tool_def(
            "math_proof_dag_status",
            "查看证明 DAG 进度摘要",
            json!({ "type": "object", "properties": {} }),
        ),
        tool_def(
            "math_sympy_verify",
            "SymPy 验证代数等式",
            json!({
                "type": "object",
                "properties": {
                    "lhs": {"type": "string", "description": "等式左侧表达式"},
                    "rhs": {"type": "string", "description": "等式右侧表达式"},
                    "assumptions": {"type": "array", "items": {"type": "string"}, "description": "前提条件（可选）"}
                },
                "required": ["lhs", "rhs"]
            }),
        ),
        tool_def(
            "math_sympy_simplify",
            "SymPy 化简表达式",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待化简的数学表达式"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_prove_inequality",
            "用 SMT solver (Z3) 证明数学不等式",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "不等式表达式"},
                    "variables": {"type": "array", "items": {"type": "string"}, "description": "变量名列表"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_asymptotic_chain",
            "验证渐近关系链的正确性",
            json!({ "type": "object", "properties": {} }),
        ),
        tool_def(
            "math_backend_available",
            "检查 Z3/SymPy/Lean 验证后端是否可用",
            json!({ "type": "object", "properties": {} }),
        ),
        tool_def(
            "math_lean_verify",
            "用 Lean 做定理形式化验证",
            json!({
                "type": "object",
                "required": ["expression"],
                "properties": {
                    "expression": {"type": "string", "description": "Lean 定理表达式"}
                }
            }),
        ),
        tool_def(
            "research_verification_prose",
            "验证论文文本质控（术语/slop/hedging）",
            json!({
                "type": "object",
                "properties": {
                    "check": {"type": "string", "description": "检查类型：terminology/slop/hedging"},
                    "text": {"type": "string", "description": "待检查文本"},
                    "glossary": {"type": "object", "description": "可选术语表（check=terminology 时使用）：{术语→标准译名} 映射"},
                    "language": {"type": "string", "enum": ["en", "zh"], "description": "可选语言（check=slop 时使用）：en=英语 (default), zh=中文"}
                },
                "required": ["check", "text"]
            }),
        ),
        tool_def(
            "research_verification_statistical",
            "验证统计声明（GRIM/p值/多重比较）",
            json!({
                "type": "object",
                "properties": {
                    "check": {"type": "string", "description": "检查类型：grim/p_value/multiple_comparison"},
                    "mean": {"type": "number", "description": "样本均值（grim 检查时必须）"},
                    "n": {"type": "integer", "description": "样本量（grim 检查时必须）"},
                    "decimals": {"type": "integer", "description": "小数位数（grim 检查可选，默认 2）"},
                    "observed": {"type": "number", "description": "观测 p 值（p_value 检查时必须）"},
                    "expected": {"type": "number", "description": "期望 p 值（p_value 检查时必须）"},
                    "tolerance": {"type": "number", "description": "允许误差（p_value 检查可选，默认 0.01）"},
                    "num_tests": {"type": "integer", "description": "检验总数（multiple_comparison 检查时必须）"},
                    "correction_applied": {"type": "boolean", "description": "是否已应用多重比较校正（multiple_comparison 检查可选）"}
                },
                "required": ["check"]
            }),
        ),
        tool_def(
            "research_smoke",
            "对 arXiv 和 Semantic Scholar 做数据源可达性烟雾测试",
            json!({ "type": "object", "properties": {} }),
        ),
        tool_def(
            "research_verification_literature",
            "验证文献引用准确性：DOI 可达性检查、声明覆盖率计算",
            json!({
                "type": "object",
                "properties": {
                    "check": {"type": "string", "enum": ["doi", "claim_coverage"], "description": "验证类型"},
                    "doi": {"type": "string", "description": "DOI 标识符（check=doi 时必须）"},
                    "claims": {"type": "array", "items": {"type": "string"}, "description": "声明列表"},
                    "references": {"type": "array", "items": {"type": "string"}, "description": "引用列表"}
                },
                "required": ["check"]
            }),
        ),
        tool_def(
            "research_verification_structure",
            "验证文档结构完整性：LaTeX 编译检查、图表引用一致性",
            json!({
                "type": "object",
                "required": ["check", "path"],
                "properties": {
                    "check": {"type": "string", "enum": ["latex", "figures"], "description": "验证类型"},
                    "path": {"type": "string", "description": "TeX 文件路径"}
                }
            }),
        ),
        tool_def(
            "research_verification_reproducibility",
            "验证实验可重复性：种子设置、确定性重跑、环境可复制、数据版本、checkpoint恢复、全审计",
            json!({
                "type": "object",
                "required": ["check"],
                "properties": {
                    "check": {"type": "string", "enum": ["seed", "deterministic", "environment", "data_versioned", "checkpoint", "full_audit"], "description": "验证类型"},
                    "path": {"type": "string", "description": "实验目录路径"}
                }
            }),
        ),
        tool_def(
            "research_verification_formal",
            "形式验证：量纲一致性检查",
            json!({
                "type": "object",
                "required": ["check", "equation"],
                "properties": {
                    "check": {"type": "string", "enum": ["dimensional"], "description": "验证类型"},
                    "equation": {"type": "string", "description": "数学表达式字符串"}
                }
            }),
        ),
        tool_literature_search(),
    ]
}

/// Dispatch a research-harness MCP tool call.
pub fn dispatch(name: &str, arguments: &Value) -> Result<Value> {
    let result = handle_research_tool(name, arguments)?;
    // handle_research_tool returns a JSON string — parse it
    let payload: Value = serde_json::from_str(&result)?;
    Ok(json!({
        "content": [{"type": "text", "text": result}],
        "structuredContent": payload,
    }))
}

fn tool_def(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

// ── Literature search tool ──

fn tool_literature_search() -> Value {
    tool_def(
        "research_literature_search",
        "Search academic literature across arXiv and Semantic Scholar. Supports fuzzy matching and authoritative filtering.",
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Search query (plain text)"},
                "limit": {"type": "integer", "description": "Max results per source (default 20, max 100)"},
                "source": {"type": "string", "enum": ["all", "semantic-scholar", "arxiv"], "description": "Source to search (default all)"},
                "year_from": {"type": "integer", "description": "Minimum publication year (inclusive)"},
                "year_to": {"type": "integer", "description": "Maximum publication year (inclusive)"},
                "sort_by": {"type": "string", "enum": ["relevance", "date"], "description": "Sort order (default relevance)"},
                "categories": {"type": "string", "description": "arXiv category filter, comma-separated (e.g. 'cs.AI,cs.LG')"},
                "advanced_query": {"type": "string", "description": "Advanced arXiv native query (e.g. 'au:vaswani AND ti:attention'). Overrides 'query' for arXiv only."},
                "fuzzy_query": {"type": "boolean", "description": "Enable fuzzy/broad matching — arXiv uses raw text with OR expansion instead of all: keyword (default false)"},
                "prefer_authoritative": {"type": "boolean", "description": "Enable two-pass authoritative ranking — fetches up to 3x results, scores by DOI/venue/citations/recency, demotes preprints (default false)"}
            },
            "required": ["query"]
        }),
    )
}
