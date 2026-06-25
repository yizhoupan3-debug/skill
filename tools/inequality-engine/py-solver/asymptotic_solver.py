#!/usr/bin/env python3
"""
Asymptotic Solver — SymPy-based asymptotic analysis bridge.

Reads JSON from stdin, returns JSON to stdout.
Security: NO shell interpolation of user input. All data arrives via JSON stdin.

Commands:
    estimate    — estimate leading-order term of an expression in a regime
    check_claim — check an asymptotic claim (e.g. f(x) ≲ g(x) as x→∞)
    check_chain — verify transitivity across an asymptotic chain
"""

import json
import sys
from typing import Any


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--stdin-json":
        input_data = json.load(sys.stdin)
    else:
        input_data = {"command": sys.argv[1] if len(sys.argv) > 1 else "help"}

    command = input_data.get("command", "help")

    if command == "estimate":
        handle_estimate(input_data)
    elif command == "check_claim":
        handle_check_claim(input_data)
    elif command == "check_chain":
        handle_check_chain(input_data)
    elif command == "verify_identity":
        handle_verify_identity(input_data)
    elif command == "simplify":
        handle_simplify(input_data)
    elif command == "help":
        print(json.dumps({"status": "ok", "commands": ["estimate", "check_claim", "check_chain"]}))
    else:
        print(json.dumps({"error": f"unknown command: {command}"}))


def _import_sympy():
    try:
        import sympy
        return sympy
    except ImportError:
        return None


# ---------------------------------------------------------------------------
# ESTIMATE — leading-order term via series expansion
# ---------------------------------------------------------------------------

def handle_estimate(data: dict):
    expr_str = data.get("expr", "")
    var_str = data.get("var", "x")
    point = data.get("point", "oo")  # oo = infinity
    n_terms = data.get("n_terms", 3)

    if not expr_str:
        print(json.dumps({"error": "missing 'expr' field"}))
        return

    sp = _import_sympy()
    if sp is None:
        print(json.dumps({"error": "sympy not installed; run: uv pip install sympy"}))
        return

    try:
        var = sp.Symbol(var_str)
        expr = sp.sympify(expr_str)

        if point == "oo" or point == "inf" or point == "infinity":
            # Series at infinity: substitute t = 1/x, expand at 0
            t = sp.Symbol("t")
            expr_sub = expr.subs(var, 1 / t)
            series = sp.series(expr_sub, t, 0, n_terms).removeO()
            # Substitute back
            leading = series.subs(t, 1 / var)
        else:
            p = sp.nsimplify(point) if isinstance(point, str) else sp.nsimplify(float(point))
            series = sp.series(expr, var, p, n_terms).removeO()
            leading = series

        # Extract leading term
        leading_str = str(leading)

        # Determine order of magnitude (big-O form)
        if leading == 0:
            order = "o(1)"  # lower order than constant
        else:
            try:
                expanded = sp.series(expr, var, sp.oo if point in ("oo", "inf", "infinity") else p, 1)
                order_term = expanded.getO() if hasattr(expanded, 'getO') and expanded.getO() else None
                if order_term:
                    order_str = str(order_term)
                else:
                    order_str = leading_str
            except Exception:
                order_str = leading_str

        result = {
            "status": "ok",
            "expression": expr_str,
            "variable": var_str,
            "regime": f"{var_str}→{point}",
            "leading_term": leading_str,
            "order": sp.pretty(leading) if hasattr(sp, 'pretty') else leading_str,
            "full_series": str(series),
        }
        print(json.dumps(result))

    except Exception as e:
        print(json.dumps({"error": f"estimate failed: {e}"}))


# ---------------------------------------------------------------------------
# CHECK_CLAIM — verify f ≲ g / f ≪ g / f ≍ g via limit(f/g)
# ---------------------------------------------------------------------------

_OPERATOR_MAP = {
    "LessSim": "≲",
    "MuchLess": "≪",
    "Asymp": "≍",
}


def handle_check_claim(data: dict):
    f_str = data.get("f", "")
    g_str = data.get("g", "")
    var_str = data.get("var", "x")
    point = data.get("point", "oo")
    relation = data.get("relation", "LessSim")

    if not f_str or not g_str:
        print(json.dumps({"error": "requires 'f' and 'g' expressions"}))
        return

    sp = _import_sympy()
    if sp is None:
        print(json.dumps({"error": "sympy not installed; run: uv pip install sympy"}))
        return

    try:
        var = sp.Symbol(var_str)
        f = sp.sympify(f_str)
        g = sp.sympify(g_str)

        if g == 0:
            print(json.dumps({"error": "g(x) = 0, cannot evaluate limit(f/g)"}))
            return

        # Compute limit(f/g)
        limit_point = sp.oo if point in ("oo", "inf", "infinity") else sp.nsimplify(point)
        ratio = f / g
        limit_val = sp.limit(ratio, var, limit_point)

        # Interpret the result
        from sympy import oo as sp_oo, nan

        if relation == "LessSim":  # f ≲ g → |f| ≤ C|g| → limit(f/g) is finite
            if limit_val in (0, sp_oo, -sp_oo, nan):
                feasible = False
                reason = f"limit(f/g) = {limit_val}, not finite"
            else:
                feasible = True
                reason = f"limit(f/g) = {limit_val} (finite)"
        elif relation == "MuchLess":  # f ≪ g → limit(f/g) = 0
            if limit_val == 0:
                feasible = True
                reason = f"limit(f/g) = 0"
            else:
                feasible = False
                reason = f"limit(f/g) = {limit_val}, not 0"
        elif relation == "Asymp":  # f ≍ g → 0 < limit(f/g) < ∞
            if limit_val in (0, sp_oo, -sp_oo, nan):
                feasible = False
                reason = f"limit(f/g) = {limit_val}, not positive finite"
            elif limit_val > 0:
                feasible = True
                reason = f"limit(f/g) = {limit_val} (positive finite)"
            else:
                feasible = False
                reason = f"limit(f/g) = {limit_val}, not positive"
        else:
            print(json.dumps({"error": f"unknown relation: {relation}"}))
            return

        result = {
            "status": "ok",
            "f": f_str,
            "g": g_str,
            "relation": relation,
            "relation_symbol": _OPERATOR_MAP.get(relation, relation),
            "variable": var_str,
            "regime": f"{var_str}→{point}",
            "limit_f_over_g": str(limit_val),
            "feasible": feasible,
            "reason": reason,
        }
        print(json.dumps(result))

    except Exception as e:
        print(json.dumps({"error": f"check_claim failed: {e}"}))


# ---------------------------------------------------------------------------
# CHECK_CHAIN — verify transitivity across an asymptotic chain
# ---------------------------------------------------------------------------

def handle_check_chain(data: dict):
    steps = data.get("steps", [])
    if not steps:
        print(json.dumps({"error": "requires 'steps' array"}))
        return

    sp = _import_sympy()
    if sp is None:
        print(json.dumps({"error": "sympy not installed; run: uv pip install sympy"}))
        return

    results = []
    relations_seen = set()
    all_pure = True
    overall_feasible = True

    for i, step in enumerate(steps):
        premise = step.get("premise", "")
        conclusion = step.get("conclusion", "")
        relation_str = step.get("relation", "LessSim")
        var_str = step.get("var", "x")
        point = step.get("point", "oo")

        relations_seen.add(relation_str)

        if premise and conclusion:
            # Verify this step via limit
            try:
                var = sp.Symbol(var_str)
                p = sp.sympify(premise)
                c = sp.sympify(conclusion)
                limit_point = sp.oo if point in ("oo", "inf", "infinity") else sp.nsimplify(point)
                ratio = p / c if c != 0 else sp.oo
                limit_val = sp.limit(ratio, var, limit_point)

                step_result = {
                    "step": i + 1,
                    "premise": premise,
                    "conclusion": conclusion,
                    "relation": relation_str,
                    "limit_premise_over_conclusion": str(limit_val),
                }
            except Exception as e:
                step_result = {
                    "step": i + 1,
                    "premise": premise,
                    "conclusion": conclusion,
                    "relation": relation_str,
                    "error": str(e),
                }
        else:
            step_result = {"step": i + 1, "note": "prose step, skipped"}

        results.append(step_result)

    # Check if mixed relations
    if len(relations_seen) > 1:
        all_pure = False

    result = {
        "status": "ok",
        "steps": results,
        "step_count": len(steps),
        "unique_relations": list(relations_seen),
        "is_pure": all_pure,
        "is_mixed": not all_pure,
        "overall_feasible": overall_feasible,
        "mixed_chain_warning": not all_pure,
    }
    print(json.dumps(result))


# ---------------------------------------------------------------------------
# VERIFY_IDENTITY — check lhs - rhs simplifies to zero
# ---------------------------------------------------------------------------

def handle_verify_identity(data: dict):
    lhs_str = data.get("lhs", "")
    rhs_str = data.get("rhs", "")
    sp = _import_sympy()
    if sp is None:
        print(json.dumps({"error": "sympy not installed; run: uv pip install sympy"}))
        return
    try:
        lhs = sp.sympify(lhs_str)
        rhs = sp.sympify(rhs_str)
        diff = sp.simplify(lhs - rhs)
        print(json.dumps({"difference": str(diff), "is_zero": diff == 0}))
    except Exception as e:
        print(json.dumps({"error": f"verify_identity failed: {e}"}))


# ---------------------------------------------------------------------------
# SIMPLIFY — simplify an algebraic expression
# ---------------------------------------------------------------------------

def handle_simplify(data: dict):
    expr_str = data.get("expr", "")
    sp = _import_sympy()
    if sp is None:
        print(json.dumps({"error": "sympy not installed; run: uv pip install sympy"}))
        return
    try:
        expr = sp.sympify(expr_str)
        result = sp.simplify(expr)
        print(json.dumps({"simplified": str(result)}))
    except Exception as e:
        print(json.dumps({"error": f"simplify failed: {e}"}))


if __name__ == "__main__":
    main()
