//! Pure Rust symbolic math engine — expression parsing, evaluation, simplification,
//! identity verification, growth classification, and asymptotic analysis.
//!
//! Replaces the Python SymPy/Z3 subprocess bridge with entirely local computation.
//! No external dependencies beyond `std` and `minilp` (for inequality solving).

use crate::verification::asymptotic::OrderRelation;
use core_errors::FrameworkError;
use std::collections::HashMap;

// ════════════════════════════════════════════════════════════════════════════
// Expression AST
// ════════════════════════════════════════════════════════════════════════════

/// Symbolic expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Const(f64),
    Var(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    /// Function call: f(args...). Supports log, exp, sin, cos, sqrt, abs.
    Fn(String, Vec<Expr>),
}

// ════════════════════════════════════════════════════════════════════════════
// Parser — recursive descent
// ════════════════════════════════════════════════════════════════════════════

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        let chars: Vec<char> = input.chars().collect();
        Self { chars, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), FrameworkError> {
        self.skip_ws();
        match self.next() {
            Some(c) if c == expected => Ok(()),
            Some(c) => Err(FrameworkError::validation(format!(
                "expected '{expected}', got '{c}'"
            ))),

            None => Err(FrameworkError::validation(format!(
                "expected '{expected}', got end of input"
            ))),
        }
    }

    /// Parse the full expression (lowest precedence: +, -).
    fn parse_expr(&mut self) -> Result<Expr, FrameworkError> {
        let mut left = self.parse_term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('+') => {
                    self.pos += 1;
                    left = Expr::Add(Box::new(left), Box::new(self.parse_term()?));
                }
                Some('-') => {
                    self.pos += 1;
                    left = Expr::Sub(Box::new(left), Box::new(self.parse_term()?));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// Parse a term (precedence: *, /).
    fn parse_term(&mut self) -> Result<Expr, FrameworkError> {
        let mut left = self.parse_power()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    left = Expr::Mul(Box::new(left), Box::new(self.parse_power()?));
                }
                Some('/') => {
                    self.pos += 1;
                    left = Expr::Div(Box::new(left), Box::new(self.parse_power()?));
                }
                // Implicit multiplication: number followed by variable/function/paren
                Some(c) if c == '(' || c.is_alphabetic() => {
                    if matches!(
                        &left,
                        Expr::Const(_) | Expr::Var(_) | Expr::Fn(..) | Expr::Neg(_) | Expr::Pow(..)
                    ) {
                        left = Expr::Mul(Box::new(left), Box::new(self.parse_power()?));
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// Parse a power expression (right-associative ^).
    fn parse_power(&mut self) -> Result<Expr, FrameworkError> {
        let base = self.parse_unary()?;
        self.skip_ws();
        if self.peek() == Some('^') {
            self.pos += 1;
            let exp = self.parse_power()?; // right-associative
            Ok(Expr::Pow(Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    /// Parse unary negation.
    fn parse_unary(&mut self) -> Result<Expr, FrameworkError> {
        self.skip_ws();
        match self.peek() {
            Some('-') => {
                self.pos += 1;
                let expr = self.parse_unary()?;
                Ok(Expr::Neg(Box::new(expr)))
            }
            Some('+') => {
                self.pos += 1;
                self.parse_unary()
            }
            _ => self.parse_factor(),
        }
    }

    /// Parse a factor: number, variable, function call, or parenthesized expression.
    fn parse_factor(&mut self) -> Result<Expr, FrameworkError> {
        self.skip_ws();
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let expr = self.parse_expr()?;
                self.expect_char(')')?;
                Ok(expr)
            }
            Some(c) if c.is_ascii_digit() || c == '.' => self.parse_number(),
            Some(c) if c.is_alphabetic() || c == '_' => {
                let name = self.parse_ident();
                self.skip_ws();
                if self.peek() == Some('(') {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(')') {
                        args.push(self.parse_expr()?);
                        while self.peek() == Some(',') {
                            self.pos += 1;
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect_char(')')?;
                    Ok(Expr::Fn(name, args))
                } else {
                    // Check for special constants
                    match name.as_str() {
                        "pi" | "π" => Ok(Expr::Const(std::f64::consts::PI)),
                        "e" => Ok(Expr::Const(std::f64::consts::E)),
                        _ => Ok(Expr::Var(name)),
                    }
                }
            }
            Some(c) => Err(FrameworkError::validation(format!(
                "unexpected character: '{c}'"
            ))),
            None => Err(FrameworkError::validation("unexpected end of input")),
        }
    }

    fn parse_number(&mut self) -> Result<Expr, FrameworkError> {
        let mut s = String::new();
        if self.peek() == Some('.') {
            // Handle numbers starting with "."
            s.push('0');
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                s.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        let val: f64 = s
            .parse()
            .map_err(|e| FrameworkError::validation(format!("bad number '{s}': {e}")))?;
        Ok(Expr::Const(val))
    }

    fn parse_ident(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        s
    }
}

/// Parse a string into an expression tree.
pub fn parse(input: &str) -> Result<Expr, FrameworkError> {
    let mut parser = Parser::new(input);
    let expr = parser.parse_expr()?;
    parser.skip_ws();
    if parser.peek().is_some() {
        return Err(FrameworkError::validation(format!(
            "unexpected trailing input at position {}",
            parser.pos
        )));
    }
    Ok(expr)
}

// ════════════════════════════════════════════════════════════════════════════
// Expression evaluation
// ════════════════════════════════════════════════════════════════════════════

/// Evaluate an expression with variable bindings.
pub fn eval(expr: &Expr, vars: &HashMap<String, f64>) -> Result<f64, FrameworkError> {
    match expr {
        Expr::Const(c) => Ok(*c),
        Expr::Var(name) => vars
            .get(name)
            .copied()
            .ok_or_else(|| FrameworkError::validation(format!("undefined variable: {name}"))),
        Expr::Neg(x) => Ok(-eval(x, vars)?),
        Expr::Add(a, b) => Ok(eval(a, vars)? + eval(b, vars)?),
        Expr::Sub(a, b) => Ok(eval(a, vars)? - eval(b, vars)?),
        Expr::Mul(a, b) => Ok(eval(a, vars)? * eval(b, vars)?),
        Expr::Div(a, b) => {
            let bv = eval(b, vars)?;
            if bv.abs() < 1e-15 {
                return Err(FrameworkError::validation("division by zero"));
            }
            Ok(eval(a, vars)? / bv)
        }
        Expr::Pow(a, b) => {
            let base = eval(a, vars)?;
            let exp = eval(b, vars)?;
            Ok(base.powf(exp))
        }
        Expr::Fn(name, args) => {
            let vals: Result<Vec<f64>, _> = args.iter().map(|a| eval(a, vars)).collect();
            let vals = vals?;
            match name.as_str() {
                "log" | "ln" => {
                    if vals.len() != 1 {
                        return Err(FrameworkError::validation("log requires 1 argument"));
                    }
                    Ok(vals[0].ln())
                }
                "log2" => {
                    if vals.len() != 1 {
                        return Err(FrameworkError::validation("log2 requires 1 argument"));
                    }
                    Ok(vals[0].log2())
                }
                "log10" => {
                    if vals.len() != 1 {
                        return Err(FrameworkError::validation("log10 requires 1 argument"));
                    }
                    Ok(vals[0].log10())
                }
                "exp" => {
                    if vals.len() != 1 {
                        return Err(FrameworkError::validation("exp requires 1 argument"));
                    }
                    Ok(vals[0].exp())
                }
                "sin" => {
                    if vals.len() != 1 {
                        return Err(FrameworkError::validation("sin requires 1 argument"));
                    }
                    Ok(vals[0].sin())
                }
                "cos" => {
                    if vals.len() != 1 {
                        return Err(FrameworkError::validation("cos requires 1 argument"));
                    }
                    Ok(vals[0].cos())
                }
                "sqrt" => {
                    if vals.len() != 1 {
                        return Err(FrameworkError::validation("sqrt requires 1 argument"));
                    }
                    Ok(vals[0].sqrt())
                }
                "abs" => {
                    if vals.len() != 1 {
                        return Err(FrameworkError::validation("abs requires 1 argument"));
                    }
                    Ok(vals[0].abs())
                }
                _ => Err(FrameworkError::validation(format!(
                    "unknown function: {name}"
                ))),
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Expression display
// ════════════════════════════════════════════════════════════════════════════

/// Convert an expression to a human-readable string.
pub fn display(expr: &Expr) -> String {
    match expr {
        Expr::Const(c) => {
            if c.fract() == 0.0 && *c >= i64::MIN as f64 && *c <= i64::MAX as f64 {
                format!("{}", *c as i64)
            } else {
                format!("{c}")
            }
        }
        Expr::Var(v) => v.clone(),
        Expr::Neg(x) => format!("(-{})", display(x)),
        Expr::Add(a, b) => format!("{} + {}", display(a), display(b)),
        Expr::Sub(a, b) => format!("{} - {}", display(a), display(b)),
        Expr::Mul(a, b) => {
            let a_str = match a.as_ref() {
                Expr::Add(..) | Expr::Sub(..) => format!("({})", display(a)),
                _ => display(a),
            };
            let b_str = match b.as_ref() {
                Expr::Add(..) | Expr::Sub(..) => format!("({})", display(b)),
                _ => display(b),
            };
            format!("{a_str}*{b_str}")
        }
        Expr::Div(a, b) => format!("{}/{}", display(a), display(b)),
        Expr::Pow(a, b) => format!("{}^{}", display(a), display(b)),
        Expr::Fn(name, args) => {
            let args_str: Vec<String> = args.iter().map(display).collect();
            format!("{}({})", name, args_str.join(", "))
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Simplification
// ════════════════════════════════════════════════════════════════════════════

/// Flatten nested adds into a flat list.
fn flatten_add(expr: &Expr) -> Vec<&Expr> {
    let mut result = Vec::new();
    match expr {
        Expr::Add(a, b) => {
            result.extend(flatten_add(a));
            result.extend(flatten_add(b));
        }
        _ => result.push(expr),
    }
    result
}

/// Flatten nested muls into a flat list.
fn flatten_mul(expr: &Expr) -> Vec<&Expr> {
    let mut result = Vec::new();
    match expr {
        Expr::Mul(a, b) => {
            result.extend(flatten_mul(a));
            result.extend(flatten_mul(b));
        }
        _ => result.push(expr),
    }
    result
}

/// Build an Add node from a list (may produce a single item).
fn make_add(items: Vec<Expr>) -> Expr {
    let mut iter = items.into_iter();
    match iter.next() {
        None => Expr::Const(0.0),
        Some(first) => {
            let mut acc = first;
            for item in iter {
                acc = Expr::Add(Box::new(acc), Box::new(item));
            }
            acc
        }
    }
}

/// Build a Mul node from a list.
fn make_mul(items: Vec<Expr>) -> Expr {
    let mut iter = items.into_iter();
    match iter.next() {
        None => Expr::Const(1.0),
        Some(first) => {
            let mut acc = first;
            for item in iter {
                acc = Expr::Mul(Box::new(acc), Box::new(item));
            }
            acc
        }
    }
}

/// Fold constants and apply basic algebraic identities.
pub fn simplify(expr: &Expr) -> Expr {
    match expr {
        Expr::Const(_) => expr.clone(),
        Expr::Var(_) => expr.clone(),
        Expr::Neg(x) => {
            let sx = simplify(x);
            match &sx {
                Expr::Const(c) => Expr::Const(-c),
                _ => Expr::Neg(Box::new(sx)),
            }
        }
        Expr::Add(a, b) => simplify_add(
            flatten_add(&simplify(a))
                .into_iter()
                .cloned()
                .chain(flatten_add(&simplify(b)).into_iter().cloned())
                .collect(),
        ),
        Expr::Sub(a, b) => {
            let sa = simplify(a);
            let sb = simplify(b);
            // a - a = 0
            if sa == sb {
                return Expr::Const(0.0);
            }
            // a - b = a + (-1)*b, with full simplification of the negated term
            let neg_term = simplify(&Expr::Mul(Box::new(Expr::Const(-1.0)), Box::new(sb)));
            simplify_add(
                flatten_add(&sa)
                    .into_iter()
                    .cloned()
                    .chain(std::iter::once(neg_term))
                    .collect(),
            )
        }
        Expr::Mul(a, b) => simplify_mul(
            flatten_mul(&simplify(a))
                .into_iter()
                .cloned()
                .chain(flatten_mul(&simplify(b)).into_iter().cloned())
                .collect(),
        ),
        Expr::Div(a, b) => {
            let sa = simplify(a);
            let sb = simplify(b);
            match (&sa, &sb) {
                (_, Expr::Const(1.0)) => sa,
                (Expr::Const(0.0), _) => Expr::Const(0.0),
                (Expr::Const(c1), Expr::Const(c2)) if *c2 != 0.0 => Expr::Const(c1 / c2),
                _ => Expr::Div(Box::new(sa), Box::new(sb)),
            }
        }
        Expr::Pow(a, b) => {
            let sa = simplify(a);
            let sb = simplify(b);
            match (&sa, &sb) {
                (_, Expr::Const(c)) if *c == 0.0 => Expr::Const(1.0),
                (_, Expr::Const(c)) if *c == 1.0 => sa,
                (Expr::Const(c1), Expr::Const(c2)) => Expr::Const(c1.powf(*c2)),
                (Expr::Const(0.0), _) => Expr::Const(0.0),
                (Expr::Const(1.0), _) => Expr::Const(1.0),
                _ => Expr::Pow(Box::new(sa), Box::new(sb)),
            }
        }
        Expr::Fn(name, args) => {
            let sargs: Vec<Expr> = args.iter().map(simplify).collect();
            Expr::Fn(name.clone(), sargs)
        }
    }
}

fn simplify_add(items: Vec<Expr>) -> Expr {
    let mut const_sum = 0.0_f64;
    // Group by variable expression using string representation as key
    let mut var_terms: Vec<(String, Expr)> = Vec::new();

    for item in items {
        match item {
            Expr::Const(c) => const_sum += c,
            Expr::Mul(ref a, ref b) if matches!(a.as_ref(), Expr::Const(_)) => {
                if let Expr::Const(c) = a.as_ref() {
                    let var_part = display(b);
                    if let Some(pos) = var_terms.iter().position(|(k, _)| *k == var_part) {
                        // Combine coefficients
                        let existing = var_terms.remove(pos);
                        if let Expr::Mul(existing_coeff, existing_var) = existing.1 {
                            if let Expr::Const(ec) = existing_coeff.as_ref() {
                                let new_coeff = ec + c;
                                if new_coeff.abs() > 1e-12 {
                                    var_terms.push((
                                        var_part,
                                        Expr::Mul(Box::new(Expr::Const(new_coeff)), existing_var),
                                    ));
                                }
                            }
                        }
                    } else {
                        var_terms.push((
                            var_part,
                            Expr::Mul(Box::new(Expr::Const(*c)), Box::new((**b).clone())),
                        ));
                    }
                }
            }
            other => {
                let key = display(&other);
                if let Some(pos) = var_terms.iter().position(|(k, _)| *k == key) {
                    // Combine repeated variable terms: x + x = 2x
                    let (_, ref existing) = var_terms[pos];
                    let coeff = coefficient(existing).unwrap_or(1.0) + 1.0;
                    if coeff.abs() > 1e-12 {
                        var_terms[pos] = (
                            key,
                            make_mul(vec![Expr::Const(coeff), strip_coefficient(existing)]),
                        );
                    } else {
                        var_terms.remove(pos);
                    }
                } else {
                    var_terms.push((key, other));
                }
            }
        }
    }

    // Build result
    let mut terms: Vec<Expr> = var_terms.into_iter().map(|(_, e)| e).collect();

    if const_sum.abs() > 1e-12 {
        terms.push(Expr::Const(const_sum));
    }
    // Sort: constants last, then by display string for determinism
    terms.sort_by(|a, b| {
        let a_is_const = matches!(a, Expr::Const(_));
        let b_is_const = matches!(b, Expr::Const(_));
        if a_is_const && !b_is_const {
            return std::cmp::Ordering::Greater;
        }
        if !a_is_const && b_is_const {
            return std::cmp::Ordering::Less;
        }
        display(a).cmp(&display(b))
    });

    make_add(terms)
}

fn simplify_mul(items: Vec<Expr>) -> Expr {
    let mut const_prod = 1.0_f64;
    let mut var_terms: Vec<Expr> = Vec::new();

    for item in items {
        match item {
            Expr::Const(c) => const_prod *= c,
            other => var_terms.push(other),
        }
    }

    if const_prod.abs() < 1e-12 {
        return Expr::Const(0.0);
    }
    if var_terms.is_empty() {
        return Expr::Const(const_prod);
    }
    if (const_prod - 1.0).abs() < 1e-12 && var_terms.len() == 1 {
        return {
            #[allow(clippy::unwrap_used, clippy::expect_used)]
            var_terms.into_iter().next().unwrap()
        };
    }

    let mut vars = var_terms;
    if (const_prod - (-1.0)).abs() < 1e-12 && vars.len() == 1 {
        return Expr::Neg(Box::new({
            #[allow(clippy::unwrap_used, clippy::expect_used)]
            vars.into_iter().next().unwrap()
        }));
    }

    vars.insert(0, Expr::Const(const_prod));
    make_mul(vars)
}

/// Extract numeric coefficient from a Mul(Const, rest) or single expression.
fn coefficient(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Const(c) => Some(*c),
        Expr::Mul(a, _) => {
            if let Expr::Const(c) = a.as_ref() {
                Some(*c)
            } else {
                Some(1.0)
            }
        }
        _ => Some(1.0),
    }
}

/// Remove leading numeric coefficient from an expression, returning 1 if not present.
fn strip_coefficient(expr: &Expr) -> Expr {
    match expr {
        Expr::Mul(a, b) => {
            if matches!(a.as_ref(), Expr::Const(_)) {
                (**b).clone()
            } else {
                expr.clone()
            }
        }
        _ => expr.clone(),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Expansion (distribute multiplication over addition)
// ════════════════════════════════════════════════════════════════════════════

/// Expand an expression: (a+b)*c → a*c + b*c.
pub fn expand(expr: &Expr) -> Expr {
    let expr = simplify(expr);
    match &expr {
        Expr::Mul(a, b) => {
            let ea = expand(a);
            let eb = expand(b);
            match (&ea, &eb) {
                (Expr::Add(..), _) => distribute_add(&ea, &eb),
                (_, Expr::Add(..)) => distribute_add(&ea, &eb),
                _ => simplify(&Expr::Mul(Box::new(ea), Box::new(eb))),
            }
        }
        Expr::Pow(a, b) => {
            // Only expand small integer powers
            if let Expr::Const(n) = b.as_ref() {
                let n = *n;
                if n.fract() == 0.0 && n >= 0.0 && n <= 10.0 {
                    let n = n as u32;
                    if n == 0 {
                        return Expr::Const(1.0);
                    }
                    if n == 1 {
                        return expand(a);
                    }
                    let base = expand(a);
                    // Repeated squaring
                    let mut result = base.clone();
                    for _ in 1..n {
                        result =
                            simplify(&Expr::Mul(Box::new(result.clone()), Box::new(base.clone())));
                    }
                    // Re-expand the result
                    return expand(&result);
                }
            }
            expr
        }
        Expr::Add(a, b) => Expr::Add(Box::new(expand(a)), Box::new(expand(b))),
        Expr::Sub(a, b) => Expr::Sub(Box::new(expand(a)), Box::new(expand(b))),
        _ => expr,
    }
}

fn distribute_add(add_expr: &Expr, other: &Expr) -> Expr {
    let terms: Vec<&Expr> = flatten_add(add_expr);

    // If other is also an Add, compute cross-term distribution:
    // (a + b) * (c + d) = a*c + a*d + b*c + b*d
    if let Expr::Add(..) = other {
        let other_terms = flatten_add(other);
        let cross: Vec<Expr> = terms
            .iter()
            .flat_map(|t| {
                other_terms
                    .iter()
                    .map(|u| simplify(&Expr::Mul(Box::new((*t).clone()), Box::new((*u).clone()))))
                    .collect::<Vec<_>>()
            })
            .collect();
        return simplify(&make_add(cross));
    }

    // Normal case: distribute add_expr over a single term
    let distributed: Vec<Expr> = terms
        .into_iter()
        .map(|t| simplify(&Expr::Mul(Box::new(t.clone()), Box::new(other.clone()))))
        .collect();
    simplify(&make_add(distributed))
}

// ════════════════════════════════════════════════════════════════════════════
// Identity verification
// ════════════════════════════════════════════════════════════════════════════

/// Check if two expression strings are equivalent.
///
/// Strategy:
/// 1. Parse both, expand to polynomial normal form, simplify, compare.
/// 2. For non-polynomial expressions, use random numerical testing.
pub fn equivalent(lhs: &str, rhs: &str) -> bool {
    let lhs_expr = match parse(lhs) {
        Ok(e) => e,
        Err(_) => return false,
    };
    let rhs_expr = match parse(rhs) {
        Ok(e) => e,
        Err(_) => return false,
    };

    // Strategy 1: Expand both, simplify, compare structurally
    let lhs_expanded = expand(&lhs_expr);
    let rhs_expanded = expand(&rhs_expr);
    let lhs_simplified = simplify(&lhs_expanded);
    let rhs_simplified = simplify(&rhs_expanded);

    if lhs_simplified == rhs_simplified {
        return true;
    }

    // Strategy 2: Random numerical testing
    let vars = collect_variables(&lhs_expr, &rhs_expr);
    if vars.is_empty() {
        // Both are constants — compare numerically
        return match (
            eval(&lhs_expr, &HashMap::new()),
            eval(&rhs_expr, &HashMap::new()),
        ) {
            (Ok(l), Ok(r)) => (l - r).abs() < 1e-8,
            _ => false,
        };
    }

    let mut rng = SimpleRng::new(42);
    for _ in 0..10 {
        let mut bindings = HashMap::new();
        for v in &vars {
            // Generate random values in a range that avoids degenerate cases
            let val = match v.as_str() {
                "n" | "x" | "y" | "z" => rng.next_range(0.5, 10.0),
                _ => rng.next_range(0.5, 10.0),
            };
            bindings.insert(v.clone(), val);
        }

        match (eval(&lhs_expr, &bindings), eval(&rhs_expr, &bindings)) {
            (Ok(l), Ok(r)) => {
                if (l - r).abs() > 1e-6 {
                    return false;
                }
            }
            _ => continue, // skip points where either side errors (e.g., div by zero)
        }
    }
    true
}

/// Collect all variable names from both expressions.
fn collect_variables(a: &Expr, b: &Expr) -> Vec<String> {
    let mut vars = Vec::new();
    collect_vars_rec(a, &mut vars);
    collect_vars_rec(b, &mut vars);
    vars.sort();
    vars.dedup();
    // Filter out constants that look like special identifiers
    vars.retain(|v| v != "pi" && v != "e");
    vars
}

fn collect_vars_rec(expr: &Expr, vars: &mut Vec<String>) {
    match expr {
        Expr::Var(v) => vars.push(v.clone()),
        Expr::Neg(x) | Expr::Pow(x, _) | Expr::Div(x, _) => collect_vars_rec(x, vars),
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) => {
            collect_vars_rec(a, vars);
            collect_vars_rec(b, vars);
        }
        Expr::Fn(_, args) => {
            for arg in args {
                collect_vars_rec(arg, vars);
            }
        }
        _ => {}
    }
}

/// Minimal deterministic PRNG for numerical identity testing.
struct SimpleRng(u64);

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn next_range(&mut self, lo: f64, hi: f64) -> f64 {
        let rand = self.next() as f64 / u64::MAX as f64;
        lo + rand * (hi - lo)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Growth classification for asymptotic analysis
// ════════════════════════════════════════════════════════════════════════════

/// Ordered growth class (PartialOrd via discriminant).
#[derive(Debug, Clone, PartialEq)]
pub enum GrowthClass {
    /// Zero (0) — decays to 0
    Zero,
    /// Constant (O(1))
    Constant,
    /// Polylogarithmic: (log n)^k, parameter = k
    Log(f64),
    /// Power: n^k, parameter = k (0<k<1 fractional, k=1 linear, k>1 superlinear)
    Power(f64),
    /// Exponential: a^n, parameter = a (>1)
    Exp(f64),
    /// Factorial: n!
    Factorial,
    /// Unclassified / infinite growth
    Inf,
}

fn growth_rank(g: &GrowthClass) -> u8 {
    match g {
        GrowthClass::Zero => 0,
        GrowthClass::Constant => 1,
        GrowthClass::Log(_) => 2,
        GrowthClass::Power(_) => 3,
        GrowthClass::Exp(_) => 4,
        GrowthClass::Factorial => 5,
        GrowthClass::Inf => 6,
    }
}

/// Compare two growth classes. Returns `Ordering::Less` if `a` grows slower than `b`.
pub fn compare_growth_classes(a: &GrowthClass, b: &GrowthClass) -> std::cmp::Ordering {
    let ra = growth_rank(a);
    let rb = growth_rank(b);
    match ra.cmp(&rb) {
        std::cmp::Ordering::Equal => {
            // Same rank — compare parameters
            match (a, b) {
                (GrowthClass::Log(ka), GrowthClass::Log(kb)) => {
                    ka.partial_cmp(kb).unwrap_or(std::cmp::Ordering::Equal)
                }
                (GrowthClass::Power(ka), GrowthClass::Power(kb)) => {
                    ka.partial_cmp(kb).unwrap_or(std::cmp::Ordering::Equal)
                }
                (GrowthClass::Exp(ca), GrowthClass::Exp(cb)) => {
                    ca.partial_cmp(cb).unwrap_or(std::cmp::Ordering::Equal)
                }
                _ => std::cmp::Ordering::Equal,
            }
        }
        other => other,
    }
}

/// Classify the growth of an expression as the variable → ∞.
pub fn classify_growth(expr: &Expr, var: &str) -> GrowthClass {
    match expr {
        Expr::Const(c) => {
            if c.abs() < 1e-12 {
                GrowthClass::Zero
            } else {
                GrowthClass::Constant
            }
        }
        Expr::Var(v) => {
            if v == var {
                GrowthClass::Power(1.0)
            } else {
                GrowthClass::Constant
            }
        }
        Expr::Neg(x) => classify_growth(x, var),
        Expr::Add(a, b) => {
            let ga = classify_growth(a, var);
            let gb = classify_growth(b, var);
            // Sum: take the higher growth class
            if compare_growth_classes(&ga, &gb) != std::cmp::Ordering::Less {
                ga
            } else {
                gb
            }
        }
        Expr::Sub(a, b) => {
            let ga = classify_growth(a, var);
            let gb = classify_growth(b, var);
            if compare_growth_classes(&ga, &gb) != std::cmp::Ordering::Less {
                ga
            } else {
                gb
            }
        }
        Expr::Mul(a, b) => {
            let ga = classify_growth(a, var);
            let gb = classify_growth(b, var);
            product_growth(&ga, &gb)
        }
        Expr::Div(a, b) => {
            let ga = classify_growth(a, var);
            let gb = classify_growth(b, var);
            // Division by constant → same as numerator
            if matches!(gb, GrowthClass::Constant) {
                return ga;
            }
            // Division by higher order → decaying
            match compare_growth_classes(&ga, &gb) {
                std::cmp::Ordering::Less => GrowthClass::Zero, // f/g → 0
                std::cmp::Ordering::Equal => GrowthClass::Constant, // same order → constant
                std::cmp::Ordering::Greater => ga,             // f dominates
            }
        }
        Expr::Pow(a, b) => {
            // Check if exponent contains var
            let has_var = contains_var(b, var);
            if has_var {
                // variable exponent: base^{variable} = Exponential
                let base_growth = classify_growth(a, var);
                match &base_growth {
                    GrowthClass::Constant => GrowthClass::Exp(1.0 + 1e-12), // e.g. 2^n
                    _ => GrowthClass::Inf,
                }
            } else {
                // constant exponent
                let base_class = classify_growth(a, var);
                let exp_val = constant_value(b);
                match (&base_class, exp_val) {
                    (GrowthClass::Log(k), Some(e)) => GrowthClass::Log(k * e),
                    (GrowthClass::Power(k), Some(e)) => GrowthClass::Power(k * e),
                    (GrowthClass::Constant, _) => GrowthClass::Constant,
                    _ => base_class,
                }
            }
        }
        Expr::Fn(name, args) => {
            match name.as_str() {
                "log" | "ln" | "log2" | "log10" => {
                    if let Some(arg) = args.first() {
                        let inner = classify_growth(arg, var);
                        // log(poly) → Log, log(constant) → Constant
                        match inner {
                            GrowthClass::Power(_) | GrowthClass::Log(_) | GrowthClass::Exp(_) => {
                                GrowthClass::Log(1.0)
                            }
                            GrowthClass::Constant => GrowthClass::Constant,
                            _ => GrowthClass::Log(1.0),
                        }
                    } else {
                        GrowthClass::Inf
                    }
                }
                "exp" => GrowthClass::Exp(std::f64::consts::E),
                "sqrt" => {
                    if let Some(arg) = args.first() {
                        let inner = classify_growth(arg, var);
                        match inner {
                            GrowthClass::Power(k) => GrowthClass::Power(k * 0.5),
                            GrowthClass::Log(k) => GrowthClass::Log(k * 0.5),
                            _ => inner,
                        }
                    } else {
                        GrowthClass::Inf
                    }
                }
                "sin" | "cos" | "abs" => {
                    // bounded functions
                    if let Some(arg) = args.first() {
                        let inner = classify_growth(arg, var);
                        match inner {
                            GrowthClass::Zero => GrowthClass::Zero,
                            _ => GrowthClass::Constant,
                        }
                    } else {
                        GrowthClass::Inf
                    }
                }
                _ => GrowthClass::Inf,
            }
        }
    }
}

fn product_growth(a: &GrowthClass, b: &GrowthClass) -> GrowthClass {
    use std::cmp::Ordering;
    match (a, b) {
        (GrowthClass::Zero, _) | (_, GrowthClass::Zero) => GrowthClass::Zero,
        (GrowthClass::Constant, x) | (x, GrowthClass::Constant) => x.clone(),
        (GrowthClass::Log(k1), GrowthClass::Log(k2)) => GrowthClass::Log(k1 + k2),
        (GrowthClass::Power(k1), GrowthClass::Power(k2)) => GrowthClass::Power(k1 + k2),
        (GrowthClass::Exp(_), _) | (_, GrowthClass::Exp(_)) => {
            // exp * anything → exp (dominant)
            let e1 = match a {
                GrowthClass::Exp(c) => *c,
                _ => 1.0,
            };
            let e2 = match b {
                GrowthClass::Exp(c) => *c,
                _ => 1.0,
            };
            GrowthClass::Exp(e1.max(e2))
        }
        (GrowthClass::Factorial, _) | (_, GrowthClass::Factorial) => GrowthClass::Factorial,
        (x, y) => {
            // Mixed: take the higher class
            match compare_growth_classes(x, y) {
                Ordering::Less => y.clone(),
                _ => x.clone(),
            }
        }
    }
}

fn contains_var(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Var(v) => v == var,
        Expr::Const(_) => false,
        Expr::Neg(x) => contains_var(x, var),
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) | Expr::Pow(a, b) => {
            contains_var(a, var) || contains_var(b, var)
        }
        Expr::Fn(_, args) => args.iter().any(|a| contains_var(a, var)),
    }
}

fn constant_value(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Const(c) => Some(*c),
        Expr::Neg(x) => constant_value(x).map(|v| -v),
        _ => None,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Public API for asymptotic analysis
// ════════════════════════════════════════════════════════════════════════════

/// Estimate the leading term of an expression as var → ∞.
pub fn leading_term(expr_str: &str, var: &str) -> Result<(String, String), FrameworkError> {
    let expr = parse(expr_str)?;
    let terms = flatten_add(&expr);
    if terms.is_empty() {
        return Err(FrameworkError::validation("empty expression"));
    }

    // Classify each term
    let mut classified: Vec<(f64, &Expr)> = terms
        .iter()
        .map(|t| {
            // For the coefficient check, try to extract it
            let class = classify_growth(t, var);
            (growth_rank(&class) as f64, *t)
        })
        .collect();

    // Find dominant term(s) — highest growth rank
    classified.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    if classified.is_empty() {
        return Err(FrameworkError::validation("cannot classify expression"));
    }

    let dominant_class = classify_growth(classified[0].1, var);
    let leading_str = display(classified[0].1);
    let order_str = growth_to_order(&dominant_class, var);

    Ok((leading_str, order_str))
}

fn growth_to_order(class: &GrowthClass, var: &str) -> String {
    match class {
        GrowthClass::Zero => "o(1)".into(),
        GrowthClass::Constant => "O(1)".into(),
        GrowthClass::Log(k) => {
            if *k == 1.0 {
                format!("O(log({var}))")
            } else {
                format!("O((log {var})^{k})")
            }
        }
        GrowthClass::Power(k) => {
            if *k == 0.0 {
                "O(1)".into()
            } else if *k == 1.0 {
                format!("O({var})")
            } else {
                format!("O({var}^{k})")
            }
        }
        GrowthClass::Exp(c) => {
            if *c == std::f64::consts::E {
                format!("O(exp({var}))")
            } else {
                format!("O({c}^{var})")
            }
        }
        GrowthClass::Factorial => format!("O({var}!)"),
        GrowthClass::Inf => "O(?)".into(),
    }
}

/// Compare the growth of two expression strings.
///
/// Returns the `OrderRelation` that holds between f and g as var → ∞.
pub fn compare_growth(f: &str, g: &str, var: &str) -> Result<OrderRelation, FrameworkError> {
    let f_expr = parse(f)?;
    let g_expr = parse(g)?;

    let gf = classify_growth(&f_expr, var);
    let gg = classify_growth(&g_expr, var);

    // f ≲ g: f grows no faster than g
    // f ≪ g: f grows strictly slower than g  (limit f/g = 0)
    // f ≍ g: same growth rate (limit f/g = finite nonzero)

    match compare_growth_classes(&gf, &gg) {
        std::cmp::Ordering::Less => Ok(OrderRelation::MuchLess), // f ≪ g
        std::cmp::Ordering::Greater => {
            // f grows strictly faster than g (f = ω(g)).
            // None of the OrderRelation variants (≪, ≲, ≍) hold from f to g.
            Err(FrameworkError::validation(format!(
                "{f} grows faster than {g}, so no finite OrderRelation holds"
            )))
        }
        std::cmp::Ordering::Equal => {
            // Same growth class — check if parameters match
            if gf == gg {
                Ok(OrderRelation::Asymp) // f ≍ g
            } else {
                // Same class, different parameters: e.g. n vs n^2 → n ≪ n^2
                // Re-compare within the class
                match compare_growth_classes(&gf, &gg) {
                    std::cmp::Ordering::Less => Ok(OrderRelation::MuchLess),
                    std::cmp::Ordering::Greater => Err(FrameworkError::validation(format!(
                        "{f} grows faster than {g} in the same class"
                    ))),
                    std::cmp::Ordering::Equal => Ok(OrderRelation::Asymp),
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Public API — identity verification and simplification
// ════════════════════════════════════════════════════════════════════════════

/// Check if two expression strings are algebraically equivalent.
/// Returns (is_equivalent, detail_string).
pub fn verify_identity(lhs: &str, rhs: &str) -> (bool, String) {
    let result = equivalent(lhs, rhs);
    if result {
        (true, format!("{lhs} = {rhs}"))
    } else {
        (false, format!("{lhs} ≠ {rhs}"))
    }
}

/// Simplify an expression string to its simplest form.
/// Returns the simplified expression string, or the original on error.
pub fn simplify_expression(expr_str: &str) -> String {
    match parse(expr_str) {
        Ok(expr) => {
            let sm = simplify(&expand(&expr));
            display(&sm)
        }
        Err(_) => expr_str.to_string(),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    // ── Parsing ──

    #[test]
    fn test_parse_simple() {
        assert_eq!(display(&parse("x + 1").unwrap()), "x + 1");
    }

    #[test]
    fn test_parse_precedence() {
        // 2 + 3 * 4 should be 2 + (3*4) not (2+3)*4
        let e = parse("2 + 3 * 4").unwrap();
        let result = eval(&e, &HashMap::new()).unwrap();
        assert!((result - 14.0).abs() < 1e-10, "expected 14, got {result}");
    }

    #[test]
    fn test_parse_parentheses() {
        let e = parse("(2 + 3) * 4").unwrap();
        let result = eval(&e, &HashMap::new()).unwrap();
        assert!((result - 20.0).abs() < 1e-10, "expected 20, got {result}");
    }

    #[test]
    fn test_parse_power_right_assoc() {
        let e = parse("2 ^ 3 ^ 2").unwrap();
        // right-assoc: 2^(3^2) = 2^9 = 512
        let result = eval(&e, &HashMap::new()).unwrap();
        assert!((result - 512.0).abs() < 1e-8, "expected 512, got {result}");
    }

    #[test]
    fn test_parse_variable() {
        let e = parse("x + 1").unwrap();
        let mut vars = HashMap::new();
        vars.insert("x".into(), 5.0);
        assert!((eval(&e, &vars).unwrap() - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_unary_minus() {
        let e = parse("-x").unwrap();
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        assert!((eval(&e, &vars).unwrap() - (-3.0)).abs() < 1e-10);
    }

    #[test]
    fn test_parse_function() {
        let e = parse("sin(0)").unwrap();
        assert!((eval(&e, &HashMap::new()).unwrap() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_implicit_mul() {
        // 2x should parse as 2*x
        let e = parse("2x").unwrap();
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        assert!((eval(&e, &vars).unwrap() - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_pi() {
        let e = parse("pi").unwrap();
        assert!((eval(&e, &HashMap::new()).unwrap() - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn test_parse_fraction() {
        // "1/2" as expression
        let e = parse("1/2").unwrap();
        assert!((eval(&e, &HashMap::new()).unwrap() - 0.5).abs() < 1e-10);
    }

    // ── Simplification ──

    #[test]
    fn test_simplify_constant_add() {
        let e = parse("2 + 3").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "5");
    }

    #[test]
    fn test_simplify_x_plus_x() {
        let e = parse("x + x").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "2*x");
    }

    #[test]
    fn test_simplify_times_zero() {
        let e = parse("x * 0").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "0");
    }

    #[test]
    fn test_simplify_times_one() {
        let e = parse("x * 1").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "x");
    }

    #[test]
    fn test_simplify_power_zero() {
        let e = parse("x ^ 0").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "1");
    }

    #[test]
    fn test_simplify_power_one() {
        let e = parse("x ^ 1").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "x");
    }

    #[test]
    fn test_simplify_sub_to_add() {
        let e = parse("x - x").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "0");
    }

    // ── Identity verification ──

    #[test]
    fn test_equivalent_trivial() {
        assert!(equivalent("x", "x"));
    }

    #[test]
    fn test_equivalent_polynomial() {
        // (x+1)^2 = x^2 + 2x + 1
        assert!(
            equivalent("(x+1)^2", "x^2 + 2*x + 1"),
            "(x+1)^2 should equal x^2 + 2x + 1"
        );
    }

    #[test]
    fn test_equivalent_not_equal() {
        assert!(!equivalent("x + 1", "x + 2"));
    }

    #[test]
    fn test_equivalent_constant() {
        assert!(equivalent("2 + 3", "5"));
    }

    #[test]
    fn test_equivalent_distributive() {
        assert!(equivalent("2*(x+1)", "2*x + 2"));
    }

    // ── Expand ──

    #[test]
    fn test_expand_simple() {
        let e = parse("(x+1)*(x+2)").unwrap();
        let expanded = expand(&e);
        let simplified = simplify(&expanded);
        // x*x + x*2 + 1*x + 1*2 = x^2 + 3x + 2
        // Our expand might not collect terms perfectly but let's check equivalence
        assert!(
            equivalent(&display(&simplified), "x^2 + 3*x + 2"),
            "expected x^2+3x+2, got {}",
            display(&simplified)
        );
    }

    // ── Growth classification ──

    #[test]
    fn test_growth_constant() {
        let e = parse("42").unwrap();
        assert_eq!(classify_growth(&e, "n"), GrowthClass::Constant);
    }

    #[test]
    fn test_growth_linear() {
        let e = parse("n").unwrap();
        assert_eq!(classify_growth(&e, "n"), GrowthClass::Power(1.0));
    }

    #[test]
    fn test_growth_quadratic() {
        let e = parse("n^2").unwrap();
        assert_eq!(classify_growth(&e, "n"), GrowthClass::Power(2.0));
    }

    #[test]
    fn test_growth_exponential() {
        let e = parse("2^n").unwrap();
        let g = classify_growth(&e, "n");
        assert!(
            matches!(g, GrowthClass::Exp(_)),
            "expected Exp, got {:?}",
            g
        );
    }

    #[test]
    fn test_growth_log() {
        let e = parse("log(n)").unwrap();
        assert_eq!(classify_growth(&e, "n"), GrowthClass::Log(1.0));
    }

    #[test]
    fn test_growth_product() {
        let e = parse("n * log(n)").unwrap();
        let g = classify_growth(&e, "n");
        assert!(
            matches!(g, GrowthClass::Power(k) if (k - 1.0).abs() < 1e-10),
            "expected Power(1), got {:?}",
            g
        );
    }

    #[test]
    fn test_compare_linear_vs_quadratic() {
        let result = compare_growth("n", "n^2", "n").unwrap();
        assert_eq!(result, OrderRelation::MuchLess, "n ≪ n^2");
    }

    #[test]
    fn test_compare_constant_vs_linear() {
        let result = compare_growth("1", "n", "n").unwrap();
        assert_eq!(result, OrderRelation::MuchLess);
    }

    #[test]
    fn test_compare_log_vs_linear() {
        let result = compare_growth("log(n)", "n", "n").unwrap();
        assert_eq!(result, OrderRelation::MuchLess, "log(n) ≪ n");
    }

    #[test]
    fn test_compare_exponential_vs_poly() {
        // n^10 grows slower than 2^n, so n^10 ≪ 2^n
        let result_slow = compare_growth("n^10", "2^n", "n").unwrap();
        assert_eq!(result_slow, OrderRelation::MuchLess, "n^10 ≪ 2^n");

        // 2^n grows faster than n^10 — no OrderRelation holds from 2^n to n^10
        let result_fast = compare_growth("2^n", "n^10", "n");
        assert!(
            result_fast.is_err(),
            "2^n grows faster than n^10, no relation should hold"
        );
    }

    #[test]
    fn test_leading_term_polynomial() {
        let (leading, _) = leading_term("n^2 + n", "n").unwrap();
        assert_eq!(leading, "n^2");
    }

    // ── Simplification API ──

    #[test]
    fn test_simplify_expression_api() {
        assert_eq!(simplify_expression("x + x"), "2*x");
        assert_eq!(simplify_expression("0 * x + 5"), "5");
        assert_eq!(simplify_expression("x ^ 0"), "1");
    }

    #[test]
    fn test_verify_identity_api() {
        let (eq, _) = verify_identity("x", "x");
        assert!(eq);
        let (neq, _) = verify_identity("x + 1", "x + 2");
        assert!(!neq);
    }

    // ── Overflow / edge cases ──

    #[test]
    fn test_parse_empty_fails() {
        assert!(parse("").is_err());
    }

    #[test]
    fn test_parse_bad_chars() {
        assert!(parse("@#$").is_err());
    }

    #[test]
    fn test_eval_undefined_var() {
        let e = parse("x + 1").unwrap();
        let result = eval(&e, &HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_growth_zero() {
        let e = parse("0").unwrap();
        assert_eq!(classify_growth(&e, "n"), GrowthClass::Zero);
    }

    #[test]
    fn test_equivalent_distribute_power2() {
        // (x+2)^2 = x^2 + 4x + 4
        assert!(equivalent("(x+2)^2", "x^2 + 4*x + 4"));
    }

    #[test]
    fn test_constant_folding_complex() {
        let e = parse("2 + 3 * 4 - 1").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "13");
    }

    #[test]
    fn test_simplify_mul_neg_one() {
        let e = parse("-1 * x").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "(-x)");
    }

    #[test]
    fn test_simplify_div_const() {
        let e = parse("10 / 2").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "5");
    }
}
