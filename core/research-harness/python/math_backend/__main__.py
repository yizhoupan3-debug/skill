#!/usr/bin/env python3
"""
math_backend CLI — JSON stdin/stdout subprocess bridge.

Usage:
    uv run -m math_backend --stdin-json

Reads a JSON request on stdin, dispatches to the appropriate backend
(Z3 or SymPy), and writes a JSON response to stdout.

Request format:
    {"id": 1, "op": "sympy_simplify", "params": {"expression": "x**2 + 2*x + 1"}}

Response format:
    {"id": 1, "status": "ok", "result": {"simplified": "(x + 1)**2"}}
    {"id": 1, "status": "error", "error": "message"}
"""

import sys
import json

from . import __version__

# Import sub-modules
from . import sympy_ops
from . import z3_ops


def check_backend_status() -> dict:
    """Combined backend status for all math backends."""
    sympy_status = sympy_ops.backend_status()
    z3_status = z3_ops.backend_status()

    # Check lean availability (optional)
    lean_available = False
    import shutil
    lean_available = shutil.which("lean") is not None

    return {
        "sympy": sympy_status,
        "z3": z3_status,
        "lean": {
            "available": lean_available,
            "description": "Lean theorem prover (probed via PATH)",
        },
    }


def dispatch(request: dict) -> dict:
    """Dispatch a single request to the appropriate handler."""
    req_id = request.get("id", 0)
    op = request.get("op", "")
    params = request.get("params", {})

    if not op:
        return {"id": req_id, "status": "error", "error": "missing 'op' field"}

    # Root-level operations
    if op == "backend_status":
        result = check_backend_status()
        return {"id": req_id, "status": "ok", "result": result}

    # Dispatch by operation prefix
    try:
        if op.startswith("sympy_"):
            result = sympy_ops.dispatch(op, params)
        elif op.startswith("z3_"):
            result = z3_ops.dispatch(op, params)
        else:
            return {"id": req_id, "status": "error", "error": f"unknown operation: {op}"}

        # result is already in {status, result/error} format from the sub-dispatch
        result["id"] = req_id
        return result
    except Exception as e:
        return {"id": req_id, "status": "error", "error": f"dispatch failed: {e}"}


def main():
    """Main entry point: read JSON from stdin, dispatch, write to stdout."""
    # Support both piped input and --stdin-json mode
    raw_lines = []
    for line in sys.stdin:
        raw_lines.append(line)

    raw_input = "".join(raw_lines).strip()
    if not raw_input:
        # Empty input, just report status and exit
        response = dispatch({"id": 0, "op": "backend_status", "params": {}})
        json.dump(response, sys.stdout)
        sys.stdout.write("\n")
        sys.stdout.flush()
        return

    # Parse JSON input
    try:
        request = json.loads(raw_input)
    except json.JSONDecodeError as e:
        response = {"id": 0, "status": "error", "error": f"JSON parse error: {e}"}
        json.dump(response, sys.stdout)
        sys.stdout.write("\n")
        sys.stdout.flush()
        return

    # Handle batch requests (array of requests)
    if isinstance(request, list):
        responses = [dispatch(req) for req in request]
        json.dump(responses, sys.stdout)
        sys.stdout.write("\n")
        sys.stdout.flush()
        return

    # Single request
    response = dispatch(request)
    json.dump(response, sys.stdout)
    sys.stdout.write("\n")
    sys.stdout.flush()


if __name__ == "__main__":
    main()
