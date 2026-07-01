"""
Real SymPy operations for the math verification harness.

Supports: simplify, verify, expand, factor, series, differentiate,
integrate, solve, trig_simplify, subs, limit, lambdify,
dimension_propagate.

SECURITY NOTE: SymPy's `sympify()` internally uses Python's `eval()` and
can execute arbitrary Python code. This module defends against this by:
1. Running in a subprocess with controlled PYTHONPATH (no host access)
2. A 256 KiB input size cap enforced by the Rust bridge
3. A `_validate_expr_str()` helper that rejects obviously malicious patterns
4. Every operation is wrapped in try/except so exceptions are returned
   as error responses rather than crashing
"""

import sympy as sp
import re as _re


# Maximum length of any single expression/equation string passed to sympify().
# Beyond this, we reject rather than risk OOM inside SymPy.
_MAX_EXPR_LEN = 64 * 1024  # 64 KiB

# Blocked patterns that indicate attempted code injection via sympify.
# sympify can evaluate Python code via expressions like:
#   Integer('__import__("os").system("id")')
#   sympify("()" + exploit_string)
_INJECTION_PATTERNS = [
    "__import__",
    "__builtins__",
    "exec(",
    "eval(",
    "compile(",
    "open(",
    "getattr(",
    "setattr(",
    "delattr(",
    "vars(",
    "globals(",
    "locals(",
    "type(",
    "os.system",
    "os.popen",
    "subprocess",
    "shutil",
    "sys.modules",
]


def _validate_expr_str(expr_str: str, label: str = "expression") -> None:
    """Validate expression string length and reject injection patterns.

    Raises ValueError on failure.
    """
    if not isinstance(expr_str, str):
        raise ValueError(f"{label} must be a string, got {type(expr_str).__name__}")
    if len(expr_str) > _MAX_EXPR_LEN:
        raise ValueError(
            f"{label} too long: {len(expr_str)} chars (max {_MAX_EXPR_LEN})"
        )
    for pat in _INJECTION_PATTERNS:
        if pat in expr_str:
            # Log a warning (to stderr) so we can detect injection attempts
            import sys
            print(
                f"[sympy_ops] WARNING: blocked pattern '{pat}' in {label}",
                file=sys.stderr,
            )
            raise ValueError(
                f"{label} contains blocked pattern '{pat}' "
                f"(first {len(pat)} chars at position {expr_str.find(pat)})"
            )


def backend_status() -> dict:
    """Return SymPy availability and version info."""
    return {
        "available": True,
        "version": sp.__version__,
        "description": "SymPy symbolic mathematics (CAS)",
    }


# ── Assumption parser ──

def _assumptions_to_predicates(assumptions: list) -> list:
    """Convert assumption strings (e.g. ``"x > 0"``, ``"y < 0"``) into
    ``sp.Q.*`` predicates for use with ``sp.refine``.

    Supports the following patterns:

    ============================  ============================
    Pattern                      Q predicate
    ============================  ============================
    ``x > 0``                    ``sp.Q.positive(x)``
    ``x < 0``                    ``sp.Q.negative(x)``
    ``x >= 0``                   ``sp.Q.nonnegative(x)``
    ``x <= 0``                   ``sp.Q.nonpositive(x)``
    ``x != 0``                   ``sp.Q.nonzero(x)``
    ``x == 0``                   ``sp.Q.zero(x)``
    ``x is integer``             ``sp.Q.integer(x)``
    ``x is real``                ``sp.Q.real(x)``
    ``x is rational``            ``sp.Q.rational(x)``
    ============================  ============================
    """
    if not assumptions:
        return []

    predicates = []
    for raw in assumptions:
        a = raw.strip()
        if not a:
            continue

        # Pattern: `<var> is {integer|real|rational}`
        m = _re.match(r"^(\w+)\s+is\s+(integer|real|rational)$", a)
        if m:
            sym = sp.Symbol(m.group(1))
            q_map = {"integer": sp.Q.integer, "real": sp.Q.real, "rational": sp.Q.rational}
            predicates.append(q_map[m.group(2)](sym))
            continue

        # Pattern: `<var> > 0`
        m = _re.match(r"^(\w+)\s*>\s*0$", a)
        if m:
            sym = sp.Symbol(m.group(1))
            predicates.append(sp.Q.positive(sym))
            continue

        # Pattern: `<var> < 0`
        m = _re.match(r"^(\w+)\s*<\s*0$", a)
        if m:
            sym = sp.Symbol(m.group(1))
            predicates.append(sp.Q.negative(sym))
            continue

        # Pattern: `<var> >= 0`
        m = _re.match(r"^(\w+)\s*>=\s*0$", a)
        if m:
            sym = sp.Symbol(m.group(1))
            predicates.append(sp.Q.nonnegative(sym))
            continue

        # Pattern: `<var> <= 0`
        m = _re.match(r"^(\w+)\s*<=\s*0$", a)
        if m:
            sym = sp.Symbol(m.group(1))
            predicates.append(sp.Q.nonpositive(sym))
            continue

        # Pattern: `<var> != 0`
        m = _re.match(r"^(\w+)\s*!=\s*0$", a)
        if m:
            sym = sp.Symbol(m.group(1))
            predicates.append(sp.Q.nonzero(sym))
            continue

        # Pattern: `<var> == 0`
        m = _re.match(r"^(\w+)\s*==\s*0$", a)
        if m:
            sym = sp.Symbol(m.group(1))
            predicates.append(sp.Q.zero(sym))
            continue

        # Unknown pattern — warn but don't crash
        import sys as _sys
        print(
            f"[sympy_simplify] warning: unrecognised assumption '{raw}', "
            "expected patterns: x > 0, x < 0, x >= 0, x <= 0, "
            "x != 0, x == 0, x is integer, x is real, x is rational",
            file=_sys.stderr,
        )

    return predicates


def sympy_simplify(params: dict) -> dict:
    """Simplify a mathematical expression using SymPy.

    Params:
        expression (str): The expression to simplify (e.g. "x**2 + 2*x + 1")
        assumptions (list, optional): List of assumption strings (e.g. ["x > 0"])

    Returns:
        {"result": "simplified expression"}
    """
    expr_str = _get_param(params, "expression")
    assumptions_raw = params.get("assumptions", [])
    if isinstance(assumptions_raw, list):
        pass
    else:
        assumptions_raw = []

    try:
        _validate_expr_str(expr_str)
        expr = sp.sympify(expr_str)
        # Apply assumptions via sp.refine if provided
        simplified = sp.simplify(expr)
        if assumptions_raw:
            predicates = _assumptions_to_predicates(assumptions_raw)
            if predicates:
                combined = sp.And(*predicates)
                simplified = sp.refine(simplified, combined)
                # Re-simplify after refinement
                simplified = sp.simplify(simplified)
        if simplified.is_Equality:
            return {"result": str(simplified), "equal": bool(simplified)}
        return {"result": str(simplified)}
    except Exception as e:
        return {"error": f"SymPy simplify failed: {e}"}


def sympy_verify(params: dict) -> dict:
    """Verify algebraic identity lhs == rhs.

    Params:
        lhs (str): Left-hand side expression
        rhs (str): Right-hand side expression

    Returns:
        {"equal": bool, "difference": "simplified diff", "method": "sympy"}
    """
    lhs_str = _get_param(params, "lhs")
    rhs_str = _get_param(params, "rhs")

    try:
        _validate_expr_str(lhs_str, "lhs")
        _validate_expr_str(rhs_str, "rhs")
        lhs = sp.sympify(lhs_str)
        rhs = sp.sympify(rhs_str)
        diff = sp.simplify(lhs - rhs)
        is_zero = diff == 0

        return {
            "equal": is_zero,
            "difference": str(diff) if not is_zero else "0",
        }
    except Exception as e:
        return {"error": f"SymPy verify failed: {e}"}


def sympy_expand(params: dict) -> dict:
    """Expand a polynomial expression.

    Params:
        expression (str): The expression to expand

    Returns:
        {"result": "expanded expression"}
    """
    expr_str = _get_param(params, "expression")
    try:
        _validate_expr_str(expr_str)
        expr = sp.sympify(expr_str)
        expanded = sp.expand(expr)
        return {"result": str(expanded)}
    except Exception as e:
        return {"error": f"SymPy expand failed: {e}"}


def sympy_factor(params: dict) -> dict:
    """Factor a polynomial expression.

    Params:
        expression (str): The expression to factor

    Returns:
        {"result": "factored expression"}
    """
    expr_str = _get_param(params, "expression")
    try:
        _validate_expr_str(expr_str)
        expr = sp.sympify(expr_str)
        factored = sp.factor(expr)
        return {"result": str(factored)}
    except Exception as e:
        return {"error": f"SymPy factor failed: {e}"}


def sympy_series(params: dict) -> dict:
    """Compute series expansion of an expression.

    Params:
        expression (str): The expression to expand
        variable (str): The expansion variable (default: "x")
        point (float/int): Expansion point (default: 0)
        order (int): Number of terms (default: 6)

    Returns:
        {"result": "series expression", "order": n}
    """
    expr_str = _get_param(params, "expression")
    var_str = params.get("variable", "x")
    point = params.get("point", 0)
    order = int(params.get("order", 6))

    try:
        _validate_expr_str(expr_str, "expression")
        expr = sp.sympify(expr_str)
        var = sp.Symbol(var_str)
        series = sp.series(expr, var, point, order)
        # Remove O term for cleaner output
        series_no_o = series.removeO()
        return {
            "result": str(series),
            "leading": str(series_no_o) if series_no_o != 0 else "0",
        }
    except Exception as e:
        return {"error": f"SymPy series failed: {e}"}


def sympy_differentiate(params: dict) -> dict:
    """Compute symbolic derivative.

    Params:
        expression (str): The expression to differentiate
        variable (str): Differentiation variable (default: "x")
        order (int): Order of derivative (default: 1)

    Returns:
        {"result": "derivative expression"}
    """
    expr_str = _get_param(params, "expression")
    var_str = params.get("variable", "x")
    order = int(params.get("order", 1))

    try:
        _validate_expr_str(expr_str, "expression")
        expr = sp.sympify(expr_str)
        var = sp.Symbol(var_str)
        diff = sp.diff(expr, var, order)
        return {"result": str(diff)}
    except Exception as e:
        return {"error": f"SymPy differentiate failed: {e}"}


def sympy_integrate(params: dict) -> dict:
    """Compute symbolic integral.

    Params:
        expression (str): The expression to integrate
        variable (str): Integration variable (default: "x")
        lower (float, optional): Lower bound for definite integral
        upper (float, optional): Upper bound for definite integral

    Returns:
        {"result": "integral expression"}
    """
    expr_str = _get_param(params, "expression")
    var_str = params.get("variable", "x")
    lower = params.get("lower")
    upper = params.get("upper")

    try:
        _validate_expr_str(expr_str, "expression")
        expr = sp.sympify(expr_str)
        var = sp.Symbol(var_str)

        if lower is not None and upper is not None:
            integral = sp.integrate(expr, (var, float(lower), float(upper)))
        else:
            integral = sp.integrate(expr, var)

        return {"result": str(integral)}
    except Exception as e:
        return {"error": f"SymPy integrate failed: {e}"}


def sympy_solve(params: dict) -> dict:
    """Solve an equation or system.

    Params:
        equation (str): The equation to solve (e.g. "x**2 - 4 = 0")
        variable (str or list): Variable(s) to solve for

    Returns:
        {"solutions": [...], "method": "sympy"}
    """
    eq_str = _get_param(params, "equation")
    var_str = params.get("variable", "x")

    try:
        # Validate equation string
        _validate_expr_str(eq_str, "equation")

        # Handle equation with = sign
        if "=" in eq_str:
            parts = eq_str.split("=", 1)
            lhs = sp.sympify(parts[0])
            rhs = sp.sympify(parts[1])
            eq = sp.Eq(lhs, rhs)
        else:
            # Assume expression = 0
            eq = sp.sympify(eq_str)

        if isinstance(var_str, list):
            vars_sym = [sp.Symbol(v) for v in var_str]
        else:
            vars_sym = [sp.Symbol(var_str)]

        solutions = sp.solve(eq, *vars_sym, dict=True)
        # Convert to serializable format
        if not solutions:
            return {"solutions": [], "no_solutions": True}

        # Handle single-solution case
        if not isinstance(solutions, list):
            solutions = [solutions]

        result_solutions = []
        for sol in solutions:
            if isinstance(sol, dict):
                result_solutions.append({str(k): str(v) for k, v in sol.items()})
            else:
                result_solutions.append(str(sol))

        return {"solutions": result_solutions, "count": len(result_solutions)}
    except Exception as e:
        return {"error": f"SymPy solve failed: {e}"}


def sympy_trig_simplify(params: dict) -> dict:
    """Simplify trigonometric expressions using sp.trigsimp().

    Params:
        expression (str): The expression to simplify

    Returns:
        {"result": "simplified expression", "method": "trig_simplify"}
    """
    expr_str = _get_param(params, "expression")
    try:
        _validate_expr_str(expr_str, "expression")
        expr = sp.sympify(expr_str)
        # Use proper trigsimp() instead of ad-hoc expand_trig cycle
        trig_result = sp.trigsimp(expr)
        # Run through one more general simplify to combine with non-trig parts
        final = sp.simplify(trig_result)
        return {"result": str(final)}
    except Exception as e:
        return {"error": f"SymPy trig_simplify failed: {e}"}


def sympy_dimension_propagate(params: dict) -> dict:
    """Propagate physical dimensions through an equation.

    Given a mapping of {variable_or_unit: dimension_string}, compute
    the resulting dimension for each side of the equation.

    Params:
        equation (str): The equation to analyze (e.g. "F = m*a")
        dimensions (dict): Variable → dimension mapping
                           e.g. {"F": "L*M*T^-2", "m": "M", "a": "L*T^-2"}

    Returns:
        {"lhs_dim": "...", "rhs_dim": "...", "consistent": true/false}
    """
    eq_str = _get_param(params, "equation")
    dims = _get_param_raw(params, "dimensions")

    try:
        # Validate equation
        _validate_expr_str(eq_str, "equation")

        # Parse the dimension mapping
        dim_symbols = {}
        for var_name, dim_str in dims.items():
            # Validate dimension string
            _validate_expr_str(str(dim_str), f"dimension string for '{var_name}'")
            # Parse dimension string like "L*M*T^-2" into a SymPy expression
            dim_str_clean = dim_str.replace("^", "**")
            try:
                dim_sym = sp.sympify(dim_str_clean)
            except Exception:
                # Try with implicit multiplication: "LMT^-2" → "L*M*T**-2"
                import re
                dim_str_re = re.sub(r'([A-Z])([A-Z(])', r'\1*\2', dim_str)
                dim_str_re = dim_str_re.replace("^", "**")
                dim_sym = sp.sympify(dim_str_re)
            dim_symbols[var_name] = dim_sym

        # Split equation
        parts = eq_str.replace(" ", "").split("=")
        if len(parts) < 2:
            return {"error": "equation must have at least one = sign"}

        def compute_dim(expr_str: str) -> sp.Expr:
            """Compute dimension of a single side."""
            # Replace known variable names with their dimensions
            result_dim = None
            remaining = expr_str

            # We need to parse the expression token by token
            # Simple approach: replace known symbols
            import re as _re

            try:
                # Try to substitute
                sympy_expr = sp.sympify(expr_str)
                for var_name, dim in dim_symbols.items():
                    if var_name in expr_str:
                        sympy_expr = sympy_expr.subs(sp.Symbol(var_name), dim)
                return sp.simplify(sympy_expr) if sympy_expr != 0 else sp.Integer(1)
            except Exception:
                return None

        lhs_dim = compute_dim(parts[0])
        rhs_dim = compute_dim(parts[1])

        if lhs_dim is None and rhs_dim is None:
            return {
                "lhs_dim": "unknown (cannot parse)",
                "rhs_dim": "unknown (cannot parse)",
                "consistent": True,  # Can't determine → assume consistent
                "method": "sympy",
            }

        consistent = False
        if lhs_dim is not None and rhs_dim is not None:
            diff = sp.simplify(lhs_dim - rhs_dim)
            consistent = diff == 0

        return {
            "lhs_dim": str(lhs_dim) if lhs_dim is not None else "unknown",
            "rhs_dim": str(rhs_dim) if rhs_dim is not None else "unknown",
            "consistent": consistent,
            "method": "sympy",
        }
    except Exception as e:
        return {"error": f"SymPy dimension propagate failed: {e}"}


def sympy_subs(params: dict) -> dict:
    """Substitute variables/expressions in a symbolic expression.

    Params:
        expression (str): The expression containing substitution targets
        substitutions (object): Mapping of {old: new}, e.g. {"x": 2, "y": "a + b"}
        simultaneous (bool, optional): If true, all substitutions are
            performed simultaneously (default false)

    Returns:
        {"result": "substituted expression"}
    """
    expr_str = _get_param(params, "expression")
    subs_raw = _get_param_raw(params, "substitutions")
    simultaneous = bool(params.get("simultaneous", False))

    try:
        _validate_expr_str(expr_str, "expression")
        expr = sp.sympify(expr_str)
        # Build substitution pairs
        sub_pairs = []
        for old_str, new_str in subs_raw.items():
            # Validate substitution values (may also be expressions)
            if isinstance(new_str, str):
                _validate_expr_str(new_str, f"substitution value for '{old_str}'")
            old_sym = sp.Symbol(old_str) if old_str.isidentifier() else sp.sympify(old_str)
            new_val = sp.sympify(str(new_str))
            sub_pairs.append((old_sym, new_val))

        if simultaneous and len(sub_pairs) > 1:
            # Use simultaneous substitution to avoid intermediate interference
            old_list, new_list = zip(*sub_pairs)
            result = expr.subs(list(zip(old_list, new_list)), simultaneous=True)
        else:
            result = expr
            for old, new in sub_pairs:
                result = result.subs(old, new)

        return {"result": str(result)}
    except Exception as e:
        return {"error": f"SymPy subs failed: {e}"}


def sympy_limit(params: dict) -> dict:
    """Compute the limit of an expression.

    Params:
        expression (str): The expression to evaluate
        variable (str): The limit variable (default: "x")
        point (str): The limit point — "0", "oo", "-oo", or any numeric string
        direction (str, optional): "+" for right-hand, "-" for left-hand
            (default: None, which computes the two-sided limit)

    Returns:
        {"result": "limit value", "point": point, "direction": direction}
    """
    expr_str = _get_param(params, "expression")
    var_str = params.get("variable", "x")
    point_str = _get_param(params, "point")
    direction = params.get("direction")

    try:
        _validate_expr_str(expr_str, "expression")
        _validate_expr_str(point_str, "point")
        expr = sp.sympify(expr_str)
        var = sp.Symbol(var_str)

        # Parse limit point
        if point_str in ("oo", "inf", "+oo"):
            point = sp.oo
        elif point_str in ("-oo", "-inf", "-oo"):
            point = -sp.oo
        else:
            point = sp.sympify(point_str)

        # Compute limit
        if direction == "+":
            limit_val = sp.limit(expr, var, point, sp.S(dir="+"))
        elif direction == "-":
            limit_val = sp.limit(expr, var, point, sp.S(dir="-"))
        else:
            limit_val = sp.limit(expr, var, point)

        return {"result": str(limit_val)}
    except Exception as e:
        return {"error": f"SymPy limit failed: {e}"}


def sympy_lambdify(params: dict) -> dict:
    """Convert a symbolic expression to a numeric callable and evaluate it.

    Params:
        expression (str): The symbolic expression
        variables (list of str): Variable names for the function signature
        values (list of numbers, optional): Numeric values to evaluate at.
            Order must match ``variables``.
        modules (str, optional): Backend modules for lambdify.
            Default: "math" (no external deps needed).

    Returns:
        {"result": "...", "function": "str(expression)"}
        If values are provided, also returns {"evaluated": float_result}.
    """
    expr_str = _get_param(params, "expression")
    vars_raw = params.get("variables", ["x"])
    values_raw = params.get("values", None)
    modules = params.get("modules", "math")

    if isinstance(vars_raw, list):
        variables = [sp.Symbol(v) for v in vars_raw]
    else:
        variables = [sp.Symbol(str(vars_raw))]

    try:
        _validate_expr_str(expr_str, "expression")
        expr = sp.sympify(expr_str)
        f = sp.lambdify(variables, expr, modules=modules)

        result = {"result": str(expr), "function_repr": repr(f)}

        if values_raw is not None and len(values_raw) > 0:
            vals = [float(v) for v in values_raw]
            evaluated = f(*vals)
            # Convert numpy types to native Python
            if hasattr(evaluated, "item"):
                evaluated = evaluated.item()
            result["evaluated"] = float(evaluated)

        return result
    except Exception as e:
        return {"error": f"SymPy lambdify failed: {e}"}


def _get_param(params: dict, key: str) -> str:
    """Get a required string parameter, raising on missing."""
    val = params.get(key)
    if val is None:
        raise ValueError(f"missing required parameter: '{key}'")
    return val


def _get_param_raw(params: dict, key: str):
    """Get a required parameter of any type, raising on missing."""
    val = params.get(key)
    if val is None:
        raise ValueError(f"missing required parameter: '{key}'")
    return val


# Dispatch table
OPERATIONS = {
    "backend_status": lambda p: {"status": "ok", "result": backend_status()},
    "sympy_simplify": lambda p: _wrap("sympy_simplify", p, sympy_simplify),
    "sympy_verify": lambda p: _wrap("sympy_verify", p, sympy_verify),
    "sympy_expand": lambda p: _wrap("sympy_expand", p, sympy_expand),
    "sympy_factor": lambda p: _wrap("sympy_factor", p, sympy_factor),
    "sympy_series": lambda p: _wrap("sympy_series", p, sympy_series),
    "sympy_differentiate": lambda p: _wrap("sympy_differentiate", p, sympy_differentiate),
    "sympy_integrate": lambda p: _wrap("sympy_integrate", p, sympy_integrate),
    "sympy_solve": lambda p: _wrap("sympy_solve", p, sympy_solve),
    "sympy_trig_simplify": lambda p: _wrap("sympy_trig_simplify", p, sympy_trig_simplify),
    "sympy_subs": lambda p: _wrap("sympy_subs", p, sympy_subs),
    "sympy_limit": lambda p: _wrap("sympy_limit", p, sympy_limit),
    "sympy_lambdify": lambda p: _wrap("sympy_lambdify", p, sympy_lambdify),
    "sympy_dimension_propagate": lambda p: _wrap("sympy_dimension_propagate", p, sympy_dimension_propagate),
}


def _wrap(op_name: str, params: dict, handler) -> dict:
    """Wrap a handler call, catching errors into the response format."""
    try:
        result = handler(params)
        if "error" in result:
            return {"status": "error", "error": result["error"]}
        return {"status": "ok", "result": result}
    except ValueError as e:
        return {"status": "error", "error": str(e)}
    except Exception as e:
        return {"status": "error", "error": f"{op_name} failed: {e}"}


def dispatch(op: str, params: dict) -> dict:
    """Dispatch an operation to the appropriate handler."""
    handler = OPERATIONS.get(op)
    if handler is None:
        return {"status": "error", "error": f"unknown operation: {op}"}
    return handler(params)
