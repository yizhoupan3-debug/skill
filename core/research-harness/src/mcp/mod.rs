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
            "research_aigc_humanize",
            "对文本执行句法改写和词汇替换,降低 AIGC 检测风险",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "待降重文本"},
                    "language": {"type": "string", "enum": ["en", "zh"], "description": "语言：en=英语, zh=中文(默认 en)"},
                    "preserve_academic": {"type": "boolean", "description": "是否保持学术语气(默认 true)"}
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
                    "operation": {"type": "string", "enum": ["start", "submit_round", "status"], "description": "操作：start（默认）/submit_round/status"},
                    "max_rounds": {"type": "integer", "minimum": 1, "maximum": 100, "description": "最大轮次"},
                    "min_rounds": {"type": "integer", "minimum": 0, "maximum": 100, "description": "最小轮次"},
                    "consecutive_stable_required": {"type": "integer", "minimum": 1, "maximum": 50, "description": "连续稳定轮次要求"},
                    "round": {"type": "integer", "minimum": 0, "description": "当前轮次号"},
                    "findings": {"type": "array", "items": {"type": "object"}, "description": "本轮审稿发现"}
                },
                "required": ["operation"]
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
                    "regime": {"type": "string", "enum": ["oo", "inf", "0", "zero"], "description": "极限 regime: oo/inf=无穷大, 0/zero=趋于零（默认 oo）"}
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
            "验证证明 DAG 结构完备性，返回通过/失败及详情",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "DAG 名称（可选，默认 default）"}
                }
            }),
        ),
        tool_def(
            "math_proof_dag_status",
            "查看证明 DAG 进度摘要，含已完成节点数/总节点数/状态",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "DAG 名称（可选，默认 default）"}
                }
            }),
        ),
        tool_def(
            "math_sympy_verify",
            "代数恒等式验证：SymPy CAS (后端可用时) + 纯 Rust 符号引擎(降级)。注意：assumptions 参数已不再支持，如需上下文相关的条件化简请使用 math_sympy_simplify。",
            json!({
                "type": "object",
                "properties": {
                    "lhs": {"type": "string", "description": "等式左侧表达式"},
                    "rhs": {"type": "string", "description": "等式右侧表达式"}
                },
                "required": ["lhs", "rhs"]
            }),
        ),
        tool_def(
            "math_sympy_simplify",
            "表达式化简：SymPy CAS (后端可用时) + 纯 Rust 符号引擎(降级)",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待化简的数学表达式"},
                    "assumptions": {"type": "array", "items": {"type": "string"}, "description": "前提条件列表（可选，例如 [\"x > 0\"]），会传给 SymPy refine 做上下文敏感的化简"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_sympy_trig_simplify",
            "三角函数表达式化简：使用 SymPy trigsimp()，适用于 sin/cos/tan 等的恒等式化简",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待化简的三角函数表达式"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_sympy_subs",
            "符号表达式变量替换：将符号表达式中的变量替换为数值或其他表达式",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "包含待替换变量的符号表达式"},
                    "substitutions": {
                        "type": "object",
                        "description": "替换映射表 {变量/表达式: 新值}，例如 {\"x\": 2, \"y\": \"a + b\"}",
                        "additionalProperties": true
                    }
                },
                "required": ["expression", "substitutions"]
            }),
        ),
        tool_def(
            "math_sympy_limit",
            "计算符号表达式极限：支持有限点、正无穷(oo)、负无穷(-oo)及左右方向",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待求极限的表达式"},
                    "variable": {"type": "string", "description": "极限变量（默认 x）"},
                    "point": {"type": "string", "description": "极限点：\"0\", \"oo\", \"-oo\", 或其他数值"},
                    "direction": {"type": "string", "enum": ["+", "-"], "description": "方向：\"+\" 右极限, \"-\" 左极限（可选，默认双侧极限）"}
                },
                "required": ["expression", "point"]
            }),
        ),
        tool_def(
            "math_sympy_lambdify",
            "符号转数值函数：将符号表达式转为可调用函数并求值",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "符号表达式"},
                    "variables": {"type": "array", "items": {"type": "string"}, "description": "变量名列表（默认 [\"x\"]）"},
                    "values": {"type": "array", "items": {"type": "number"}, "description": "求值数值列表，顺序与 variables 对应（可选）"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_prove_inequality",
            "证明数学不等式：Z3 (非线性/SMT) + minilp (线性), 自动降级（底层使用 Z3 SMT 求解器）",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "不等式表达式"},
                    "timeout_ms": {"type": "integer", "description": "Z3 超时时间(毫秒), 默认 5000", "default": 5000}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_asymptotic_chain",
            "验证渐近关系链的正确性（纯 Rust 增长分类）",
            json!({
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "premise": {"type": "string", "description": "前提表达式"},
                                "conclusion": {"type": "string", "description": "结论表达式"},
                                "relation": {"type": "string", "enum": ["LessSim", "MuchLess", "Asymp", "MuchGreater"], "description": "渐近关系：LessSim=≲, MuchLess=≪, Asymp=≍, MuchGreater=≫"},
                                "justification": {"type": "string", "description": "证明理由（可选）"}
                            },
                            "required": ["premise", "conclusion", "relation"]
                        },
                        "description": "渐近步列表"
                    },
                    "variable": {"type": "string", "description": "变量名"},
                    "regime": {"type": "string", "enum": ["oo", "inf", "0", "zero"], "description": "极限 regime: oo/inf=无穷大, 0/zero=趋于零（默认 oo）"}
                },
                "required": ["steps", "variable"]
            }),
        ),
        tool_def(
            "math_backend_available",
            "检查验证后端可用状态：Z3 / SymPy / Lean / all",
            json!({
                "type": "object",
                "properties": {
                    "backend": {
                        "type": "string",
                        "enum": ["z3", "sympy", "lean", "all"],
                        "description": "要检查的后端（默认 all）"
                    }
                },
                "required": []
            }),
        ),
        tool_def(
            "math_lean_verify",
            "用 Lean 做定理形式化验证",
            json!({
                "type": "object",
                "required": ["script"],
                "properties": {
                    "script": {"type": "string", "description": "Lean 定理表达式"}
                }
            }),
        ),
        tool_def(
            "math_sympy_expand",
            "展开多项式表达式：使用 SymPy expand()，适用于乘积展开和幂展开",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待展开的数学表达式（如 \"(x+1)^2\"）"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_sympy_factor",
            "因式分解表达式：使用 SymPy factor()，适用于多项式因式分解",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待因式分解的表达式（如 \"x^2 + 2*x + 1\"）"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_sympy_series",
            "计算符号级数展开：使用 SymPy series()，支持在指定点展开到指定阶数",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待展开的表达式"},
                    "variable": {"type": "string", "description": "展开变量（默认 x）"},
                    "point": {"type": "number", "description": "展开点（默认 0）"},
                    "order": {"type": "integer", "description": "展开阶数（默认 6）"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_sympy_differentiate",
            "符号微分：使用 SymPy diff()，支持任意阶导数",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待求导的表达式"},
                    "variable": {"type": "string", "description": "微分变量（默认 x）"},
                    "order": {"type": "integer", "description": "求导阶数（默认 1）"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_sympy_integrate",
            "符号积分：使用 SymPy integrate()，支持定积分和不定积分",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "待积分的表达式"},
                    "variable": {"type": "string", "description": "积分变量（默认 x）"},
                    "lower": {"type": "number", "description": "定积分下限（可选），省略则执行不定积分"},
                    "upper": {"type": "number", "description": "定积分上限（可选），省略则执行不定积分"}
                },
                "required": ["expression"]
            }),
        ),
        tool_def(
            "math_sympy_solve",
            "解方程/方程组：使用 SymPy solve()，支持等号和表达式形式",
            json!({
                "type": "object",
                "properties": {
                    "equation": {"type": "string", "description": "待解的方程（如 \"x^2 - 4 = 0\" 或 \"x^2 - 4\"）"},
                    "variable": {"type": "string", "description": "求解变量（默认 x）"}
                },
                "required": ["equation"]
            }),
        ),
        tool_def(
            "math_sympy_dimension_propagate",
            "物理量纲传播验证：分析方程两侧量纲一致性",
            json!({
                "type": "object",
                "properties": {
                    "equation": {"type": "string", "description": "物理方程（如 \"F = m*a\"）"},
                    "dimensions": {
                        "type": "object",
                        "description": "变量→量纲映射，如 {\"F\": \"L*M*T^-2\", \"m\": \"M\", \"a\": \"L*T^-2\"}",
                        "additionalProperties": true
                    }
                },
                "required": ["equation", "dimensions"]
            }),
        ),
        // ── Z3 solver tools (in dispatch but missing schema — added 2026-07-01) ──
        tool_def(
            "math_z3_prove",
            "Z3 SMT 验证逻辑表达式（含量词、非线性算术）",
            json!({
                "type": "object",
                "required": ["expression"],
                "properties": {
                    "expression": {"type": "string", "description": "待验证的逻辑表达式（如 'x > 0 implies x + 1 > 0'）"}
                }
            }),
        ),
        tool_def(
            "math_z3_solver_push",
            "创建 Z3 求解器上下文快照（push n 层）",
            json!({
                "type": "object",
                "properties": {
                    "n": {"type": "integer", "description": "推入层数（默认 1）"}
                }
            }),
        ),
        tool_def(
            "math_z3_solver_pop",
            "恢复 Z3 求解器上下文快照（pop n 层）",
            json!({
                "type": "object",
                "properties": {
                    "n": {"type": "integer", "description": "弹出层数（默认 1）"}
                }
            }),
        ),
        tool_def(
            "math_z3_solver_add",
            "向 Z3 求解器添加约束表达式",
            json!({
                "type": "object",
                "required": ["expression"],
                "properties": {
                    "expression": {"type": "string", "description": "约束表达式字符串"}
                }
            }),
        ),
        tool_def(
            "math_z3_solver_check",
            "检查当前 Z3 求解器上下文的可满足性",
            json!({
                "type": "object",
                "properties": {
                    "timeout_ms": {"type": "integer", "description": "超时时间（毫秒，默认不限）"}
                }
            }),
        ),
        tool_def(
            "math_z3_solver_reset",
            "重置 Z3 求解器状态（清空全部约束）",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool_def(
            "math_z3_solver_batch",
            "批量执行 Z3 求解器操作（push/pop/add/check/reset）",
            json!({
                "type": "object",
                "required": ["steps"],
                "properties": {
                    "steps": {
                        "type": "array",
                        "description": "步骤列表",
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": {"type": "string", "enum": ["push", "pop", "add", "check", "reset"], "description": "操作类型"},
                                "n": {"type": "integer", "description": "push/pop 层数"},
                                "expression": {"type": "string", "description": "add 操作的约束表达式"},
                                "timeout_ms": {"type": "integer", "description": "check 操作的超时毫秒数"}
                            },
                            "required": ["action"]
                        }
                    }
                }
            }),
        ),
        // ── Z3 optimize / check system tools (inputSchema added 2026-07-05) ──
        tool_def(
            "math_z3_optimize",
            "Z3 约束优化：max/min 目标函数，支持多约束",
            json!({
                "type": "object",
                "required": ["objective", "constraints", "direction"],
                "properties": {
                    "objective": {"type": "string", "description": "目标函数表达式（如 'x + 2*y'）"},
                    "constraints": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "约束表达式列表（如 ['x >= 0', 'y <= 5']）"
                    },
                    "direction": {"type": "string", "enum": ["minimize", "maximize"], "description": "优化方向"}
                }
            }),
        ),
        tool_def(
            "math_z3_check_system",
            "Z3 多约束系统可满足性检查",
            json!({
                "type": "object",
                "required": ["constraints"],
                "properties": {
                    "constraints": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "约束表达式列表（如 ['x > 0', 'x < 10']）"
                    },
                    "timeout_ms": {"type": "integer", "description": "超时毫秒数（可选）"}
                }
            }),
        ),
        tool_def(
            "research_verification_prose",
            "验证论文文本质控（术语/slop/hedging）",
            json!({
                "type": "object",
                "properties": {
                    "check": {"type": "string", "enum": ["terminology", "slop", "hedging"], "description": "检查类型：terminology/slop/hedging"},
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
                    "check": {"type": "string", "enum": ["grim", "p_value", "multiple_comparison"], "description": "检查类型：grim/p_value/multiple_comparison"},
                    "mean": {"type": "number", "description": "样本均值（grim 检查时必须）"},
                    "n": {"type": "integer", "minimum": 1, "maximum": 1000000000, "description": "样本量（grim 检查时必须）"},
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
            "通用实验烟雾测试引擎 — 运行可执行实验模板，支持 LRU+TTL 缓存、并行子进程执行、参数注入为环境变量",
            json!({
                "type": "object",
                "properties": {
                    "template": {"type": "string", "description": "模板文件名（位于 templates/ 目录下的可执行文件）"},
                    "params": {
                        "type": "array",
                        "items": {"type": "object", "description": "参数键值对，如 {\"lr\": \"0.01\", \"bs\": \"32\"}"},
                        "description": "参数组合列表 — 每个元素启动一次独立的实验运行"
                    },
                    "concurrency": {"type": "integer", "minimum": 1, "maximum": 32, "description": "最大并行子进程数（1–32，默认 4）", "default": 4},
                    "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 300000, "description": "单次实验超时时间（毫秒，默认 60000）", "default": 60000},
                    "no_cache": {"type": "boolean", "description": "绕过 LRU+TTL 缓存（默认 false）", "default": false}
                },
                "required": ["template", "params"]
            }),
        ),
        tool_def(
            "research_ablation",
            "组件级 ablation 分析 — 跑基线 + 逐个去部件，返回贡献矩阵（每部件增益/损害/推荐）",
            json!({
                "type": "object",
                "properties": {
                    "template": {"type": "string", "description": "模板文件名（位于 templates/ 下的可执行文件）"},
                    "baseline_params": {
                        "type": "object",
                        "additionalProperties": {"type": "string"},
                        "description": "基线参数键值对，如 {\"lr\": \"0.01\", \"bs\": \"32\"}"
                    },
                    "components": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string", "description": "部件名称"},
                                "description": {"type": "string", "description": "部件描述"},
                                "ablation_params": {
                                    "type": "object",
                                    "additionalProperties": {"type": "string"},
                                    "description": "可选—去部件后使用的参数覆盖（默认同 baseline_params）"
                                }
                            },
                            "required": ["name"]
                        },
                        "description": "要测试的部件列表 — 每个元素定义一个独立 ablation"
                    },
                    "metrics": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "关注的指标名（如 [\"accuracy\", \"latency_ms\"]）— 空则自动检测所有数值字段"
                    },
                    "concurrency": {"type": "integer", "minimum": 1, "maximum": 32, "description": "最大并行子进程数（1–32，默认 4）", "default": 4},
                    "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 300000, "description": "单次实验超时毫秒（默认 60000）", "default": 60000},
                    "no_cache": {"type": "boolean", "description": "绕过 LRU+TTL 缓存（默认 false）", "default": false}
                },
                "required": ["template", "baseline_params", "components"]
            }),
        ),
        tool_def(
            "research_evaluate",
            "方案评估 — 对比 baseline（现有方案）vs candidate（候选方案）的功能覆盖/性能/集成成本/推荐。每个方案需要 template + params + capabilities 列表。",
            json!({
                "type": "object",
                "properties": {
                    "baseline": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string", "description": "方案名称"},
                            "template": {"type": "string", "description": "模板文件名"},
                            "params": {"type": "object", "additionalProperties": {"type": "string"}, "description": "实验参数"},
                            "capabilities": {"type": "array", "items": {"type": "string"}, "description": "功能点列表"}
                        },
                        "required": ["name", "template"]
                    },
                    "candidate": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string", "description": "方案名称"},
                            "template": {"type": "string", "description": "模板文件名"},
                            "params": {"type": "object", "additionalProperties": {"type": "string"}, "description": "实验参数"},
                            "capabilities": {"type": "array", "items": {"type": "string"}, "description": "功能点列表"}
                        },
                        "required": ["name", "template"]
                    },
                    "dimensions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string", "description": "评估维度名（必须对应模板输出的指标名）"},
                                "higher_is_better": {"type": "boolean", "description": "该维度是否越高越好", "default": true},
                                "weight": {"type": "number", "description": "权重（影响最终推荐），默认 1.0", "default": 1.0}
                            },
                            "required": ["name"]
                        },
                        "description": "评估维度列表"
                    },
                    "concurrency": {"type": "integer", "minimum": 1, "maximum": 32, "description": "最大并行子进程数（1–32，默认 4）", "default": 4},
                    "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 300000, "description": "单次实验超时毫秒（默认 60000）", "default": 60000},
                    "no_cache": {"type": "boolean", "description": "绕过 LRU+TTL 缓存（默认 false）", "default": false}
                },
                "required": ["baseline", "candidate", "dimensions"]
            }),
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
                    "path": {"type": "string", "description": "实验目录路径（seed/environment/data_versioned/checkpoint/full_audit 检查时必须）"},
                    "run_paths": {"type": "array", "items": {"type": "string"}, "description": "多次运行输出目录路径列表（deterministic 检查时必须至少2个；full_audit 可选）"}
                }
            }),
        ),
        tool_def(
            "research_verification_formal",
            "形式验证：量纲一致性检查、witness一致性检查、步骤依赖图完整性检查",
            json!({
                "type": "object",
                "properties": {
                    "check": {"type": "string", "enum": ["dimensional", "witness", "step_dependency"], "description": "验证类型：dimensional=量纲一致性, witness=特例值代入验证, step_dependency=步骤依赖图检查"},
                    "equation": {"type": "string", "description": "数学表达式字符串（dimensional/witness 检查时使用）"},
                    "witnesses": {
                        "type": "array",
                        "items": {"type": "object"},
                        "description": "特例值列表（witness 检查时必须），每个元素是变量名→数值的映射，如 {\"x\": 1, \"y\": 2}"
                    },
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string", "description": "步骤 ID"},
                                "depends_on": {"type": "array", "items": {"type": "string"}, "description": "依赖的步骤 ID 列表"},
                                "description": {"type": "string", "description": "步骤描述（可选）"}
                            }
                        },
                        "description": "步骤依赖列表（step_dependency 检查时必须），每个元素需含 id 和 depends_on"
                    }
                },
                "required": ["check"]
            }),
        ),
        // ── Auto theorem proving tools (added 2026-07-01) ──
        tool_def(
            "math_auto_prove",
            "自动定理证明：依次尝试 SymPy → Z3 → inequality engine，返回统一的证明结果和证明轨迹",
            json!({
                "type": "object",
                "required": ["lhs", "rhs"],
                "properties": {
                    "lhs": {"type": "string", "description": "等式左侧表达式"},
                    "rhs": {"type": "string", "description": "等式右侧表达式"},
                    "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 600000, "description": "超时时间（毫秒，默认 10000）"}
                }
            }),
        ),
        tool_def(
            "math_identity_chain",
            "验证等式链传递性：检查 a = b = c = d 中每一对是否相等，报告断裂位置",
            json!({
                "type": "object",
                "required": ["chain"],
                "properties": {
                    "chain": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "等式链表达式列表，如 [\"(x+1)^2\", \"x^2 + 2*x + 1\", \"x^2 + 2*x + 1\"]"
                    }
                }
            }),
        ),
        tool_def(
            "math_tighten_bounds",
            "Z3 不等式边界细化：对单变量约束逐步收紧变量范围，返回更精确的区间（底层使用 Z3 二分搜索迭代）",
            json!({
                "type": "object",
                "required": ["expression", "variable", "lower", "upper"],
                "properties": {
                    "expression": {"type": "string", "description": "约束表达式（如 \"x^2 <= 25\"）"},
                    "variable": {"type": "string", "description": "目标变量名"},
                    "lower": {"type": "number", "description": "初始下界"},
                    "upper": {"type": "number", "description": "初始上界"},
                    "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 60000, "description": "单次 Z3 查询超时（毫秒，默认 5000）"}
                }
            }),
        ),
        tool_def(
            "math_witness_consistency",
            "代入验证：给定等式和变量赋值列表，验证代入后两侧数值相等。支持随机批量生成",
            json!({
                "type": "object",
                "required": ["lhs", "rhs"],
                "properties": {
                    "lhs": {"type": "string", "description": "等式左侧表达式"},
                    "rhs": {"type": "string", "description": "等式右侧表达式"},
                    "witnesses": {
                        "type": "array",
                        "items": {"type": "object"},
                        "description": "自定义赋值列表（可选），每个元素是 {变量→数值} 映射"
                    },
                    "num_random": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 100000,
                        "description": "自动生成的随机测试数量（可选，默认 0；witnesses 未提供时默认 50）"
                    },
                    "seed": {
                        "type": "integer",
                        "description": "随机种子（可选，默认 42）"
                    }
                }
            }),
        ),
        tool_def(
            "math_check_homomorphism",
            "检查两个表达式间的同态/同构关系：f(x) = g(x+c), f(x) = k*g(x), f(x) = k*g(x+c)",
            json!({
                "type": "object",
                "required": ["f", "g"],
                "properties": {
                    "f": {"type": "string", "description": "第一个表达式"},
                    "g": {"type": "string", "description": "第二个表达式"}
                }
            }),
        ),
        tool_def(
            "math_proof_trace_record",
            "获取指定验证操作的证明轨迹记录",
            json!({
                "type": "object",
                "required": ["lhs", "rhs"],
                "properties": {
                    "lhs": {"type": "string", "description": "等式左侧"},
                    "rhs": {"type": "string", "description": "等式右侧"}
                }
            }),
        ),
        // ── Perturbation expansion tool (added 2026-07-03) ──
        tool_def(
            "math_perturbation_expand",
            "正则摄动展开：将微分方程按小参数展开为幂级数形式 u = u0 + ε·u1 + ε²·u2 + ...，逐阶求解。适用于弱非线性振动、边界层问题等。",
            json!({
                "type": "object",
                "required": ["equation", "parameter"],
                "properties": {
                    "equation": {
                        "type": "string",
                        "description": "含小参数的微分方程表达式（表达式=0），如 Derivative(u(t), t, 2) + u(t) + eps*u(t)**3"
                    },
                    "variable": {
                        "type": "string",
                        "description": "自变量名称（默认 x）"
                    },
                    "parameter": {
                        "type": "string",
                        "description": "小参数名称，如 eps, epsilon, ε"
                    },
                    "order": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10,
                        "description": "展开阶数（默认 2）。order=1 展开到 O(ε), order=2 到 O(ε²)"
                    },
                    "bc": {
                        "type": "string",
                        "description": "边界/初始条件，如 u(0)=1, u'(0)=0（可选）"
                    }
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
        "跨 arXiv 和 Semantic Scholar 搜索学术文献，支持模糊匹配和权威排序",
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "搜索查询（纯文本）"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "每源最大结果数（默认 20，最大 100）"},
                "source": {"type": "string", "enum": ["all", "semantic-scholar", "arxiv"], "description": "搜索源（默认 all）"},
                "year_from": {"type": "integer", "description": "最早出版年份（含）"},
                "year_to": {"type": "integer", "description": "最晚出版年份（含）"},
                "sort_by": {"type": "string", "enum": ["relevance", "date"], "description": "排序方式（默认 relevance）"},
                "categories": {"type": "string", "description": "arXiv 分类过滤，逗号分隔（如 'cs.AI,cs.LG'）"},
                "advanced_query": {"type": "string", "description": "arXiv 高级原生查询（如 'au:vaswani AND ti:attention'）。覆盖 query（仅 arXiv）"},
                "fuzzy_query": {"type": "boolean", "description": "启用模糊/宽泛匹配 — arXiv 使用原始文本 OR 展开而非 all: 关键词（默认 false）"},
                "prefer_authoritative": {"type": "boolean", "description": "启用双通道权威排序 — 获取多达 3 倍结果，按 DOI/venue/引用量/时效性评分，降低预印本优先级（默认 false）"}
            },
            "required": ["query"]
        }),
    )
}
