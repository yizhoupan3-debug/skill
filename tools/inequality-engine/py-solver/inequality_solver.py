#!/usr/bin/env python3
"""
Inequality Solver — Z3 exact linear programming bridge.

Reads JSON from stdin, returns JSON to stdout.
Security: NO shell interpolation of user input. All data arrives via JSON stdin.

Commands:
    solve     — check feasibility of an InequalitySystem
    parse     — parse a LaTeX inequality string into an Inequality struct
"""

import json
import sys
from typing import Any


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--stdin-json":
        input_data = json.load(sys.stdin)
    else:
        # For direct invocation with a command argument
        input_data = {"command": sys.argv[1] if len(sys.argv) > 1 else "help"}

    command = input_data.get("command", "help")

    if command == "solve":
        handle_solve(input_data)
    elif command == "parse":
        handle_parse(input_data)
    elif command == "check_parse":
        # Just test if parse works, for diagnostic
        handle_check_parse(input_data)
    elif command == "help":
        print(json.dumps({"status": "ok", "commands": ["solve", "parse", "check_parse"]}))
    else:
        print(json.dumps({"error": f"unknown command: {command}"}))


# ---------------------------------------------------------------------------
# SOLVE
# ---------------------------------------------------------------------------

def handle_solve(data: dict):
    system = data.get("system")
    timeout_ms = data.get("timeout_ms", 5000)

    if not system:
        print(json.dumps({"error": "missing 'system' field"}))
        return

    constraints = system.get("constraints", [])
    if not constraints:
        print(json.dumps({"Feasible": {"model": {}}}))
        return

    try:
        import z3
    except ImportError:
        print(json.dumps({"Error": {"message": "z3-solver not installed; run: uv pip install z3-solver"}}))
        return

    solver = z3.Solver()
    solver.set("timeout", timeout_ms)
    var_map = {}

    for c in constraints:
        coeffs = c.get("coefficients", [])
        vars_ = c.get("vars", [])
        sense = c.get("sense", "Eq")
        rhs = c.get("rhs", 0.0)

        # Build Z3 expression
        expr = None
        for coeff, var_name in zip(coeffs, vars_):
            if var_name not in var_map:
                var_map[var_name] = z3.Real(var_name)
            term = z3.RealVal(float(coeff)) * var_map[var_name]
            expr = term if expr is None else expr + term

        if expr is None:
            expr = z3.RealVal(0)

        rhs_val = z3.RealVal(float(rhs))

        if sense == "Lt":
            solver.add(expr < rhs_val)
        elif sense == "Le":
            solver.add(expr <= rhs_val)
        elif sense == "Eq":
            solver.add(expr == rhs_val)
        elif sense == "Ge":
            solver.add(expr >= rhs_val)
        elif sense == "Gt":
            solver.add(expr > rhs_val)

    result = solver.check()

    if result == z3.sat:
        model = solver.model()
        model_dict = {}
        for var_name, z3_var in var_map.items():
            val = model[z3_var]
            if val is not None:
                model_dict[var_name] = float(val.as_fraction())
        print(json.dumps({"Feasible": {"model": model_dict}}))
    elif result == z3.unsat:
        # Try to extract unsat core
        try:
            core = solver.unsat_core()
            core_str = [str(d) for d in core] if core else ["(no core)"]
        except Exception:
            core_str = ["(unsat core extraction not supported)"]
        print(json.dumps({
            "Infeasible": {
                "proof_certificate": f"Z3 proved unsatisfiable. Unsat core: {core_str}"
            }
        }))
    else:
        print(json.dumps({"Timeout": {"timeout_ms": timeout_ms}}))


# ---------------------------------------------------------------------------
# PARSE — LaTeX inequality → Inequality struct
# ---------------------------------------------------------------------------

def handle_parse(data: dict):
    expr = data.get("expr", "")
    if not expr:
        print(json.dumps({"error": "missing 'expr' field"}))
        return

    try:
        # Try sympy first
        from sympy.parsing.sympy_parser import parse_expr
        from sympy import Symbol

        # Normalize LaTeX operators
        cleaned = expr.replace("\\leq", "<=").replace("\\le", "<=")
        cleaned = cleaned.replace("\\geq", ">=").replace("\\ge", ">=")
        cleaned = cleaned.replace("\\lt", "<").replace("\\gt", ">")
        cleaned = cleaned.replace("\\cdot", "*").replace(" ", "")

        # Find the inequality operator position
        import re
        op_match = re.search(r"<=|>=|==|=|<|>", cleaned)
        if not op_match:
            print(json.dumps({"error": "no inequality operator found"}))
            return

        lhs_str = cleaned[:op_match.start()]
        op_str = op_match.group()
        rhs_str = cleaned[op_match.end():]

        # Map operator
        sense_map = {
            "<": "Lt", "<=": "Le", "==": "Eq", "=": "Eq",
            ">=": "Ge", ">": "Gt",
        }
        sense = sense_map.get(op_str, "Eq")

        # Parse RHS as constant
        try:
            rhs = float(parse_expr(rhs_str))
        except Exception:
            rhs = 0.0

        # Parse LHS into variable terms
        # Use sympy to expand and collect coefficients
        lhs_expr = parse_expr(lhs_str)
        coeffs = {}
        if hasattr(lhs_expr, "free_symbols") and lhs_expr.free_symbols:
            for sym in lhs_expr.free_symbols:
                name = str(sym)
                # Extract coefficient by differentiating wrt the symbol
                from sympy import diff
                try:
                    coeff = float(diff(lhs_expr, Symbol(name)))
                except Exception:
                    coeff = 0.0
                if coeff != 0.0:
                    coeffs[name] = coeff

        # Constant term = evaluate with all symbols = 0
        const_val = 0.0
        if hasattr(lhs_expr, "free_symbols") and lhs_expr.free_symbols:
            subs_dict = {s: 0 for s in lhs_expr.free_symbols}
            const_val = float(lhs_expr.subs(subs_dict))
        else:
            const_val = float(lhs_expr)

        # Build Inequality struct
        vars_list = list(coeffs.keys())
        coeffs_list = [coeffs[v] for v in vars_list]

        # Shift constant: a*x + b*y + c <= d  => a*x + b*y <= d - c
        adjusted_rhs = rhs - const_val

        result = {
            "coefficients": coeffs_list,
            "vars": vars_list,
            "sense": sense,
            "rhs": adjusted_rhs,
        }
        print(json.dumps(result))

    except ImportError:
        print(json.dumps({"error": "sympy not installed; run: uv pip install sympy"}))
    except Exception as e:
        print(json.dumps({"error": f"parse failed: {e}"}))


def handle_check_parse(data: dict):
    """Just check if parsing is possible without returning full result."""
    expr = data.get("expr", "")
    try:
        from sympy.parsing.sympy_parser import parse_expr
        parse_expr(expr)
        print(json.dumps({"status": "ok", "parsable": True}))
    except Exception:
        print(json.dumps({"status": "ok", "parsable": False}))


if __name__ == "__main__":
    main()
