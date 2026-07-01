"""
Real Z3 operations for the math verification harness.

Supports: check_inequality, check_system, optimize, prove,
solver_push, solver_pop, solver_add, solver_check, backends_status.
"""

import sys
import z3
import json
import ast as _ast
import re as _re

# Increase recursion limit for deeply nested expressions (e.g., x+x+...+x
# with 10,000 terms produces a 10,000-node AST that would hit the default
# ~1000 limit). This is safe because Z3's AST builder and Python's ast.walk
# are the only deeply recursive paths here.
sys.setrecursionlimit(max(sys.getrecursionlimit(), 10000))


def backend_status() -> dict:
    """Return Z3 availability and version info."""
    return {
        "available": True,
        "version": z3.get_version_string(),
        "description": "Z3 SMT solver (Microsoft Research)",
    }


# ===========================================================================
# Global persistent solver for push/pop incremental solving
# ===========================================================================

_SOLVER = None


def _get_persistent_solver():
    """Get or create the module-level persistent Z3 solver."""
    global _SOLVER
    if _SOLVER is None:
        _SOLVER = z3.Solver()
    return _SOLVER


def _reset_persistent_solver():
    """Reset the persistent solver (clear all constraints and contexts)."""
    global _SOLVER
    _SOLVER = z3.Solver()


# ===========================================================================
# Sort inference
# ===========================================================================


def _normalize_expression(expr_str: str) -> str:
    """Normalize operator syntax: ^ → **, AND → and, etc."""
    expr = expr_str.replace("^", "**")
    expr = _re.sub(r'\bAND\b', 'and', expr)
    expr = _re.sub(r'\bOR\b', 'or', expr)
    expr = _re.sub(r'\bNOT\b', 'not', expr)
    return expr.strip()


def _infer_sort_mode(expr_str: str) -> str:
    """Infer the Z3 sort mode from an expression string.

    Returns one of:
      - 'real'  → use z3.Real() for variables, z3.RealVal() for constants
      - 'int'   → use z3.Int() for variables, z3.IntVal() for integer constants
      - 'bool'  → use z3.Bool() for variables

    Detection rules:
      - If the expression contains '%' (Mod) or '//' (FloorDiv) → 'int'
        (unless also contains '/' or float literals → 'real' overrides)
      - If the expression contains only boolean operators (and, or, not,
        Implies) and comparisons, with no arithmetic → 'bool'
      - Otherwise → 'real' (default, backward compatible)
    """
    expr = expr_str.strip()

    # Quick heuristic check for boolean-only expressions
    # Only contains: and, or, not, Implies, ==, !=, <=, >=, <, >, True, False,
    # parentheses, variables, and whitespace
    # Does NOT contain: +, -, *, /, **, ^, sin, cos, sqrt, abs, exp, log, ln

    # Check for arithmetic operators (excluding those inside comparison chains)
    # We look at the top-level presence of +, -, *, / (not inside quotes)
    has_plus = '+' in expr
    has_minus = '-' in expr
    has_star = '*' in expr and '**' not in expr
    # Actually handle ** carefully - it's arithmetic too
    has_pow = '**' in expr or '^' in expr
    has_div = '/' in expr
    has_mod = '%' in expr
    has_floordiv = '//' in expr

    has_arithmetic = has_plus or has_minus or has_star or has_pow or has_div
    has_int_operator = has_mod or has_floordiv
    has_float_literal = '.' in _re.sub(r'\d+\.\d*(?:[eE][+-]?\d+)?', '.', expr) and '.' in expr

    # If expression has int operators (% or //) but no division or float literals → int
    if has_int_operator and not has_div and not has_float_literal:
        return 'int'

    # If expression has no arithmetic operators at all → could be boolean-only
    if not has_arithmetic and not has_int_operator:
        # Check for boolean keywords
        has_bool_keywords = any(
            kw in expr.lower().replace(' ', '')
            for kw in ['and', 'or', 'not', 'implies']
        )
        # A pure boolean expression would have and/or/not/Implies
        # with comparisons (==, !=, <, >, <=, >=) but no arithmetic
        if has_bool_keywords:
            return 'bool'

    # Default: real (backward compatible)
    return 'real'


def _infer_sorts(expr_str: str, variables: list) -> dict:
    """Infer individual variable sorts from expression analysis.

    Returns a dict mapping var_name → 'Real' | 'Int' | 'Bool'.

    Uses AST analysis for precise detection:
      - Variables used in ast.Mod or ast.FloorDiv → 'Int'
      - Variables used in ast.Div, with float constants, or in
        real-valued functions (sin, cos, sqrt) → 'Real' (overrides Int)
      - Variables used only in boolean context → 'Bool' (if not in arithmetic)
      - Otherwise → 'Real' (default, backward compatible)

    Note: Pure comparisons (>, <, ==, etc.) do NOT force Real — they work
    with both Int and Real. Only division and float literals force Real.
    """
    # Default all to Real
    sorts = {v: 'Real' for v in variables}

    if not variables:
        return sorts

    # Normalize before AST parsing (AND → and, etc.)
    normalized = _normalize_expression(expr_str)

    try:
        tree = _ast.parse(normalized, mode='eval')
    except SyntaxError:
        return sorts

    # Collect context per variable
    in_int_op = set()       # used in % or //
    in_real_op = set()      # used in / or float contexts
    in_boolean = set()      # used in and/or/not
    in_compare = set()      # used in comparisons (>, <, ==, etc.)

    for node in _ast.walk(tree):
        if isinstance(node, _ast.BinOp):
            names = {n.id for n in _ast.walk(node) if isinstance(n, _ast.Name)}
            if isinstance(node.op, (_ast.Mod, _ast.FloorDiv)):
                in_int_op.update(names)
            elif isinstance(node.op, _ast.Div):
                in_real_op.update(names)

        if isinstance(node, _ast.UnaryOp):
            names = {n.id for n in _ast.walk(node) if isinstance(n, _ast.Name)}
            if isinstance(node.op, _ast.Not):
                in_boolean.update(names)

        if isinstance(node, _ast.Compare):
            names = {n.id for n in _ast.walk(node) if isinstance(n, _ast.Name)}
            in_compare.update(names)
            # Check for float constants — those force Real
            for child in _ast.walk(node):
                if isinstance(child, _ast.Constant) and isinstance(child.value, float):
                    in_real_op.update(names)

        if isinstance(node, _ast.BoolOp):
            names = {n.id for n in _ast.walk(node) if isinstance(n, _ast.Name)}
            in_boolean.update(names)

        if isinstance(node, _ast.Call):
            func_name = node.func.id if isinstance(node.func, _ast.Name) else None
            if func_name in ('sin', 'cos', 'sqrt', 'abs', 'exp', 'log', 'ln'):
                names = {n.id for n in _ast.walk(node) if isinstance(n, _ast.Name)}
                in_real_op.update(names)
            elif func_name in ('And', 'Or', 'Not', 'Implies'):
                names = {n.id for n in _ast.walk(node) if isinstance(n, _ast.Name)}
                in_boolean.update(names)

        if isinstance(node, _ast.Constant) and isinstance(node.value, float):
            # A float literal propagates Real to surrounding name nodes
            for n in _ast.walk(tree):
                if isinstance(n, _ast.Name):
                    in_real_op.add(n.id)

        if isinstance(node, _ast.Pow):
            # Power with non-integer exponent forces Real
            # We can't determine the exponent statically in all cases,
            # so we conservatively don't force Real here.
            # The expression builder handles non-integer exponent errors.
            pass

    # Determine final sorts (Real > Int > Bool > default)
    for v in variables:
        if v in in_real_op:
            sorts[v] = 'Real'
        elif v in in_int_op:
            sorts[v] = 'Int'
        elif v in in_boolean and v not in in_compare:
            sorts[v] = 'Bool'
        # Variables in comparisons only retain their default (Real)
        # This is backward compatible and comparisons work for both types.
        # The merging logic in check_system/optimize can narrow to Int
        # if another constraint forces it.

    return sorts


# ===========================================================================
# Z3 variable creation with sort awareness
# ===========================================================================


def _create_z3_vars(variables: list, sorts: dict = None) -> dict:
    """Create Z3 variables with appropriate sorts.

    Args:
        variables: List of variable name strings
        sorts: Optional dict of var_name → 'Real' | 'Int' | 'Bool'
               If None, all variables are created as Real.

    Returns:
        Dict mapping var_name → Z3 variable
    """
    if sorts is None:
        sorts = {}
    z3_vars = {}
    for v in variables:
        sort = sorts.get(v, 'Real')
        if sort == 'Int':
            z3_vars[v] = z3.Int(v)
        elif sort == 'Bool':
            z3_vars[v] = z3.Bool(v)
        else:
            z3_vars[v] = z3.Real(v)
    return z3_vars


def _parse_python_expr(expr_str: str, variables: list, sorts: dict = None) -> tuple:
    """Parse a Python-style math expression into a Z3 formula.

    Handles: +, -, *, /, **, ^, ==, !=, <=, <, >=, >, %, //, sin, cos,
    sqrt, abs, And, Or, Not, Implies.

    Note: exp, log, and ln are NOT supported by Z3 and will raise ValueError.

    Args:
        expr_str: The expression string
        variables: List of variable names
        sorts: Optional dict mapping var_name -> 'Real'|'Int'|'Bool'
               If None, sorts are inferred from the expression.

    Returns:
        (z3_formula, z3_vars_dict)
    """
    # Infer sorts if not provided
    if sorts is None:
        sorts = _infer_sorts(expr_str, variables)

    # Create Z3 variables with inferred sorts
    z3_vars = _create_z3_vars(variables, sorts)

    # Parse expression via _safe_build_z3
    return _safe_build_z3(expr_str, z3_vars, sorts)


def _safe_build_z3(expr_str: str, z3_vars: dict, sorts: dict = None) -> tuple:
    """Safely convert a Python math expression to a Z3 formula.

    Uses a restricted AST evaluation approach with Z3 operations.
    Directly parses Python syntax — no marker preprocessing needed.

    Handles: +, -, *, /, **, ^, ==, !=, <=, <, >=, >, %, //, (), sin, cos,
    sqrt, abs, and, or, not, Implies.

    Note: exp, log, and ln are NOT supported by Z3 and will raise ValueError.

    Args:
        expr_str: The expression string
        z3_vars: Dict mapping variable names to Z3 variables
        sorts: Optional dict of var_name -> 'Real'|'Int'|'Bool' for constant handling

    Returns:
        (z3_formula, z3_vars_dict)
    """
    if sorts is None:
        sorts = {}

    # Normalize operator syntax
    expr = _normalize_expression(expr_str)

    def _is_var_int(var_name: str) -> bool:
        return sorts.get(var_name, 'Real') == 'Int'

    def build(node):
        """Recursively build Z3 expression from AST node."""
        if isinstance(node, _ast.Expression):
            return build(node.body)

        if isinstance(node, _ast.Constant):
            if isinstance(node.value, (int, float)):
                # Use IntVal for integer contexts, RealVal otherwise
                if isinstance(node.value, int):
                    # Check if we're in an overall integer context
                    # Use variable sort as a heuristic
                    return z3.IntVal(node.value)
                else:
                    return z3.RealVal(node.value)
            elif node.value is True:
                return z3.BoolVal(True)
            elif node.value is False:
                return z3.BoolVal(False)
            return node.value

        if isinstance(node, _ast.Name):
            name = node.id
            if name in z3_vars:
                return z3_vars[name]
            float_map = {"pi": 3.141592653589793, "e": 2.718281828459045}
            if name in float_map:
                return z3.RealVal(float_map[name])
            # Constants True/False
            if name == "True":
                return z3.BoolVal(True)
            if name == "False":
                return z3.BoolVal(False)
            try:
                return z3.RealVal(float(name))
            except (ValueError, TypeError):
                pass
            raise ValueError(f"Unknown variable: {name}")

        if isinstance(node, _ast.UnaryOp):
            if isinstance(node.op, _ast.USub):
                return -build(node.operand)
            if isinstance(node.op, _ast.UAdd):
                return build(node.operand)
            if isinstance(node.op, _ast.Not):
                return z3.Not(build(node.operand))
            raise ValueError(f"Unsupported unary op: {type(node.op).__name__}")

        if isinstance(node, _ast.BinOp):
            left = build(node.left)
            right = build(node.right)
            op_type = type(node.op)
            if op_type == _ast.Add:
                return left + right
            if op_type == _ast.Sub:
                return left - right
            if op_type == _ast.Mult:
                return left * right
            if op_type == _ast.Div:
                return left / right
            if op_type == _ast.Mod:
                return left % right
            if op_type == _ast.FloorDiv:
                return left / right
            if op_type == _ast.Pow:
                if isinstance(right, z3.RatNumRef):
                    try:
                        exp_val = float(str(right))
                        if exp_val == int(exp_val):
                            n = int(exp_val)
                            if n == 0:
                                return z3.IntVal(1) if any(
                                    _is_var_int(v) for v in z3_vars
                                ) else z3.RealVal(1)
                            if n == 1:
                                return left
                            result = left
                            for _ in range(n - 1):
                                result = result * left
                            return result
                    except (ValueError, TypeError):
                        pass
                # Non-integer or variable exponent: use Z3 native power operator
                return left ** right
            raise ValueError(f"Unsupported binary op: {type(node.op).__name__}")

        if isinstance(node, _ast.Compare):
            left = build(node.left)
            comparators = [build(c) for c in node.comparators]
            if len(node.ops) == 1 and len(comparators) == 1:
                op_type = type(node.ops[0])
                right = comparators[0]
                if op_type == _ast.Eq:
                    return left == right
                if op_type == _ast.NotEq:
                    return left != right
                if op_type == _ast.Lt:
                    return left < right
                if op_type == _ast.LtE:
                    return left <= right
                if op_type == _ast.Gt:
                    return left > right
                if op_type == _ast.GtE:
                    return left >= right
                raise ValueError(
                    f"Unsupported compare op: {type(node.ops[0]).__name__}"
                )
            result = None
            for i in range(len(node.ops)):
                op_type = type(node.ops[i])
                right = comparators[i]
                if op_type == _ast.Lt:
                    clause = left < right
                elif op_type == _ast.LtE:
                    clause = left <= right
                elif op_type == _ast.Gt:
                    clause = left > right
                elif op_type == _ast.GtE:
                    clause = left >= right
                elif op_type == _ast.Eq:
                    clause = left == right
                elif op_type == _ast.NotEq:
                    clause = left != right
                else:
                    raise ValueError(
                        f"Unsupported compare op in chain: {type(node.ops[i]).__name__}"
                    )
                result = z3.And(result, clause) if result is not None else clause
                left = right
            return result

        if isinstance(node, _ast.BoolOp):
            values = [build(v) for v in node.values]
            if isinstance(node.op, _ast.And):
                return z3.And(*values)
            if isinstance(node.op, _ast.Or):
                return z3.Or(*values)
            raise ValueError(f"Unsupported boolean op: {type(node.op).__name__}")

        if isinstance(node, _ast.Call):
            func_name = node.func.id if isinstance(node.func, _ast.Name) else None
            args = [build(arg) for arg in node.args]
            if func_name == "sin":
                return z3.Sin(*args)
            if func_name == "cos":
                return z3.Cos(*args)
            if func_name == "sqrt":
                return args[0] ** 0.5
            if func_name == "abs":
                return z3.If(args[0] >= 0, args[0], -args[0])
            if func_name == "exp":
                raise ValueError(
                    "exp is not supported by Z3. "
                    "Consider using SymPy or Reformulating as a constraint."
                )
            if func_name in ("log", "ln"):
                raise ValueError(
                    f"{func_name} is not supported by Z3. "
                    "Consider using SymPy or Reformulating as a constraint."
                )
            if func_name == "And" and len(args) >= 2:
                return z3.And(*args)
            if func_name == "Or" and len(args) >= 2:
                return z3.Or(*args)
            if func_name == "Not":
                return z3.Not(*args)
            if func_name == "Implies" and len(args) >= 2:
                return z3.Implies(*args)
            raise ValueError(f"Unknown function: {func_name}")

        raise ValueError(f"Unsupported AST node type: {type(node).__name__}")

    try:
        parsed = _ast.parse(expr, mode="eval")
        formula = build(parsed.body)
        return formula, z3_vars
    except SyntaxError as e:
        raise ValueError(f"Syntax error in '{expr_str}': {e}")
    except Exception as e:
        raise ValueError(f"Z3 parse error for '{expr_str}': {e}")


def _extract_variables(expr_str: str) -> list:
    """Extract variable names from an expression string.

    Returns sorted list of unique variable names, excluding numbers
    and known function names. Normalizes case for logical keywords
    (AND → And, etc.) before filtering.
    """
    # Normalize case for logical keywords
    normalized = _normalize_expression(expr_str)

    # Find all alphabetic identifiers
    identifiers = set(_re.findall(r'[a-zA-Z_][a-zA-Z0-9_]*', normalized))

    # Filter out known names
    known_names = {
        "sin", "cos", "tan", "sqrt", "abs", "exp", "log", "ln",
        "And", "Or", "Not", "Implies", "True", "False",
        "and", "or", "not", "implies", "true", "false",
        "pi", "e", "x", "y", "z",
    }
    # Keep x, y, z if they appear (they might be variables)
    return sorted(
        identifiers - known_names
        | {n for n in identifiers if n in {"x", "y", "z", "t", "n", "k", "m", "i", "j", "a", "b", "c", "d", "u", "v", "w"}}
    )


# ===========================================================================
# Operation implementations
# ===========================================================================


def _model_to_dict(model, z3_vars: dict) -> dict:
    """Convert a Z3 model to a serializable dict."""
    model_dict = {}
    for var_name, var_z3 in z3_vars.items():
        val = model.eval(var_z3, model_completion=True)
        if val is not None:
            try:
                model_dict[var_name] = float(str(val))
            except (ValueError, TypeError):
                try:
                    model_dict[var_name] = int(str(val))
                except (ValueError, TypeError):
                    model_dict[var_name] = str(val)
    return model_dict


def _has_real_ops(expr_str: str) -> bool:
    """Check if an expression truly requires Real sort.

    Returns True if the expression contains division, float literals,
    or real-valued functions (sin, cos, sqrt, abs).
    """
    try:
        tree = _ast.parse(_normalize_expression(expr_str), mode='eval')
    except SyntaxError:
        return False

    for node in _ast.walk(tree):
        if isinstance(node, _ast.BinOp) and isinstance(node.op, (_ast.Div, _ast.Pow)):
            # Division always forces Real
            if isinstance(node.op, _ast.Div):
                return True
        if isinstance(node, _ast.Constant) and isinstance(node.value, float):
            return True
        if isinstance(node, _ast.Call):
            func_name = node.func.id if isinstance(node.func, _ast.Name) else ''
            if func_name in ('sin', 'cos', 'sqrt', 'abs'):
                return True
    return False


def z3_check(params: dict) -> dict:
    """Check a single inequality expression using Z3 SMT solver.

    Params:
        expression (str): The inequality expression (e.g. "x**2 + y**2 <= 1")
        variables (list, optional): List of variable names

    Returns:
        {"result": "sat"/"unsat"/"unknown", "model": {...} if sat}
    """
    expr_str = _get_param(params, "expression")
    variables = params.get("variables")
    timeout = int(params.get("timeout_ms", 5000))

    try:
        # Auto-detect variables if not provided
        if not variables:
            variables = _extract_variables(expr_str)

        # Infer sorts and parse
        sorts = _infer_sorts(expr_str, variables)
        formula, z3_vars = _parse_python_expr(expr_str, variables, sorts)

        solver = z3.Solver()
        solver.set("timeout", timeout)
        solver.add(formula)

        result = solver.check()
        status = str(result)

        response = {"result": status}

        if result == z3.sat:
            model = solver.model()
            response["model"] = _model_to_dict(model, z3_vars)

        return response
    except Exception as e:
        return {"error": f"Z3 check failed: {e}"}


def z3_check_system(params: dict) -> dict:
    """Check a system of constraints using Z3.

    Params:
        constraints (list): List of constraint expression strings
        variables (list, optional): List of variable names

    Returns:
        {"result": "sat"/"unsat"/"unknown", "model": {var: val, ...}}
    """
    constraints = _get_param(params, "constraints")
    if isinstance(constraints, str):
        constraints = json.loads(constraints)
    variables = params.get("variables")
    timeout = int(params.get("timeout_ms", 10000))

    try:
        # Auto-detect variables
        all_vars_set = set()
        if variables:
            all_vars_set = set(variables)
        else:
            for c in constraints:
                all_vars_set.update(_extract_variables(c))

        variables = sorted(all_vars_set) if all_vars_set else ["x"]

        solver = z3.Solver()
        solver.set("timeout", timeout)

        # Collect sorts across all constraints
        all_sorts = {}
        for c in constraints:
            c_sorts = _infer_sorts(c, variables)
            c_has_real = _has_real_ops(c)
            for v, s in c_sorts.items():
                existing = all_sorts.get(v)
                if existing is None:
                    all_sorts[v] = s
                else:
                    # Real only overrides Int if the expression truly
                    # requires Real (division, float literals, sin, cos)
                    if s == 'Real' and existing == 'Int' and not c_has_real:
                        continue  # Keep Int — comparison alone doesn't force Real
                    if s == 'Real' or existing == 'Real':
                        all_sorts[v] = 'Real'
                    elif s == 'Int' or existing == 'Int':
                        all_sorts[v] = 'Int'

        z3_vars = _create_z3_vars(variables, all_sorts)

        for c in constraints:
            formula, _ = _parse_python_expr(c, variables, all_sorts)
            solver.add(formula)

        result = solver.check()
        status = str(result)

        response = {"result": status}

        if result == z3.sat:
            model = solver.model()
            response["model"] = _model_to_dict(model, z3_vars)

        return response
    except Exception as e:
        return {"error": f"Z3 check system failed: {e}"}


def z3_optimize(params: dict) -> dict:
    """Optimize an objective function subject to constraints.

    Params:
        objective (str): The objective function to optimize (e.g. "x + y")
        constraints (list): List of constraint expression strings
        variables (list, optional): List of variable names
        direction (str): "minimize" or "maximize" (default: "maximize")

    Returns:
        {"result": "sat"/"unsat", "optimum": value, "model": {var: val, ...}}
    """
    objective_str = _get_param(params, "objective")
    constraints = _get_param(params, "constraints")
    if isinstance(constraints, str):
        constraints = json.loads(constraints)
    variables = params.get("variables")
    direction = params.get("direction", "maximize")

    try:
        # Collect all variables
        all_vars_set = set()
        if variables:
            all_vars_set = set(variables)
        else:
            all_vars_set.update(_extract_variables(objective_str))
            for c in constraints:
                all_vars_set.update(_extract_variables(c))

        variables = sorted(all_vars_set) if all_vars_set else ["x"]

        # Collect sorts
        all_sorts = _infer_sorts(objective_str, variables)
        for c in constraints:
            c_sorts = _infer_sorts(c, variables)
            c_has_real = _has_real_ops(c)
            for v, s in c_sorts.items():
                existing = all_sorts.get(v)
                if existing is None:
                    all_sorts[v] = s
                else:
                    # Real only overrides Int if the expression truly
                    # requires Real (division, float literals, sin, cos)
                    if s == 'Real' and existing == 'Int' and not c_has_real:
                        continue
                    if s == 'Real' or existing == 'Real':
                        all_sorts[v] = 'Real'
                    elif s == 'Int' or existing == 'Int':
                        all_sorts[v] = 'Int'

        z3_vars = _create_z3_vars(variables, all_sorts)

        optimizer = z3.Optimize()
        optimizer.set("timeout", 10000)

        # Add constraints
        for c in constraints:
            formula, _ = _parse_python_expr(c, variables, all_sorts)
            optimizer.add(formula)

        # Parse objective
        obj_formula, _ = _parse_python_expr(objective_str, variables, all_sorts)

        if direction == "minimize":
            handle = optimizer.minimize(obj_formula)
        else:
            handle = optimizer.maximize(obj_formula)

        result = optimizer.check()
        status = str(result)

        response = {"result": status}

        if result == z3.sat:
            model = optimizer.model()
            response["model"] = _model_to_dict(model, z3_vars)
            try:
                response["optimum"] = float(str(handle.value()))
            except Exception:
                response["optimum"] = None

        return response
    except Exception as e:
        return {"error": f"Z3 optimize failed: {e}"}


def z3_prove(params: dict) -> dict:
    """Check if a formula is universally valid using z3.Prove.

    z3.Prove(expr) is a convenience wrapper that checks if the negation
    of expr is unsatisfiable (i.e., expr holds for all assignments).

    Params:
        expression (str): The formula to prove (e.g. "Implies(x > 0, x + 1 > 0)")
        variables (list, optional): List of variable names

    Returns:
        {"result": "proved"/"disproved"/"unknown",
         "proved": true/false,
         "counterexample": {...} if disproved}
    """
    expr_str = _get_param(params, "expression")
    variables = params.get("variables")
    timeout = int(params.get("timeout_ms", 10000))

    try:
        # Auto-detect variables if not provided
        if not variables:
            variables = _extract_variables(expr_str)

        sorts = _infer_sorts(expr_str, variables)
        formula, z3_vars = _parse_python_expr(expr_str, variables, sorts)

        # Create solver for proving: negate the formula
        solver = z3.Solver()
        solver.set("timeout", timeout)
        solver.add(z3.Not(formula))

        result = solver.check()
        response = {"proved": False}

        if result == z3.unsat:
            # Negation is unsat → formula is universally valid
            response["result"] = "proved"
            response["proved"] = True
        elif result == z3.sat:
            # Negation is sat → found a counterexample
            model = solver.model()
            response["result"] = "disproved"
            response["proved"] = False
            response["counterexample"] = _model_to_dict(model, z3_vars)
        else:
            response["result"] = "unknown"
            response["proved"] = False

        return response
    except Exception as e:
        return {"error": f"Z3 prove failed: {e}"}


# ===========================================================================
# Solver push/pop — incremental constraint solving
# ===========================================================================


def z3_solver_push(params: dict) -> dict:
    """Push a new context onto the persistent solver stack.

    Note: Each call to this operation starts a fresh solver in a new Python
    process. For truly incremental push/pop across multiple steps, use
    the batched operation `z3_solver_batch` instead.

    Params:
        n (int, optional): Number of contexts to push (default: 1)

    Returns:
        {"result": "ok"}
    """
    try:
        n = int(params.get("n", 1))
        solver = _get_persistent_solver()
        for _ in range(n):
            solver.push()
        return {"result": "ok", "num_scopes": n}
    except Exception as e:
        return {"error": f"Z3 solver push failed: {e}"}


def z3_solver_pop(params: dict) -> dict:
    """Pop a context from the persistent solver stack.

    Note: Each call to this operation starts a fresh solver in a new Python
    process. For truly incremental push/pop across multiple steps, use
    the batched operation `z3_solver_batch` instead.

    Params:
        n (int, optional): Number of contexts to pop (default: 1)

    Returns:
        {"result": "ok"}
    """
    try:
        n = int(params.get("n", 1))
        solver = _get_persistent_solver()
        for _ in range(n):
            solver.pop()
        return {"result": "ok", "num_scopes": n}
    except Exception as e:
        return {"error": f"Z3 solver pop failed: {e}"}


def z3_solver_add(params: dict) -> dict:
    """Add a constraint to the persistent solver.

    Note: Each call to this operation starts a fresh solver in a new Python
    process. For truly incremental solving across multiple steps, use
    the batched operation `z3_solver_batch` instead.

    Params:
        expression (str): The constraint expression to add
        variables (list, optional): List of variable names

    Returns:
        {"result": "ok"}
    """
    expr_str = _get_param(params, "expression")
    variables = params.get("variables")

    try:
        if not variables:
            variables = _extract_variables(expr_str)

        solver = _get_persistent_solver()
        sorts = _infer_sorts(expr_str, variables)

        formula, _ = _parse_python_expr(expr_str, variables, sorts)
        solver.add(formula)

        return {"result": "ok"}
    except Exception as e:
        return {"error": f"Z3 solver add failed: {e}"}


def z3_solver_check(params: dict) -> dict:
    """Check satisfiability of the persistent solver.

    Note: Each call to this operation starts a fresh solver in a new Python
    process. For truly incremental solving across multiple steps, use
    the batched operation `z3_solver_batch` instead.

    Params:
        timeout_ms (int, optional): Timeout in milliseconds (default: 5000)

    Returns:
        {"result": "sat"/"unsat"/"unknown", "model": {...} if sat}
    """
    try:
        timeout = int(params.get("timeout_ms", 5000))
        solver = _get_persistent_solver()
        solver.set("timeout", timeout)

        result = solver.check()
        status = str(result)

        response = {"result": status}

        if result == z3.sat:
            model = solver.model()
            model_dict = _extract_model_from_solver(model, solver)
            if model_dict:
                response["model"] = model_dict

        return response
    except Exception as e:
        return {"error": f"Z3 solver check failed: {e}"}


def z3_solver_reset(params: dict) -> dict:
    """Reset the persistent solver (clear all constraints and contexts).

    Returns:
        {"result": "ok"}
    """
    try:
        _reset_persistent_solver()
        return {"result": "ok"}
    except Exception as e:
        return {"error": f"Z3 solver reset failed: {e}"}


def _extract_model_from_solver(model, solver) -> dict:
    """Extract a model dict from a Z3 model and solver.

    Walks the solver's assertions to discover variable names
    and reconstruct their Z3 variables for model evaluation.
    """
    model_dict = {}
    for d in solver.assertions():
        for child in _ast.walk(_ast.parse(str(d), mode='eval')):
            if isinstance(child, _ast.Name):
                name = child.id
                if name in model_dict:
                    continue
                var_z3 = None
                try:
                    var_z3 = z3.Real(name)
                except Exception:
                    try:
                        var_z3 = z3.Int(name)
                    except Exception:
                        continue
                if var_z3 is not None:
                    val = model.eval(var_z3, model_completion=True)
                    if val is not None:
                        try:
                            model_dict[name] = float(str(val))
                        except (ValueError, TypeError):
                            try:
                                model_dict[name] = int(str(val))
                            except (ValueError, TypeError):
                                model_dict[name] = str(val)
    return model_dict


def z3_solver_batch(params: dict) -> dict:
    """Run a batch of incremental solver steps in a single Python process.

    This is the RECOMMENDED way to use incremental constraint solving
    with push/pop, as it maintains solver state across all steps.

    Each step is a dict with:
        action (str): Required. One of "push", "pop", "add", "check", "reset"
        n (int): For push/pop — number of scopes (default: 1)
        expression (str): For add — the constraint expression
        timeout_ms (int): For check — timeout in ms (default: 5000)

    Params:
        steps (list): List of step dicts

    Returns:
        {
            "steps": [
                {"action": "push", "result": "ok", "num_scopes": 1},
                {"action": "add", "result": "ok"},
                {"action": "check", "result": "sat", "model": {...}},
                {"action": "pop", "result": "ok", "num_scopes": 1}
            ]
        }
    """
    steps = _get_param(params, "steps")
    if isinstance(steps, str):
        steps = json.loads(steps)

    if not isinstance(steps, list):
        return {"error": "'steps' must be a list"}

    try:
        solver = z3.Solver()
        results = []

        for i, step in enumerate(steps):
            action = step.get("action", "")
            step_result = {"action": action}

            try:
                if action == "reset":
                    solver = z3.Solver()
                    step_result["result"] = "ok"

                elif action == "push":
                    n = int(step.get("n", 1))
                    for _ in range(n):
                        solver.push()
                    step_result["result"] = "ok"
                    step_result["num_scopes"] = n

                elif action == "pop":
                    n = int(step.get("n", 1))
                    for _ in range(n):
                        solver.pop()
                    step_result["result"] = "ok"
                    step_result["num_scopes"] = n

                elif action == "add":
                    expr_str = step.get("expression", "")
                    if not expr_str:
                        raise ValueError("'expression' is required for 'add' action")
                    variables = step.get("variables")
                    if not variables:
                        variables = _extract_variables(expr_str)
                    sorts = _infer_sorts(expr_str, variables)
                    formula, _ = _parse_python_expr(expr_str, variables, sorts)
                    solver.add(formula)
                    step_result["result"] = "ok"

                elif action == "check":
                    timeout = int(step.get("timeout_ms", 5000))
                    solver.set("timeout", timeout)
                    check_result = solver.check()
                    status = str(check_result)
                    step_result["result"] = status

                    if check_result == z3.sat:
                        model = solver.model()
                        model_dict = _extract_model_from_solver(model, solver)
                        if model_dict:
                            step_result["model"] = model_dict

                else:
                    raise ValueError(f"unknown action: {action}")

            except Exception as e:
                step_result["result"] = "error"
                step_result["error"] = str(e)

            results.append(step_result)

        return {"steps": results, "num_steps": len(results)}
    except Exception as e:
        return {"error": f"Z3 solver batch failed: {e}"}


# Dispatch table
OPERATIONS = {
    "backend_status": lambda p: {"status": "ok", "result": backend_status()},
    "z3_check": lambda p: _wrap("z3_check", p, z3_check),
    "z3_check_system": lambda p: _wrap("z3_check_system", p, z3_check_system),
    "z3_optimize": lambda p: _wrap("z3_optimize", p, z3_optimize),
    "z3_prove": lambda p: _wrap("z3_prove", p, z3_prove),
    "z3_solver_push": lambda p: _wrap("z3_solver_push", p, z3_solver_push),
    "z3_solver_pop": lambda p: _wrap("z3_solver_pop", p, z3_solver_pop),
    "z3_solver_add": lambda p: _wrap("z3_solver_add", p, z3_solver_add),
    "z3_solver_check": lambda p: _wrap("z3_solver_check", p, z3_solver_check),
    "z3_solver_reset": lambda p: _wrap("z3_solver_reset", p, z3_solver_reset),
    "z3_solver_batch": lambda p: _wrap("z3_solver_batch", p, z3_solver_batch),
}


def _wrap(op_name: str, params: dict, handler) -> dict:
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
    handler = OPERATIONS.get(op)
    if handler is None:
        return {"status": "error", "error": f"unknown operation: {op}"}
    return handler(params)


def _get_param(params: dict, key: str):
    val = params.get(key)
    if val is None:
        raise ValueError(f"missing required parameter: '{key}'")
    return val
