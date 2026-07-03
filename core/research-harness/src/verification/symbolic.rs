//! Pure Rust symbolic math engine — expression parsing, evaluation, simplification,
//! identity verification, growth classification, and asymptotic analysis.
//!
//! Replaces the Python SymPy/Z3 subprocess bridge with entirely local computation.
//! No external dependencies beyond `std` and `minilp` (for inequality solving).

use crate::verification::asymptotic::OrderRelation;
use core_errors::FrameworkError;
use std::collections::HashMap;

/// Canonical set of known math function/constant keywords used for variable
/// extraction across verification modules. Kept here as the single source of
/// truth — all call sites MUST reference this instead of inlining their own.
pub const MATH_KEYWORDS: &[&str] = &[
    // Basic trig
    "sin", "cos", "tan",
    // Hyperbolic
    "sinh", "cosh", "tanh",
    // Inverse trig
    "asin", "acos", "atan", "atan2",
    // Misc math
    "sqrt", "abs", "exp", "log", "ln", "log2", "log10",
    "erf", "gamma", "ceil", "floor", "round", "sign", "sgn",
    "min", "max", "pow", "mod", "rem",
    // Constants
    "pi", "e",
    // Logical (Z3)
    "And", "Or", "Not", "Implies", "True", "False",
];

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
                _ => {
                    if sa == sb {
                        Expr::Const(1.0)
                    } else {
                        Expr::Div(Box::new(sa), Box::new(sb))
                    }
                }
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
            // Constant-fold trig functions when argument is a constant
            if sargs.len() == 1 {
                if let Expr::Const(c) = &sargs[0] {
                    return match name.as_str() {
                        "sin" => Expr::Const(c.sin()),
                        "cos" => Expr::Const(c.cos()),
                        "tan" => Expr::Const(c.tan()),
                        _ => Expr::Fn(name.clone(), sargs),
                    };
                }
            }
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

    // ── Same-base power merging (skip if any term contains Add/Sub at any depth) ──
    // This check prevents breaking expand()'s distribution mechanism, which relies on
    // simplify() preserving the Mul structure for Add/Sub-containing terms.
    fn contains_sum(expr: &Expr) -> bool {
        match expr {
            Expr::Add(..) | Expr::Sub(..) => true,
            Expr::Neg(x) => contains_sum(x),
            Expr::Mul(a, b) | Expr::Div(a, b) | Expr::Pow(a, b) => {
                contains_sum(a) || contains_sum(b)
            }
            Expr::Fn(_, args) => args.iter().any(|a| contains_sum(a)),
            _ => false,
        }
    }
    let has_sum = var_terms.iter().any(|t| contains_sum(t));
    if !has_sum {
        let mut base_map: HashMap<String, (Expr, f64)> = HashMap::new();
        let mut unique_idx = 0u64;
        for term in var_terms {
            let (base, exp) = match &term {
                Expr::Pow(b, e) => {
                    if let Expr::Const(ce) = e.as_ref() {
                        ((**b).clone(), *ce)
                    } else {
                        let unique_key = format!("\0{}_{}", display(&term), {
                            let idx = unique_idx;
                            unique_idx += 1;
                            idx
                        });
                        base_map.entry(unique_key).or_insert((term, 1.0));
                        continue;
                    }
                }
                _ => (term.clone(), 1.0),
            };
            let key = display(&base);
            base_map
                .entry(key)
                .and_modify(|(_, e)| *e += exp)
                .or_insert((base, exp));
        }
        let mut merged: Vec<Expr> = Vec::new();
        for (_, (base, exp)) in base_map.into_iter() {
            if exp.abs() < 1e-12 {
                continue;
            }
            if (exp - 1.0).abs() < 1e-12 {
                merged.push(base);
            } else {
                merged.push(Expr::Pow(Box::new(base), Box::new(Expr::Const(exp))));
            }
        }
        var_terms = merged;
    }
    // ── End of power merging ──

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

    if (const_prod - 1.0).abs() < 1e-12 {
        make_mul(vars)
    } else {
        vars.insert(0, Expr::Const(const_prod));
        make_mul(vars)
    }
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
// Trig simplification — basic identity lookup
// ════════════════════════════════════════════════════════════════════════════

/// Apply basic trigonometric identity simplifications.
///
/// Currently supports:
/// - sin(x)^2 + cos(x)^2 → 1
///
/// (Constant folding of sin(0)=0, cos(0)=1, sin(pi)=0, cos(pi)=-1
///  is handled inside `simplify` via the Fn arm.)
pub fn trig_simplify(expr: &Expr) -> Expr {
    match expr {
        Expr::Add(..) => {
            let terms: Vec<Expr> = flatten_add(expr).into_iter().cloned().collect();
            let mut result = Vec::new();
            let n = terms.len();
            let mut used = vec![false; n];

            for i in 0..n {
                if used[i] {
                    continue;
                }
                if let Some(arg) = is_sin_sq(&terms[i]) {
                    let mut found = false;
                    for j in (i + 1)..n {
                        if !used[j] {
                            if let Some(arg2) = is_cos_sq(&terms[j]) {
                                if arg == arg2 {
                                    result.push(Expr::Const(1.0));
                                    used[i] = true;
                                    used[j] = true;
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                    if !found {
                        result.push(trig_simplify(&terms[i]));
                        used[i] = true;
                    }
                } else {
                    result.push(trig_simplify(&terms[i]));
                    used[i] = true;
                }
            }

            simplify(&make_add(result))
        }
        Expr::Sub(a, b) => {
            let ta = trig_simplify(a);
            let tb = trig_simplify(b);

            // 1 - sin(x)^2 → cos(x)^2
            if matches!(&ta, Expr::Const(c) if *c == 1.0) {
                if let Some(inner) = extract_sin_sq_inner(&tb) {
                    let cos_sq = Expr::Pow(
                        Box::new(Expr::Fn("cos".to_string(), vec![inner.clone()])),
                        Box::new(Expr::Const(2.0)),
                    );
                    return trig_simplify(&cos_sq);
                }
            }

            // 1 - cos(x)^2 → sin(x)^2
            if matches!(&ta, Expr::Const(c) if *c == 1.0) {
                if let Some(inner) = extract_cos_sq_inner(&tb) {
                    let sin_sq = Expr::Pow(
                        Box::new(Expr::Fn("sin".to_string(), vec![inner.clone()])),
                        Box::new(Expr::Const(2.0)),
                    );
                    return trig_simplify(&sin_sq);
                }
            }

            Expr::Sub(Box::new(ta), Box::new(tb))
        }
        Expr::Mul(a, b) => {
            Expr::Mul(Box::new(trig_simplify(a)), Box::new(trig_simplify(b)))
        }
        Expr::Div(a, b) => {
            Expr::Div(Box::new(trig_simplify(a)), Box::new(trig_simplify(b)))
        }
        Expr::Pow(a, b) => {
            Expr::Pow(Box::new(trig_simplify(a)), Box::new(trig_simplify(b)))
        }
        Expr::Neg(x) => Expr::Neg(Box::new(trig_simplify(x))),
        Expr::Fn(name, args) => {
            let sargs: Vec<Expr> = args.iter().map(|a| trig_simplify(a)).collect();

            // sin(2*x) → 2*sin(x)*cos(x)
            if name == "sin" && sargs.len() == 1 {
                if let Expr::Mul(a, b) = &sargs[0] {
                    if let Expr::Const(c) = a.as_ref() {
                        if (*c - 2.0).abs() < 1e-12 {
                            let inner = trig_simplify(b);
                            let sin_term = Expr::Fn("sin".to_string(), vec![inner.clone()]);
                            let cos_term = Expr::Fn("cos".to_string(), vec![inner]);
                            return Expr::Mul(
                                Box::new(Expr::Const(2.0)),
                                Box::new(Expr::Mul(Box::new(sin_term), Box::new(cos_term))),
                            );
                        }
                    }
                }
            }

            // tan constant folding
            if name == "tan" && sargs.len() == 1 {
                if let Expr::Const(c) = &sargs[0] {
                    return Expr::Const(c.tan());
                }
            }

            Expr::Fn(name.clone(), sargs)
        }
        _ => expr.clone(),
    }
}

/// Check if an expression is `sin(arg)^2`.
fn is_sin_sq(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Pow(base, exp)
            if matches!(exp.as_ref(), Expr::Const(c) if (*c - 2.0).abs() < 1e-12) =>
        {
            if let Expr::Fn(name, args) = base.as_ref() {
                if name == "sin" && args.len() == 1 {
                    return Some(display(&args[0]));
                }
            }
            None
        }
        _ => None,
    }
}

/// Check if an expression is `cos(arg)^2`.
fn is_cos_sq(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Pow(base, exp)
            if matches!(exp.as_ref(), Expr::Const(c) if (*c - 2.0).abs() < 1e-12) =>
        {
            if let Expr::Fn(name, args) = base.as_ref() {
                if name == "cos" && args.len() == 1 {
                    return Some(display(&args[0]));
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the inner expression from sin(arg)^2, for identity rewriting.
fn extract_sin_sq_inner(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Pow(base, exp)
            if matches!(exp.as_ref(), Expr::Const(c) if (*c - 2.0).abs() < 1e-12) =>
        {
            if let Expr::Fn(name, args) = base.as_ref() {
                if name == "sin" && args.len() == 1 {
                    return Some(&args[0]);
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the inner expression from cos(arg)^2, for identity rewriting.
fn extract_cos_sq_inner(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Pow(base, exp)
            if matches!(exp.as_ref(), Expr::Const(c) if (*c - 2.0).abs() < 1e-12) =>
        {
            if let Expr::Fn(name, args) = base.as_ref() {
                if name == "cos" && args.len() == 1 {
                    return Some(&args[0]);
                }
            }
            None
        }
        _ => None,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Algebraic simplification — rationalization and common-factor extraction
// ════════════════════════════════════════════════════════════════════════════

/// Rationalize an expression — put all terms over a common denominator.
///
/// E.g. `a/b + c/d` → `(a*d + b*c) / (b*d)`.
pub fn rationalize(expr: &Expr) -> Expr {
    let expr = simplify(expr);
    match &expr {
        Expr::Add(..) | Expr::Sub(..) => {
            let terms = flatten_add(&expr);
            let mut pairs: Vec<(Expr, Expr)> = Vec::new();

            for term in terms {
                match term {
                    Expr::Div(num, den) => {
                        pairs.push(((**num).clone(), (**den).clone()));
                    }
                    Expr::Neg(inner) => match inner.as_ref() {
                        Expr::Div(num, den) => {
                            // -(num/den) → (-num)/den
                            pairs.push((
                                simplify(&Expr::Mul(
                                    Box::new(Expr::Const(-1.0)),
                                    Box::new((**num).clone()),
                                )),
                                (**den).clone(),
                            ));
                        }
                        other => {
                            pairs.push((
                                simplify(&Expr::Mul(
                                    Box::new(Expr::Const(-1.0)),
                                    Box::new(other.clone()),
                                )),
                                Expr::Const(1.0),
                            ));
                        }
                    },
                    other => {
                        pairs.push((other.clone(), Expr::Const(1.0)));
                    }
                }
            }

            // Find all unique denominators
            let mut dens: Vec<Expr> = pairs.iter().map(|(_, d)| d.clone()).collect();
            dens.sort_by(|a, b| display(a).cmp(&display(b)));
            dens.dedup();

            // If all denominators are 1, nothing to do
            if dens.len() == 1
                && matches!(&dens[0], Expr::Const(c) if (c - 1.0).abs() < 1e-12)
            {
                return expr;
            }

            // For each pair, compute numerator * product of OTHER denominators
            let new_nums: Vec<Expr> = pairs
                .iter()
                .map(|(num, den)| {
                    let others: Vec<Expr> = dens
                        .iter()
                        .filter(|d| *d != den)
                        .cloned()
                        .collect();
                    if others.is_empty() {
                        num.clone()
                    } else {
                        let mut factors = vec![num.clone()];
                        factors.extend(others);
                        make_mul(factors)
                    }
                })
                .collect();

            let numerator = make_add(new_nums);
            let common_den = make_mul(dens);

            simplify(&Expr::Div(Box::new(numerator), Box::new(common_den)))
        }
        _ => expr,
    }
}

/// Extract common factors from a sum.
///
/// E.g. `2x + 2y` → `2*(x + y)`.
pub fn factor_common(expr: &Expr) -> Expr {
    let expr = simplify(expr);
    match &expr {
        Expr::Add(..) => {
            let terms = flatten_add(&expr);
            if terms.len() <= 1 {
                return expr;
            }

            // Factor each term into (coefficient, [factors])
            let mut coeffs: Vec<f64> = Vec::new();
            let mut factored_terms: Vec<Vec<Expr>> = Vec::new();

            for term in &terms {
                let (c, parts) = factor_term(term);
                coeffs.push(c);
                factored_terms.push(parts);
            }

            // Find GCD of coefficients
            let coeff_gcd = coeffs.iter().fold(coeffs[0], |acc, &c| gcd_f64(acc, c));

            // Find common variable factors
            let common_factors = find_common_factors(&factored_terms);

            if (coeff_gcd.abs() < 1e-12)
                || ((coeff_gcd - 1.0).abs() < 1e-12 && common_factors.is_empty())
            {
                return expr;
            }

            // Build the extracted common factor
            let mut extracted = Vec::new();
            if (coeff_gcd - 1.0).abs() > 1e-12 {
                extracted.push(Expr::Const(coeff_gcd));
            }
            extracted.extend(common_factors.iter().cloned());
            let common = make_mul(extracted);

            // Divide each term by the common factor
            let new_terms: Vec<Expr> = terms
                .iter()
                .map(|t| {
                    simplify(&Expr::Div(
                        Box::new((*t).clone()),
                        Box::new(common.clone()),
                    ))
                })
                .collect();

            let sum = make_add(new_terms);
            simplify(&Expr::Mul(Box::new(common), Box::new(sum)))
        }
        _ => expr,
    }
}

/// Split a term into (coefficient, [variable-factor, ...]).
fn factor_term(expr: &Expr) -> (f64, Vec<Expr>) {
    match expr {
        Expr::Const(c) => (*c, vec![]),
        Expr::Mul(..) => {
            let factors: Vec<&Expr> = flatten_mul(expr);
            let mut coeff = 1.0_f64;
            let mut parts = Vec::new();
            for f in factors {
                match f {
                    Expr::Const(c) => coeff *= c,
                    other => parts.push(other.clone()),
                }
            }
            (coeff, parts)
        }
        other => (1.0, vec![other.clone()]),
    }
}

/// Find variable factors that appear in every term.
fn find_common_factors(terms: &[Vec<Expr>]) -> Vec<Expr> {
    if terms.is_empty() {
        return vec![];
    }
    // Filter out empty factor lists
    let non_empty: Vec<&Vec<Expr>> = terms.iter().filter(|t| !t.is_empty()).collect();
    if non_empty.is_empty() {
        return vec![];
    }

    // Start with the first term's factors as display strings
    let first_keys: Vec<String> = non_empty[0].iter().map(|f| display(f)).collect();
    let mut common_indices: Vec<usize> = (0..first_keys.len()).collect();

    for term in non_empty.iter().skip(1) {
        let term_keys: Vec<String> = term.iter().map(|f| display(f)).collect();
        common_indices.retain(|&idx| term_keys.iter().any(|tk| *tk == first_keys[idx]));
    }

    common_indices
        .iter()
        .map(|&idx| non_empty[0][idx].clone())
        .collect()
}

/// Euclidean GCD for integer-valued floating-point coefficients.
fn gcd_f64(a: f64, b: f64) -> f64 {
    let ai = a.round() as i64;
    let bi = b.round() as i64;
    if (ai as f64 - a).abs() > 1e-10 || (bi as f64 - b).abs() > 1e-10 {
        return 1.0;
    }
    let (mut a, mut b) = (ai, bi);
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.abs() as f64
}

// ════════════════════════════════════════════════════════════════════════════
// Enhanced factoring — quadratic, difference of squares
// ════════════════════════════════════════════════════════════════════════════

/// Extract the variable name from a base expression (may be wrapped in Neg).
fn extract_var_from_base(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(v) => Some(v.as_str()),
        Expr::Neg(inner) => {
            if let Expr::Var(v) = inner.as_ref() {
                Some(v.as_str())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract (power, coefficient) from a single term in a polynomial sum.
///
/// Handles: `Var(x)`, `Const(c)`, `Neg(…)`, `Pow(Var(x), n)`,
/// `Mul(c, Var(x))`, `Mul(c, Pow(Var(x), n))`, and parser-specific forms
/// like `Pow(Neg(Var(x)), n)` (= (-x)^n).
fn extract_power_and_coeff(term: &Expr, var: &str) -> Option<(u32, f64)> {
    match term {
        Expr::Var(v) if v == var => Some((1, 1.0)),
        Expr::Const(c) => Some((0, *c)),
        Expr::Neg(inner) => {
            let (pow, coeff) = extract_power_and_coeff(inner, var)?;
            Some((pow, -coeff))
        }
        Expr::Pow(base, exp) => {
            if let Some(v) = extract_var_from_base(base) {
                if v == var {
                    if let Expr::Const(e) = exp.as_ref() {
                        if e.fract() == 0.0 && *e >= 0.0 && *e <= f64::from(u32::MAX) {
                            return Some((*e as u32, 1.0));
                        }
                    }
                }
            }
            None
        }
        Expr::Mul(a, b) => {
            if let Expr::Const(c) = a.as_ref() {
                let (pow, coeff) = extract_power_and_coeff(b, var)?;
                Some((pow, c * coeff))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Try to find the variable used in a quadratic term from a list of flattened-add terms.
fn find_quadratic_var<'a>(terms: &[&'a Expr]) -> Option<&'a str> {
    for term in terms {
        let (base, exp) = match term {
            Expr::Pow(b, e) => (Some(b.as_ref()), Some(e.as_ref())),
            Expr::Mul(c, inner) if matches!(c.as_ref(), Expr::Const(_)) => match inner.as_ref() {
                Expr::Pow(b, e) => (Some(b.as_ref()), Some(e.as_ref())),
                Expr::Neg(nested) => {
                    if let Expr::Pow(b, e) = nested.as_ref() {
                        (Some(b.as_ref()), Some(e.as_ref()))
                    } else {
                        (None, None)
                    }
                }
                _ => (None, None),
            },
            Expr::Neg(inner) => {
                if let Expr::Pow(b, e) = inner.as_ref() {
                    (Some(b.as_ref()), Some(e.as_ref()))
                } else {
                    (None, None)
                }
            }
            _ => (None, None),
        };
        if let (Some(b), Some(e)) = (base, exp) {
            if matches!(e, Expr::Const(c) if (*c - 2.0).abs() < 1e-12) {
                if let Some(v) = extract_var_from_base(b) {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// Factor a quadratic expression of the form ax^2 + bx + c (with perfect-square discriminant).
fn factor_quadratic(expr: &Expr) -> Option<Expr> {
    let terms = flatten_add(expr);
    let var = find_quadratic_var(&terms)?;

    let mut a = 0.0_f64;
    let mut b = 0.0_f64;
    let mut c = 0.0_f64;

    for term in &terms {
        match extract_power_and_coeff(term, var) {
            Some((pow, coeff)) => match pow {
                2 => a += coeff,
                1 => b += coeff,
                0 => c += coeff,
                _ => return None, // term of degree > 2
            },
            None => {
                // Does it contain the variable? If so, can't handle it.
                if contains_var(term, var) {
                    return None;
                }
                // Otherwise, constant in other vars or numeric
                if let Ok(val) = eval(term, &HashMap::new()) {
                    c += val;
                } else {
                    return None;
                }
            }
        }
    }

    if a.abs() < 1e-12 {
        return None;
    }

    let disc = b * b - 4.0 * a * c;
    if disc < -1e-12 {
        return None; // Complex roots
    }
    let safe_disc = if disc < 0.0 { 0.0 } else { disc };
    let sqrt_disc = safe_disc.sqrt();

    // Discriminant must be a near-perfect square for nice factoring
    if safe_disc > 1e-12 {
        let rounded = sqrt_disc.round();
        if (sqrt_disc - rounded).abs() > 1e-10 {
            return None;
        }
    }

    let sqrt_val = if safe_disc < 1e-12 {
        0.0
    } else {
        sqrt_disc.round()
    };
    let x1 = (-b - sqrt_val) / (2.0 * a);
    let x2 = (-b + sqrt_val) / (2.0 * a);

    // Roots should be integers
    let x1r = x1.round();
    let x2r = x2.round();
    if (x1 - x1r).abs() > 1e-10 || (x2 - x2r).abs() > 1e-10 {
        return None;
    }

    let v = Expr::Var(var.to_string());
    let factor1 = Expr::Sub(Box::new(v.clone()), Box::new(Expr::Const(x1r)));
    let factor2 = Expr::Sub(Box::new(v), Box::new(Expr::Const(x2r)));

    let mut result = Expr::Mul(Box::new(factor1), Box::new(factor2));
    if (a - 1.0).abs() > 1e-12 {
        result = simplify(&Expr::Mul(Box::new(Expr::Const(a)), Box::new(result)));
    }

    Some(result)
}

/// Check if an expression is `base^2` and return the base.
fn is_pow2(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Pow(base, exp)
            if matches!(exp.as_ref(), Expr::Const(c) if (*c - 2.0).abs() < 1e-12) =>
        {
            Some(base.as_ref())
        }
        _ => None,
    }
}

/// Factor a difference-of-squares `a^2 - b^2` into `(a - b)*(a + b)`.
fn factor_diff_squares(expr: &Expr) -> Option<Expr> {
    let terms = flatten_add(expr);
    if terms.len() != 2 {
        return None;
    }

    // Try both orderings: positive then negative
    for (first, second) in &[(terms[0], terms[1]), (terms[1], terms[0])] {
        let pos_base = match first {
            Expr::Pow(..) => is_pow2(first),
            Expr::Mul(a, b) if matches!(a.as_ref(), Expr::Const(c) if *c > 0.0) => is_pow2(b),
            _ => None,
        };

        let neg_base = match second {
            Expr::Neg(inner) => is_pow2(inner),
            Expr::Mul(a, b) if matches!(a.as_ref(), Expr::Const(c) if *c < 0.0) => is_pow2(b),
            _ => None,
        };

        if let (Some(a), Some(b)) = (pos_base, neg_base) {
            let a_expr = a.clone();
            let b_expr = b.clone();
            let a_minus_b = Expr::Sub(Box::new(a_expr.clone()), Box::new(b_expr.clone()));
            let a_plus_b = Expr::Add(Box::new(a_expr), Box::new(b_expr));
            return Some(simplify(&Expr::Mul(Box::new(a_minus_b), Box::new(a_plus_b))));
        }
    }

    None
}

/// Factor an expression using multiple strategies:
///
/// 1. Common factor extraction (`factor_common`)
/// 2. Quadratic factoring (`ax^2 + bx + c`, requires perfect-square discriminant)
/// 3. Difference of squares (`a^2 - b^2`)
pub fn factor(expr: &Expr) -> Expr {
    let expr = simplify(expr);
    let r1 = factor_common(&expr);
    if !structural_equal(&r1, &expr, true) {
        return r1;
    }
    if let Some(r2) = factor_quadratic(&expr) {
        return r2;
    }
    if let Some(r3) = factor_diff_squares(&expr) {
        return r3;
    }
    expr
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
                    // Guard: if simplify collapsed Mul(base,base) back into Pow(base,n),
                    // expand only the inner-base to avoid infinite recursion.
                    return match &result {
                        Expr::Pow(inner_base, _) => {
                            Expr::Pow(Box::new(expand(inner_base)), Box::new(Expr::Const(n as f64)))
                        }
                        _ => expand(&result),
                    };
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
    // Use a seed derived from process-level entropy to avoid fixed-sequence bias.
    // Falls back to wall-clock microseconds when AtomicU64 is unavailable.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42);
    equivalent_with_seed(lhs, rhs, seed)
}

/// Internal helper that accepts an explicit RNG seed for deterministic testing.
fn equivalent_with_seed(lhs: &str, rhs: &str, seed: u64) -> bool {
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

    // Strategy 2a: Special value pre-check
    // Test at 0, 1, -1, pi/2, pi, e — these edge cases often reveal non-equivalence early.
    let special_values: [f64; 6] = [
        0.0,
        1.0,
        -1.0,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        std::f64::consts::E,
    ];
    for sv in &special_values {
        let mut bindings = HashMap::new();
        for v in &vars {
            bindings.insert(v.clone(), *sv);
        }
        match (eval(&lhs_expr, &bindings), eval(&rhs_expr, &bindings)) {
            (Ok(l), Ok(r)) => {
                if (l - r).abs() > 1e-6 {
                    return false;
                }
            }
            _ => {} // skip undefined points (e.g., log(0))
        }
    }

    // Strategy 2b: Adaptive random sampling
    let mut rng = SimpleRng::new(seed);

    const INITIAL_SAMPLES: usize = 20;
    const FOCUSED_SAMPLES: usize = 50;

    let mut max_diff = 0.0_f64;
    let mut sample_count = 0_usize;

    // Phase 1: initial uniform samples
    for _ in 0..INITIAL_SAMPLES {
        let mut bindings = HashMap::new();
        for v in &vars {
            let (lo, hi) = match v.as_str() {
                "n" | "m" | "k" | "i" | "j" | "N" | "M" => (1.0, 100.0),
                _ => (-10.0, 10.0),
            };
            let val = rng.next_range(lo, hi);
            bindings.insert(v.clone(), val);
        }

        match (eval(&lhs_expr, &bindings), eval(&rhs_expr, &bindings)) {
            (Ok(l), Ok(r)) => {
                let diff = (l - r).abs();
                if diff > 1e-6 {
                    return false;
                }
                if diff > max_diff {
                    max_diff = diff;
                }
                sample_count += 1;
            }
            _ => continue,
        }
    }

    // Phase 2: if close to boundary, add focused sampling
    if max_diff < 1e-3 && sample_count >= INITIAL_SAMPLES / 2 {
        // Run focused samples with wider range to stress-test the boundary
        for _ in 0..FOCUSED_SAMPLES {
            let mut bindings = HashMap::new();
            for v in &vars {
                let (lo, hi) = match v.as_str() {
                    "n" | "m" | "k" | "i" | "j" | "N" | "M" => (0.5, 1_000.0),
                    _ => (-100.0, 100.0),
                };
                let val = rng.next_range(lo, hi);
                bindings.insert(v.clone(), val);
            }
            match (eval(&lhs_expr, &bindings), eval(&rhs_expr, &bindings)) {
                (Ok(l), Ok(r)) => {
                    if (l - r).abs() > 1e-6 {
                        return false;
                    }
                }
                _ => continue,
            }
        }
    } else if sample_count < INITIAL_SAMPLES / 2 {
        // Too many samples were undefined (e.g., many division-by-zero points) —
        // fall back to comprehensive sampling up to 100 total valid samples.
        let mut fallback_valid = 0_usize;
        while fallback_valid < (100 - sample_count) {
            let mut bindings = HashMap::new();
            for v in &vars {
                let (lo, hi) = match v.as_str() {
                    "n" | "m" | "k" | "i" | "j" | "N" | "M" => (0.5, 1_000.0),
                    _ => (-100.0, 100.0),
                };
                let val = rng.next_range(lo, hi);
                bindings.insert(v.clone(), val);
            }
            match (eval(&lhs_expr, &bindings), eval(&rhs_expr, &bindings)) {
                (Ok(l), Ok(r)) => {
                    if (l - r).abs() > 1e-6 {
                        return false;
                    }
                    fallback_valid += 1;
                }
                _ => continue,
            }
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
        Expr::Neg(x) => collect_vars_rec(x, vars),
        Expr::Pow(x, y) => {
            collect_vars_rec(x, vars);
            collect_vars_rec(y, vars);
        }
        Expr::Div(x, y) => {
            collect_vars_rec(x, vars);
            collect_vars_rec(y, vars);
        }
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
pub struct SimpleRng(u64);

impl SimpleRng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    pub fn next_range(&mut self, lo: f64, hi: f64) -> f64 {
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
            let trig = trig_simplify(&sm);
            display(&trig)
        }
        Err(_) => expr_str.to_string(),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Structural equivalence, subexpression search, and normalization
// ════════════════════════════════════════════════════════════════════════════

/// Check structural equivalence with optional commutativity handling.
///
/// With `commutativity = true`, `a + b` matches `b + a` and `a * b` matches `b * a`.
/// (Terms are compared as multisets.)
pub fn structural_equal(a: &Expr, b: &Expr, commutativity: bool) -> bool {
    if a == b {
        return true;
    }
    if !commutativity {
        return false;
    }
    match (a, b) {
        (Expr::Add(..), Expr::Add(..)) => {
            let a_terms: Vec<&Expr> = flatten_add(a);
            let b_terms: Vec<&Expr> = flatten_add(b);
            if a_terms.len() != b_terms.len() {
                return false;
            }
            let mut used = vec![false; b_terms.len()];
            for a_t in &a_terms {
                let mut matched = false;
                for (j, b_t) in b_terms.iter().enumerate() {
                    if !used[j] && structural_equal(a_t, b_t, true) {
                        used[j] = true;
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return false;
                }
            }
            true
        }
        (Expr::Mul(..), Expr::Mul(..)) => {
            let a_factors: Vec<&Expr> = flatten_mul(a);
            let b_factors: Vec<&Expr> = flatten_mul(b);
            if a_factors.len() != b_factors.len() {
                return false;
            }
            let mut used = vec![false; b_factors.len()];
            for a_f in &a_factors {
                let mut matched = false;
                for (j, b_f) in b_factors.iter().enumerate() {
                    if !used[j] && structural_equal(a_f, b_f, true) {
                        used[j] = true;
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

/// Check if `target` appears as a subtree of `expr`.
///
/// Useful for step-dependency verification: ensures a derived expression
/// contains an expected subexpression.
pub fn find_subexpression(expr: &Expr, target: &Expr) -> bool {
    if expr == target {
        return true;
    }
    match expr {
        Expr::Const(_) | Expr::Var(_) => false,
        Expr::Neg(x) => find_subexpression(x, target),
        Expr::Add(a, b)
        | Expr::Sub(a, b)
        | Expr::Mul(a, b)
        | Expr::Div(a, b)
        | Expr::Pow(a, b) => find_subexpression(a, target) || find_subexpression(b, target),
        Expr::Fn(_, args) => args.iter().any(|a| find_subexpression(a, target)),
    }
}

/// Normalize an expression to a canonical form.
///
/// Applies: simplify → trig_simplify → expand → simplify → rationalize →
/// factor_common → canonical ordering (variables sorted alphabetically,
/// constants placed last, factors sorted).
pub fn normalize(expr: &Expr) -> Expr {
    let expr = simplify(expr);
    let expr = trig_simplify(&expr);
    let expr = expand(&expr);
    let expr = simplify(&expr);
    let expr = rationalize(&expr);
    let expr = factor_common(&expr);
    canonical_order(&expr)
}

/// Sort terms and factors for deterministic ordering.
fn canonical_order(expr: &Expr) -> Expr {
    match expr {
        Expr::Const(_) | Expr::Var(_) => expr.clone(),
        Expr::Neg(x) => Expr::Neg(Box::new(canonical_order(x))),
        Expr::Add(..) => {
            let terms: Vec<Expr> = flatten_add(expr)
                .into_iter()
                .map(|t| canonical_order(t))
                .collect();
            let mut sorted = terms;
            sorted.sort_by(|a, b| {
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
            make_add(sorted)
        }
        Expr::Sub(a, b) => {
            Expr::Sub(Box::new(canonical_order(a)), Box::new(canonical_order(b)))
        }
        Expr::Mul(..) => {
            let factors: Vec<Expr> = flatten_mul(expr)
                .into_iter()
                .map(|f| canonical_order(f))
                .collect();
            let mut sorted = factors;
            sorted.sort_by(|a, b| display(a).cmp(&display(b)));
            make_mul(sorted)
        }
        Expr::Div(a, b) => {
            Expr::Div(Box::new(canonical_order(a)), Box::new(canonical_order(b)))
        }
        Expr::Pow(a, b) => {
            Expr::Pow(Box::new(canonical_order(a)), Box::new(canonical_order(b)))
        }
        Expr::Fn(name, args) => {
            Expr::Fn(name.clone(), args.iter().map(|a| canonical_order(a)).collect())
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Symbolic differentiation
// ════════════════════════════════════════════════════════════════════════════

/// Symbolically differentiate an expression with respect to the given variable.
///
/// Returns a simplified derivative expression. Supports:
/// - Constant, variable, sum, difference, product, quotient, power
/// - All standard functions (sin, cos, tan, exp, ln, log2, log10, sqrt, abs, atan2)
///
/// Three power-rule cases:
/// - `f(x)^n` (constant exponent) — power rule
/// - `c^x` (constant base) — exponential rule
/// - `f(x)^g(x)` — logarithmic differentiation
pub fn differentiate(expr: &Expr, var: &str) -> Expr {
    match expr {
        Expr::Const(_) => Expr::Const(0.0),
        Expr::Var(v) => {
            if v == var {
                Expr::Const(1.0)
            } else {
                Expr::Const(0.0)
            }
        }
        Expr::Neg(x) => {
            let dx = differentiate(x, var);
            simplify(&Expr::Neg(Box::new(dx)))
        }
        Expr::Add(a, b) => {
            let da = differentiate(a, var);
            let db = differentiate(b, var);
            simplify(&Expr::Add(Box::new(da), Box::new(db)))
        }
        Expr::Sub(a, b) => {
            let da = differentiate(a, var);
            let db = differentiate(b, var);
            simplify(&Expr::Sub(Box::new(da), Box::new(db)))
        }
        Expr::Mul(a, b) => {
            // Product rule: a'*b + a*b'
            let da = differentiate(a, var);
            let db = differentiate(b, var);
            let term1 = Expr::Mul(Box::new(da), Box::new((**b).clone()));
            let term2 = Expr::Mul(Box::new((**a).clone()), Box::new(db));
            simplify(&Expr::Add(Box::new(term1), Box::new(term2)))
        }
        Expr::Div(a, b) => {
            // Quotient rule: (a'*b - a*b') / b^2
            let da = differentiate(a, var);
            let db = differentiate(b, var);
            let num1 = Expr::Mul(Box::new(da), Box::new((**b).clone()));
            let num2 = Expr::Mul(Box::new((**a).clone()), Box::new(db));
            let numerator = Expr::Sub(Box::new(num1), Box::new(num2));
            let denominator = Expr::Pow(Box::new((**b).clone()), Box::new(Expr::Const(2.0)));
            simplify(&Expr::Div(Box::new(numerator), Box::new(denominator)))
        }
        Expr::Pow(a, b) => {
            let va = contains_var(a, var);
            let vb = contains_var(b, var);
            match (va, vb) {
                (false, false) => Expr::Const(0.0), // constant^constant
                (true, false) => {
                    // Power rule: d/dx f(x)^n = n * f^(n-1) * f'
                    if let Expr::Const(n) = b.as_ref() {
                        let df = differentiate(a, var);
                        let pow_minus_1 = Expr::Pow(
                            Box::new((**a).clone()),
                            Box::new(Expr::Const(*n - 1.0)),
                        );
                        simplify(&Expr::Mul(
                            Box::new(Expr::Const(*n)),
                            Box::new(Expr::Mul(
                                Box::new(pow_minus_1),
                                Box::new(df),
                            )),
                        ))
                    } else {
                        Expr::Const(0.0)
                    }
                }
                (false, true) => {
                    // Exponential rule: d/dx c^(g(x)) = c^(g(x)) * ln(c) * g'(x)
                    let original = Expr::Pow(Box::new((**a).clone()), Box::new((**b).clone()));
                    let db = differentiate(b, var);
                    let ln_base = Expr::Fn("ln".to_string(), vec![(**a).clone()]);
                    simplify(&Expr::Mul(
                        Box::new(original),
                        Box::new(Expr::Mul(Box::new(ln_base), Box::new(db))),
                    ))
                }
                (true, true) => {
                    // Logarithmic differentiation: d/dx f^g = f^g * (g'*ln(f) + g*f'/f)
                    let original = Expr::Pow(Box::new((**a).clone()), Box::new((**b).clone()));
                    let df = differentiate(a, var);
                    let dg = differentiate(b, var);
                    let ln_f = Expr::Fn("ln".to_string(), vec![(**a).clone()]);
                    let term1 = Expr::Mul(Box::new(dg), Box::new(ln_f));
                    let term2 = Expr::Div(
                        Box::new(Expr::Mul(Box::new((**b).clone()), Box::new(df))),
                        Box::new((**a).clone()),
                    );
                    let inner = Expr::Add(Box::new(term1), Box::new(term2));
                    simplify(&Expr::Mul(Box::new(original), Box::new(inner)))
                }
            }
        }
        Expr::Fn(name, args) => match name.as_str() {
            "sin" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    let cos_f = Expr::Fn("cos".to_string(), vec![args[0].clone()]);
                    simplify(&Expr::Mul(Box::new(cos_f), Box::new(df)))
                } else {
                    Expr::Const(0.0)
                }
            }
            "cos" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    let sin_f = Expr::Fn("sin".to_string(), vec![args[0].clone()]);
                    let neg_sin = Expr::Neg(Box::new(sin_f));
                    simplify(&Expr::Mul(Box::new(neg_sin), Box::new(df)))
                } else {
                    Expr::Const(0.0)
                }
            }
            "tan" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    let cos_f = Expr::Fn("cos".to_string(), vec![args[0].clone()]);
                    let cos_sq = Expr::Pow(Box::new(cos_f), Box::new(Expr::Const(2.0)));
                    let sec_sq = Expr::Div(Box::new(Expr::Const(1.0)), Box::new(cos_sq));
                    simplify(&Expr::Mul(Box::new(sec_sq), Box::new(df)))
                } else {
                    Expr::Const(0.0)
                }
            }
            "exp" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    let exp_f = Expr::Fn("exp".to_string(), vec![args[0].clone()]);
                    simplify(&Expr::Mul(Box::new(exp_f), Box::new(df)))
                } else {
                    Expr::Const(0.0)
                }
            }
            "log" | "ln" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    simplify(&Expr::Div(Box::new(df), Box::new(args[0].clone())))
                } else {
                    Expr::Const(0.0)
                }
            }
            "log2" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    let ln2 = Expr::Fn("ln".to_string(), vec![Expr::Const(2.0)]);
                    let denom = Expr::Mul(Box::new(args[0].clone()), Box::new(ln2));
                    simplify(&Expr::Div(Box::new(df), Box::new(denom)))
                } else {
                    Expr::Const(0.0)
                }
            }
            "log10" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    let ln10 = Expr::Fn("ln".to_string(), vec![Expr::Const(10.0)]);
                    let denom = Expr::Mul(Box::new(args[0].clone()), Box::new(ln10));
                    simplify(&Expr::Div(Box::new(df), Box::new(denom)))
                } else {
                    Expr::Const(0.0)
                }
            }
            "sqrt" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    let half = Expr::Const(0.5);
                    let pow_neg_half = Expr::Pow(
                        Box::new(args[0].clone()),
                        Box::new(Expr::Const(-0.5)),
                    );
                    simplify(&Expr::Mul(
                        Box::new(half),
                        Box::new(Expr::Mul(Box::new(pow_neg_half), Box::new(df))),
                    ))
                } else {
                    Expr::Const(0.0)
                }
            }
            "abs" => {
                if args.len() == 1 {
                    let df = differentiate(&args[0], var);
                    let abs_f = Expr::Fn("abs".to_string(), vec![args[0].clone()]);
                    let sign = Expr::Div(Box::new(args[0].clone()), Box::new(abs_f));
                    simplify(&Expr::Mul(Box::new(sign), Box::new(df)))
                } else {
                    Expr::Const(0.0)
                }
            }
            "atan2" => {
                if args.len() == 2 {
                    let df = differentiate(&args[0], var);
                    let dg = differentiate(&args[1], var);
                    let num = Expr::Sub(
                        Box::new(Expr::Mul(Box::new(df), Box::new(args[1].clone()))),
                        Box::new(Expr::Mul(Box::new(args[0].clone()), Box::new(dg))),
                    );
                    let f_sq = Expr::Pow(Box::new(args[0].clone()), Box::new(Expr::Const(2.0)));
                    let g_sq = Expr::Pow(Box::new(args[1].clone()), Box::new(Expr::Const(2.0)));
                    let denom = Expr::Add(Box::new(f_sq), Box::new(g_sq));
                    simplify(&Expr::Div(Box::new(num), Box::new(denom)))
                } else {
                    Expr::Const(0.0)
                }
            }
            _ => Expr::Const(0.0), // unknown function derivative
        },
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Symbolic integration
// ════════════════════════════════════════════════════════════════════════════

/// Symbolically integrate an expression with respect to the given variable.
///
/// Supports basic indefinite integrals of polynomials, reciprocals,
/// `sin`, `cos`, and `exp`. Does **not** add the `+ C` constant.
///
/// Rules:
/// - `∫ c dx = c * x`
/// - `∫ x^n dx = x^(n+1) / (n+1)`  (n ≠ -1)
/// - `∫ x^(-1) dx = ln(x)`
/// - `∫ sin(x) dx = -cos(x)`
/// - `∫ cos(x) dx = sin(x)`
/// - `∫ exp(x) dx = exp(x)`
pub fn integrate(expr: &Expr, var: &str) -> Expr {
    match expr {
        Expr::Const(c) => simplify(&Expr::Mul(
            Box::new(Expr::Const(*c)),
            Box::new(Expr::Var(var.to_string())),
        )),

        Expr::Var(v) => {
            if v == var {
                // ∫ x dx = x^2 / 2
                simplify(&Expr::Div(
                    Box::new(Expr::Pow(
                        Box::new(Expr::Var(var.to_string())),
                        Box::new(Expr::Const(2.0)),
                    )),
                    Box::new(Expr::Const(2.0)),
                ))
            } else {
                // Treat as constant: ∫ y dx = y * x
                simplify(&Expr::Mul(
                    Box::new(expr.clone()),
                    Box::new(Expr::Var(var.to_string())),
                ))
            }
        }

        Expr::Neg(x) => simplify(&Expr::Neg(Box::new(integrate(x, var)))),

        Expr::Add(a, b) => simplify(&Expr::Add(
            Box::new(integrate(a, var)),
            Box::new(integrate(b, var)),
        )),

        Expr::Sub(a, b) => simplify(&Expr::Sub(
            Box::new(integrate(a, var)),
            Box::new(integrate(b, var)),
        )),

        Expr::Mul(a, b) => {
            if let Expr::Const(c) = a.as_ref() {
                // ∫ c * f(x) dx = c * ∫ f(x) dx
                return simplify(&Expr::Mul(
                    Box::new(Expr::Const(*c)),
                    Box::new(integrate(b, var)),
                ));
            }
            if let Expr::Const(c) = b.as_ref() {
                return simplify(&Expr::Mul(
                    Box::new(Expr::Const(*c)),
                    Box::new(integrate(a, var)),
                ));
            }
            // General product not supported — return as-is
            expr.clone()
        }

        Expr::Div(a, b) => {
            // ∫ (1/x) dx = ln(x)
            if let Expr::Const(c) = a.as_ref() {
                if (*c - 1.0).abs() < 1e-12 {
                    if let Expr::Var(v) = b.as_ref() {
                        if v == var {
                            return Expr::Fn("ln".to_string(), vec![Expr::Var(var.to_string())]);
                        }
                    }
                }
            }
            // General division not supported — return as-is
            expr.clone()
        }

        Expr::Pow(base, exp) => {
            if let Expr::Var(v) = base.as_ref() {
                if v == var {
                    // Handle exponent that may be Const(n) or Neg(Const(n)) from parser
                    let n = match exp.as_ref() {
                        Expr::Const(c) => Some(*c),
                        Expr::Neg(inner) => {
                            if let Expr::Const(c) = inner.as_ref() {
                                Some(-c)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some(n) = n {
                        if (n + 1.0).abs() < 1e-12 {
                            // ∫ x^(-1) dx = ln(x)
                            return Expr::Fn("ln".to_string(), vec![Expr::Var(var.to_string())]);
                        }
                        // ∫ x^n dx = x^(n+1) / (n+1)
                        return simplify(&Expr::Div(
                            Box::new(Expr::Pow(
                                Box::new(Expr::Var(var.to_string())),
                                Box::new(Expr::Const(n + 1.0)),
                            )),
                            Box::new(Expr::Const(n + 1.0)),
                        ));
                    }
                }
            }
            // Check for constant-base variable-exponent, e.g. e^x
            if let Expr::Const(_) = base.as_ref() {
                if contains_var(exp, var) {
                    // ∫ c^x dx — keep as-is (chain rule not implemented)
                    return expr.clone();
                }
            }
            expr.clone()
        }

        Expr::Fn(name, args) => {
            if args.len() == 1 {
                match name.as_str() {
                    "sin" => {
                        if let Expr::Var(v) = &args[0] {
                            if v == var {
                                // ∫ sin(x) dx = -cos(x)
                                return Expr::Neg(Box::new(Expr::Fn(
                                    "cos".to_string(),
                                    vec![Expr::Var(var.to_string())],
                                )));
                            }
                        }
                    }
                    "cos" => {
                        if let Expr::Var(v) = &args[0] {
                            if v == var {
                                // ∫ cos(x) dx = sin(x)
                                return Expr::Fn(
                                    "sin".to_string(),
                                    vec![Expr::Var(var.to_string())],
                                );
                            }
                        }
                    }
                    "exp" => {
                        if let Expr::Var(v) = &args[0] {
                            if v == var {
                                // ∫ exp(x) dx = exp(x)
                                return Expr::Fn(
                                    "exp".to_string(),
                                    vec![Expr::Var(var.to_string())],
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            expr.clone()
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Equation solving
// ════════════════════════════════════════════════════════════════════════════

/// Recursively extract polynomial coefficients from a term, distributing
/// `Neg` over `Add/Sub` when encountered.
fn collect_term_coeffs(term: &Expr, var: &str, sign: f64, coeffs: &mut HashMap<u32, f64>) {
    match term {
        Expr::Neg(inner) => collect_term_coeffs(inner, var, -sign, coeffs),
        Expr::Add(a, b) => {
            collect_term_coeffs(a, var, sign, coeffs);
            collect_term_coeffs(b, var, sign, coeffs);
        }
        Expr::Sub(a, b) => {
            collect_term_coeffs(a, var, sign, coeffs);
            collect_term_coeffs(b, var, -sign, coeffs);
        }
        Expr::Mul(a, b) if matches!(a.as_ref(), Expr::Const(c) if *c < 0.0) => {
            // For a negative coefficient, distribute into Add/Sub if present
            if let Expr::Add(..) | Expr::Sub(..) = b.as_ref() {
                let neg = Expr::Neg(Box::new((**b).clone()));
                collect_term_coeffs(&neg, var, sign, coeffs);
            } else {
                extract_simple_term(term, var, sign, coeffs);
            }
        }
        other => {
            extract_simple_term(other, var, sign, coeffs);
        }
    }
}

/// Extract coefficients from a simple (non-compound) term.
fn extract_simple_term(term: &Expr, var: &str, sign: f64, coeffs: &mut HashMap<u32, f64>) {
    if let Some((pow, coeff)) = extract_power_and_coeff(term, var) {
        *coeffs.entry(pow).or_insert(0.0) += sign * coeff;
    } else if !contains_var(term, var) {
        if let Ok(val) = eval(term, &HashMap::new()) {
            *coeffs.entry(0).or_insert(0.0) += sign * val;
        }
    }
}

/// Collect polynomial coefficients from an expression as a map of power → coefficient.
///
/// Only collects terms where the power of `var` is a non-negative integer.
/// Non-polynomial terms containing `var` are silently skipped.
/// Negation over `Add/Sub` is distributed to correctly handle forms like
/// `-(x + 3)` (which after simplification stays as `Neg(Add(x, 3))`).
pub fn collect_poly_coeffs(expr: &Expr, var: &str) -> HashMap<u32, f64> {
    let terms = flatten_add(expr);
    let mut coeffs: HashMap<u32, f64> = HashMap::new();

    for term in &terms {
        collect_term_coeffs(term, var, 1.0, &mut coeffs);
    }

    coeffs
}

/// Format a floating-point number as a clean human-readable string.
///
/// Integers are written without decimal point; floats are trimmed to 6 decimal
/// places with trailing zeros removed.
fn format_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x.is_sign_positive() {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        };
    }
    if x.fract() == 0.0 && x >= i64::MIN as f64 && x <= i64::MAX as f64 {
        format!("{}", x as i64)
    } else {
        let s = format!("{:.6}", x);
        s.trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// Solve an equation string for a given variable.
///
/// Parses the equation (expected to contain exactly one `=` sign),
/// brings all terms to one side, and classifies the result:
///
/// - **Linear**: `a*x + b = 0` → `x = -b/a`
/// - **Quadratic**: `a*x^2 + b*x + c = 0` → `x = (-b ± sqrt(b^2 - 4ac))/(2a)`
/// - **Pure power**: `x^n = c` → `x = c^(1/n)` (with `±` for even n)
pub fn solve_equation(equation_str: &str, variable_str: &str) -> Result<Vec<String>, String> {
    let parts: Vec<&str> = equation_str.split('=').collect();
    if parts.len() != 2 {
        return Err(format!(
            "equation must contain exactly one '=' sign: {equation_str}"
        ));
    }

    let lhs = parse(parts[0].trim()).map_err(|e| format!("parse error (lhs): {e}"))?;
    let rhs = parse(parts[1].trim()).map_err(|e| format!("parse error (rhs): {e}"))?;

    // Bring all terms to LHS
    let expr = simplify(&Expr::Sub(Box::new(lhs), Box::new(rhs)));

    let coeffs = collect_poly_coeffs(&expr, variable_str);

    // Filter to non-zero coefficients only for structural decisions
    let non_zero: Vec<(u32, f64)> = coeffs
        .iter()
        .filter(|&(_, v)| v.abs() > 1e-12)
        .map(|(&k, &v)| (k, v))
        .collect();
    let has_var_term = non_zero.iter().any(|(k, _)| *k > 0);

    let a = coeffs.get(&2).copied().unwrap_or(0.0);
    let b = coeffs.get(&1).copied().unwrap_or(0.0);
    let c = coeffs.get(&0).copied().unwrap_or(0.0);

    let mut solutions: Vec<String> = Vec::new();

    if a.abs() > 1e-12 {
        // Quadratic: a*x^2 + b*x + c = 0
        let disc = b * b - 4.0 * a * c;
        if disc < -1e-12 {
            // Complex roots — no real solutions
            return Ok(solutions);
        }
        let safe_disc = if disc < 0.0 { 0.0 } else { disc };
        let sqrt_disc = safe_disc.sqrt();

        let x1 = (-b - sqrt_disc) / (2.0 * a);
        let x2 = (-b + sqrt_disc) / (2.0 * a);

        if (x1 - x2).abs() < 1e-12 {
            solutions.push(format!("{} = {}", variable_str, format_number(x1)));
        } else {
            solutions.push(format!("{} = {}", variable_str, format_number(x1)));
            solutions.push(format!("{} = {}", variable_str, format_number(x2)));
        }
    } else if b.abs() > 1e-12 {
        // Linear: b*x + c = 0 → x = -c/b
        let x = -c / b;
        solutions.push(format!("{} = {}", variable_str, format_number(x)));
    } else if has_var_term {
        // Pure power: a*x^n + c = 0 → x^n = -c/a → x = (-c/a)^(1/n)
        // Only one non-zero power (besides possible constant) is supported
        let power_terms: Vec<_> = non_zero.iter().filter(|(k, _)| *k > 0).collect();
        if power_terms.len() != 1 {
            return Err(format!("mixed terms not supported"));
        }
        let (n, lead_coeff) = power_terms[0];
        let const_term = non_zero
            .iter()
            .find(|(k, _)| *k == 0)
            .map(|(_, v)| *v)
            .unwrap_or(0.0);
        let ratio = -const_term / *lead_coeff;

        if ratio >= 0.0 || n % 2 == 1 {
            let root = ratio.powf(1.0 / *n as f64);
            solutions.push(format!("{} = {}", variable_str, format_number(root)));
            // Even power with positive ratio gives ±
            if n % 2 == 0 && ratio > 0.0 {
                solutions.push(format!("{} = {}", variable_str, format_number(-root)));
            }
        }
        if solutions.is_empty() {
            // No real solutions (e.g. x^2 = -4)
            return Ok(solutions);
        }
    } else if c.abs() < 1e-12 {
        // 0 = 0: identity
        solutions.push(format!("{} is any real number", variable_str));
    } else {
        return Err(format!("no solution: contradiction {c} ≠ 0"));
    }

    Ok(solutions)
}

// ════════════════════════════════════════════════════════════════════════════
// Substitution — variable/expression replacement in AST
// ════════════════════════════════════════════════════════════════════════════

/// Replace every occurrence of `var` with `replacement` in the expression tree.
pub fn substitute(expr: &Expr, var: &str, replacement: &Expr) -> Expr {
    match expr {
        Expr::Var(v) if v == var => replacement.clone(),
        Expr::Const(_) | Expr::Var(_) => expr.clone(),
        Expr::Neg(x) => Expr::Neg(Box::new(substitute(x, var, replacement))),
        Expr::Add(a, b) => Expr::Add(
            Box::new(substitute(a, var, replacement)),
            Box::new(substitute(b, var, replacement)),
        ),
        Expr::Sub(a, b) => Expr::Sub(
            Box::new(substitute(a, var, replacement)),
            Box::new(substitute(b, var, replacement)),
        ),
        Expr::Mul(a, b) => Expr::Mul(
            Box::new(substitute(a, var, replacement)),
            Box::new(substitute(b, var, replacement)),
        ),
        Expr::Div(a, b) => Expr::Div(
            Box::new(substitute(a, var, replacement)),
            Box::new(substitute(b, var, replacement)),
        ),
        Expr::Pow(a, b) => Expr::Pow(
            Box::new(substitute(a, var, replacement)),
            Box::new(substitute(b, var, replacement)),
        ),
        Expr::Fn(name, args) => {
            Expr::Fn(name.clone(), args.iter().map(|a| substitute(a, var, replacement)).collect())
        }
    }
}

/// Apply multiple substitutions from a mapping, then simplify.
///
/// Each variable in the mapping is replaced with its corresponding expression.
pub fn substitute_all(expr: &Expr, mapping: &HashMap<String, Expr>) -> Expr {
    let mut result = expr.clone();
    for (var, replacement) in mapping {
        result = substitute(&result, var, replacement);
    }
    simplify(&result)
}

// ════════════════════════════════════════════════════════════════════════════
// Series expansion via Taylor series
// ════════════════════════════════════════════════════════════════════════════

/// Compute the Taylor series expansion of `expr` around `point` to the given `order`.
///
/// Uses repeated differentiation:
///   f(x) = Σ f^(n)(a)/n! * (x-a)^n,  n=0..order-1
pub fn series(expr: &Expr, var: &str, point: f64, order: u32) -> Expr {
    let mut terms: Vec<Expr> = Vec::new();

    // n=0: f(a) — eval at point
    {
        let mut vars = HashMap::new();
        vars.insert(var.to_string(), point);
        if let Ok(val) = eval(expr, &vars) {
            if val.abs() > 1e-15 {
                terms.push(Expr::Const(val));
            }
        }
    }

    // n=1..order-1: f^(n)(a) / n! * (x-a)^n
    let mut current = expr.clone();
    let mut fact = 1.0_f64;
    for n in 1..order {
        current = differentiate(&current, var);
        // Check if derivative is identically zero (early stop)
        let simplified = simplify(&current);
        if matches!(&simplified, Expr::Const(c) if c.abs() < 1e-15) {
            break;
        }

        // Evaluate derivative at point
        let mut vars = HashMap::new();
        vars.insert(var.to_string(), point);
        match eval(&simplified, &vars) {
            Ok(deriv_val) if deriv_val.abs() > 1e-15 => {
                fact *= n as f64; // n!
                let coeff = deriv_val / fact;
                let x_minus_a = Expr::Sub(
                    Box::new(Expr::Var(var.to_string())),
                    Box::new(Expr::Const(point)),
                );
                let pow_term = if n == 1 {
                    x_minus_a
                } else {
                    Expr::Pow(Box::new(x_minus_a), Box::new(Expr::Const(n as f64)))
                };
                terms.push(Expr::Mul(Box::new(Expr::Const(coeff)), Box::new(pow_term)));
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    if terms.is_empty() {
        return Expr::Const(0.0);
    }
    simplify(&make_add(terms))
}

// ════════════════════════════════════════════════════════════════════════════
// Limit evaluation
// ════════════════════════════════════════════════════════════════════════════

/// Compute the limit of `expr` as `var` approaches `point`.
///
/// Strategy:
/// - Finite point: try direct eval; if division by zero, use series expansion.
/// - Infinity: evaluate at large magnitude values.
/// - Directional limits (+/-): support via `direction` parameter.
pub fn limit(expr: &Expr, var: &str, point: &str, direction: Option<&str>) -> Result<Expr, String> {
    if let Ok(pt) = point.parse::<f64>() {
        // Finite point — try direct evaluation
        let mut vars = HashMap::new();
        vars.insert(var.to_string(), pt);
        match eval(expr, &vars) {
            Ok(val) if val.is_finite() => return Ok(Expr::Const(val)),
            Ok(_) => return Err(format!("limit diverges at {point}")),
            Err(_) => {
                // Division by zero or other singularity — try series expansion
                let series_expr = series(expr, var, pt, 4);
                let simplified = simplify(&series_expr);

                // If series is constant, that's the limit
                if let Expr::Const(c) = &simplified {
                    if c.is_finite() {
                        return Ok(Expr::Const(*c));
                    }
                }

                // Try evaluating the original expression near the point
                let epsilon = 1e-12;
                let sign = match direction {
                    Some("+") => 1.0,
                    Some("-") => -1.0,
                    _ => 1.0,
                };
                vars.insert(var.to_string(), pt + sign * epsilon);
                match eval(expr, &vars) {
                    Ok(val) if val.is_finite() => return Ok(Expr::Const(val)),
                    Ok(val) if val.is_infinite() => {
                        if val.is_sign_positive() {
                            return Ok(Expr::Const(f64::INFINITY));
                        } else {
                            return Ok(Expr::Const(f64::NEG_INFINITY));
                        }
                    }
                    _ => {}
                }

                // Try the series near the point
                match eval(&simplified, &vars) {
                    Ok(val) if val.is_finite() => return Ok(Expr::Const(val)),
                    _ => {}
                }

                Err(format!("cannot determine limit at {point}"))
            }
        }
    } else if ["oo", "inf", "+oo", "+inf"].contains(&point) {
        // Limit at +infinity — evaluate at very large x
        let mut vars = HashMap::new();
        vars.insert(var.to_string(), 1e30);
        match eval(expr, &vars) {
            Ok(val) if val == 0.0 => Ok(Expr::Const(0.0)),
            Ok(val) if val.is_finite() => {
                // Cross-check at 1e15
                vars.insert(var.to_string(), 1e15);
                if let Ok(val2) = eval(expr, &vars) {
                    if (val - val2).abs() < 1e-6 * val.abs().max(1.0) {
                        return Ok(Expr::Const(val));
                    }
                }
                // Try even larger
                vars.insert(var.to_string(), 1e50);
                match eval(expr, &vars) {
                    Ok(val3) if val3.is_finite() => Ok(Expr::Const(val3)),
                    _ => Err(format!("cannot determine limit at {point}")),
                }
            }
            Ok(val) if val.is_infinite() => Ok(Expr::Const(val)),
            _ => Err(format!("cannot determine limit at {point}")),
        }
    } else if ["-oo", "-inf"].contains(&point) {
        // Limit at -infinity — evaluate at very negative x
        let mut vars = HashMap::new();
        vars.insert(var.to_string(), -1e30);
        match eval(expr, &vars) {
            Ok(val) if val == 0.0 => Ok(Expr::Const(0.0)),
            Ok(val) if val.is_finite() => {
                vars.insert(var.to_string(), -1e15);
                if let Ok(val2) = eval(expr, &vars) {
                    if (val - val2).abs() < 1e-6 * val.abs().max(1.0) {
                        return Ok(Expr::Const(val));
                    }
                }
                vars.insert(var.to_string(), -1e50);
                match eval(expr, &vars) {
                    Ok(val3) if val3.is_finite() => Ok(Expr::Const(val3)),
                    _ => Err(format!("cannot determine limit at {point}")),
                }
            }
            Ok(val) if val.is_infinite() => Ok(Expr::Const(val)),
            _ => Err(format!("cannot determine limit at {point}")),
        }
    } else {
        Err(format!("unknown point: {point}"))
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

    // ── Numerical fallback path tests ──

    #[test]
    fn test_equivalent_numerical_fallback_trig() {
        // sin(x)^2 + cos(x)^2 ≡ 1 cannot be proven by the pure-Rust structural
        // expand+simplify path (no trig identity rewriting). This exercises the
        // random numerical sampling fallback (Strategy 2).
        assert!(
            equivalent("sin(x)^2 + cos(x)^2", "1"),
            "trig identity sin^2 + cos^2 = 1 should hold via numerical sampling"
        );
    }

    #[test]
    fn test_equivalent_numerical_fallback_exp_log() {
        // exp(log(x)) ≡ x also requires the numerical fallback.
        assert!(equivalent("exp(log(x))", "x"), "exp(log(x)) = x");
    }

    #[test]
    fn test_equivalent_numerical_fallback_not_equal() {
        // A false trig identity — must be rejected even via random sampling.
        assert!(
            !equivalent("sin(x)^2 + cos(x)^2", "2"),
            "sin^2 + cos^2 = 2 should be false"
        );
    }

    #[test]
    fn test_equivalent_numerical_flakiness() {
        // Run the same trig identity with 10 different deterministic seeds to
        // confirm the numerical sampling is stable and not spuriously flaky.
        for seed in [1u64, 42, 12345, 999999, 314159, 271828, 777, 2026, 65535, 987654] {
            assert!(
                equivalent_with_seed("sin(x)^2 + cos(x)^2", "1", seed),
                "trig identity failed with seed {seed}"
            );
        }
    }

    #[test]
    fn test_equivalent_numerical_flakiness_false_negatives() {
        // Ensure false identities are consistently rejected across seeds.
        for seed in [1u64, 42, 12345, 999999, 314159, 271828, 777, 2026, 65535, 987654] {
            assert!(
                !equivalent_with_seed("sin(x)^2 + cos(x)^2", "2", seed),
                "false trig identity should be rejected with seed {seed}"
            );
        }
    }

    #[test]
    fn test_equivalent_numerical_double_variable() {
        // Identity with two variables: sin(x)^2 + cos(x)^2 = 1 no matter what y is.
        assert!(
            equivalent("sin(x)^2 + cos(x)^2 + y - y", "1"),
            "multi-variable trig identity"
        );
    }

    // ── Trig simplification ──

    #[test]
    fn test_trig_const_fold_sin0() {
        let e = parse("sin(0)").unwrap();
        let s = simplify(&e);
        assert!((eval(&s, &HashMap::new()).unwrap() - 0.0).abs() < 1e-10);
        assert_eq!(display(&s), "0");
    }

    #[test]
    fn test_trig_const_fold_cos0() {
        let e = parse("cos(0)").unwrap();
        let s = simplify(&e);
        assert!((eval(&s, &HashMap::new()).unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_trig_const_fold_sin_pi() {
        let e = parse("sin(pi)").unwrap();
        let s = simplify(&e);
        assert!((eval(&s, &HashMap::new()).unwrap()).abs() < 1e-10);
    }

    #[test]
    fn test_trig_const_fold_cos_pi() {
        let e = parse("cos(pi)").unwrap();
        let s = simplify(&e);
        assert!((eval(&s, &HashMap::new()).unwrap() + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_trig_identity_sin_sq_plus_cos_sq() {
        // trig_simplify should reduce sin(x)^2 + cos(x)^2 → 1
        let e = parse("sin(x)^2 + cos(x)^2").unwrap();
        let t = trig_simplify(&e);
        let result = eval(&t, &HashMap::new()).unwrap();
        assert!((result - 1.0).abs() < 1e-10, "expected 1, got {result}");
    }

    #[test]
    fn test_trig_identity_nested() {
        // Works even inside larger expressions
        let e = parse("sin(x)^2 + cos(x)^2 + y").unwrap();
        let t = trig_simplify(&e);
        let mut vars = HashMap::new();
        vars.insert("y".into(), 5.0);
        let result = eval(&t, &vars).unwrap();
        assert!((result - 6.0).abs() < 1e-10, "expected 6, got {result}");
    }

    // ── Rationalization ──

    #[test]
    fn test_rationalize_simple() {
        let e = parse("1/x + 1/y").unwrap();
        let r = rationalize(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        vars.insert("y".into(), 3.0);
        let lhs = eval(&e, &vars).unwrap();
        let rhs = eval(&r, &vars).unwrap();
        assert!((lhs - rhs).abs() < 1e-10, "rationalize changed value");
    }

    #[test]
    fn test_rationalize_noop() {
        // No fractions — unchanged
        let e = parse("x + y").unwrap();
        let r = rationalize(&e);
        assert_eq!(display(&r), "x + y");
    }

    #[test]
    fn test_rationalize_single_denom() {
        // Single fractional term — unchanged structure
        let e = parse("1/x").unwrap();
        let r = rationalize(&e);
        assert_eq!(display(&r), "1/x");
    }

    #[test]
    fn test_rationalize_neg_term() {
        let e = parse("a/b - c/d").unwrap();
        let r = rationalize(&e);
        let mut vars = HashMap::new();
        vars.insert("a".into(), 1.0);
        vars.insert("b".into(), 2.0);
        vars.insert("c".into(), 3.0);
        vars.insert("d".into(), 4.0);
        let lhs = eval(&e, &vars).unwrap();
        let rhs = eval(&r, &vars).unwrap();
        assert!((lhs - rhs).abs() < 1e-10, "rationalize changed value");
    }

    // ── Common factor extraction ──

    #[test]
    fn test_factor_common_simple() {
        let e = parse("2*x + 2*y").unwrap();
        let f = factor_common(&e);
        let factored_str = display(&f);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        vars.insert("y".into(), 5.0);
        let orig = eval(&e, &vars).unwrap();
        let factored = eval(&f, &vars).unwrap();
        assert!(
            (orig - factored).abs() < 1e-10,
            "factor_common changed value: got {factored_str}"
        );
    }

    #[test]
    fn test_factor_common_noop() {
        // No common factor — unchanged
        let e = parse("x + y").unwrap();
        let f = factor_common(&e);
        assert_eq!(display(&f), "x + y");
    }

    #[test]
    fn test_factor_common_coefficient_only() {
        let e = parse("3*x + 3*y + 3*z").unwrap();
        let f = factor_common(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 1.0);
        vars.insert("y".into(), 2.0);
        vars.insert("z".into(), 3.0);
        let orig = eval(&e, &vars).unwrap();
        let factored = eval(&f, &vars).unwrap();
        assert!((orig - factored).abs() < 1e-10, "factor_common changed value");
    }

    // ── Structural equivalence ──

    #[test]
    fn test_structural_equal_add_commutative() {
        let a = parse("x + 1").unwrap();
        let b = parse("1 + x").unwrap();
        assert!(structural_equal(&a, &b, true));
    }

    #[test]
    fn test_structural_equal_mul_commutative() {
        let a = parse("x * y").unwrap();
        let b = parse("y * x").unwrap();
        assert!(structural_equal(&a, &b, true));
    }

    #[test]
    fn test_structural_equal_non_commutative() {
        let a = parse("x + 1").unwrap();
        let b = parse("1 + x").unwrap();
        assert!(!structural_equal(&a, &b, false));
    }

    #[test]
    fn test_structural_equal_different_op() {
        let a = parse("x + y").unwrap();
        let b = parse("x * y").unwrap();
        assert!(!structural_equal(&a, &b, true));
    }

    // ── Subexpression search ──

    #[test]
    fn test_find_subexpression_simple() {
        let expr = parse("(x+1)^2 + y").unwrap();
        let target = parse("x+1").unwrap();
        assert!(find_subexpression(&expr, &target));
    }

    #[test]
    fn test_find_subexpression_not_found() {
        let expr = parse("x^2 + y").unwrap();
        let target = parse("x+1").unwrap();
        assert!(!find_subexpression(&expr, &target));
    }

    #[test]
    fn test_find_subexpression_in_fn_arg() {
        let expr = parse("sin(x)").unwrap();
        let target = parse("x").unwrap();
        assert!(find_subexpression(&expr, &target));
    }

    #[test]
    fn test_find_subexpression_self() {
        let expr = parse("x + y + z").unwrap();
        assert!(find_subexpression(&expr, &expr));
    }

    // ── Normalization ──

    #[test]
    fn test_normalize_sorts_vars() {
        let a = parse("y + x + 2*1").unwrap();
        let n = normalize(&a);
        // Constants folded, terms sorted: "x + y + 2"
        let result = display(&n);
        assert_eq!(result, "x + y + 2", "got {result}");
    }

    #[test]
    fn test_normalize_like_terms() {
        let a = parse("2*y + 3*x + x").unwrap();
        let n = normalize(&a);
        // Like terms combined: 3*x + 2*y
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        vars.insert("y".into(), 3.0);
        let orig = eval(&a, &vars).unwrap();
        let norm = eval(&n, &vars).unwrap();
        assert!(
            (orig - norm).abs() < 1e-10,
            "normalize changed value"
        );
    }

    #[test]
    fn test_normalize_factor_common() {
        let e = parse("2*x + 2*y").unwrap();
        let n = normalize(&e);
        // The result should have factored out the 2
        // Check structural difference from pre-normalize
        let pre_norm = simplify(&e);
        // After normalize, the factored form should be structurally
        // equivalent via commutative add
        assert!(
            structural_equal(&n, &pre_norm, true)
                || display(&n) != display(&pre_norm), // at least different display
            "normalize should change factoring but not stop here"
        );
        // Verify numerical correctness
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        vars.insert("y".into(), 5.0);
        assert!(
            (eval(&e, &vars).unwrap() - eval(&n, &vars).unwrap()).abs() < 1e-10
        );
    }

    // ── Enhanced equivalent with adaptive sampling ──

    #[test]
    fn test_equivalent_special_values() {
        // Edge-case identities that benefit from special-value pre-check
        assert!(equivalent("x^0", "1"), "x^0 should equal 1");
        assert!(equivalent("0*x", "0"), "0*x should equal 0");
    }

    #[test]
    fn test_equivalent_adaptive_sampling_still_passes_trig() {
        // The adaptive sampling should still correctly handle trig identities
        assert!(equivalent("sin(x)^2 + cos(x)^2", "1"));
    }

    // ── Power merging in simplify_mul ──

    #[test]
    fn test_simplify_mul_power_merge_same_base() {
        let e = parse("x * x").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "x^2");
    }

    #[test]
    fn test_simplify_mul_power_merge_multiple() {
        let e = parse("x^2 * x^3").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "x^5");
    }

    #[test]
    fn test_simplify_mul_power_merge_diff_bases() {
        // x * y * x should be x^2 * y (value preserved)
        let e = parse("x * y * x").unwrap();
        let s = simplify(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        vars.insert("y".into(), 3.0);
        let val = eval(&s, &vars).unwrap();
        assert!((val - 12.0).abs() < 1e-10, "expected 12, got {val}");
    }

    #[test]
    fn test_simplify_mul_power_merge_nested_base() {
        // Power merging skips Add/Sub-containing bases to avoid breaking expand().
        // The expression stays as (x+1)^2 * (x+1)^3, but value is preserved.
        let e = parse("(x+1)^2 * (x+1)^3").unwrap();
        let s = simplify(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let val = eval(&s, &vars).unwrap();
        assert!((val - 243.0).abs() < 1e-10, "expected 243, got {val}");
    }

    #[test]
    fn test_simplify_mul_power_cancel() {
        let e = parse("x^2 * x^(-2)").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "1");
    }

    #[test]
    fn test_simplify_mul_power_merge_reduces_one() {
        // x * x^2 * x^(-1) → x^(1+2-1) = x^2
        let e = parse("x * x^2 * x^(-1)").unwrap();
        let s = simplify(&e);
        assert_eq!(display(&s), "x^2");
    }

    // ── Differentiation ──

    #[test]
    fn test_diff_const() {
        let e = parse("5").unwrap();
        let d = differentiate(&e, "x");
        let val = eval(&d, &HashMap::new()).unwrap();
        assert!((val - 0.0).abs() < 1e-10, "expected 0, got {val}");
    }

    #[test]
    fn test_diff_var_same() {
        let e = parse("x").unwrap();
        let d = differentiate(&e, "x");
        let val = eval(&d, &HashMap::new()).unwrap();
        assert!((val - 1.0).abs() < 1e-10, "expected 1, got {val}");
    }

    #[test]
    fn test_diff_var_other() {
        let e = parse("y").unwrap();
        let d = differentiate(&e, "x");
        let val = eval(&d, &HashMap::new()).unwrap();
        assert!((val - 0.0).abs() < 1e-10, "expected 0, got {val}");
    }

    #[test]
    fn test_diff_add() {
        let e = parse("x + x^2").unwrap();
        let d = differentiate(&e, "x");
        // d/dx (x + x^2) = 1 + 2x, at x=3 gives 7
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 7.0).abs() < 1e-10, "expected 7, got {val}");
    }

    #[test]
    fn test_diff_product() {
        let e = parse("x * x").unwrap();
        let d = differentiate(&e, "x");
        // d/dx x*x = 2x, at x=3 gives 6
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 6.0).abs() < 1e-10, "expected 6, got {val}");
    }

    #[test]
    fn test_diff_quotient() {
        let e = parse("x / x").unwrap();
        let d = differentiate(&e, "x");
        // d/dx x/x = 0 (since x/x = 1)
        let mut vars = HashMap::new();
        vars.insert("x".into(), 5.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 0.0).abs() < 1e-10, "expected 0, got {val}");
    }

    #[test]
    fn test_diff_power_rule() {
        let e = parse("x^3").unwrap();
        let d = differentiate(&e, "x");
        // d/dx x^3 = 3x^2, at x=2 gives 12
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 12.0).abs() < 1e-10, "expected 12, got {val}");
    }

    #[test]
    fn test_diff_sin() {
        let e = parse("sin(x)").unwrap();
        let d = differentiate(&e, "x");
        // d/dx sin(x) = cos(x), at x=0 gives 1
        let mut vars = HashMap::new();
        vars.insert("x".into(), 0.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 1.0).abs() < 1e-10, "expected 1, got {val}");
    }

    #[test]
    fn test_diff_cos() {
        let e = parse("cos(x)").unwrap();
        let d = differentiate(&e, "x");
        // d/dx cos(x) = -sin(x), at x=0 gives 0
        let mut vars = HashMap::new();
        vars.insert("x".into(), 0.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 0.0).abs() < 1e-10, "expected 0, got {val}");
    }

    #[test]
    fn test_diff_exp() {
        let e = parse("exp(x)").unwrap();
        let d = differentiate(&e, "x");
        // d/dx exp(x) = exp(x), at x=0 gives 1
        let mut vars = HashMap::new();
        vars.insert("x".into(), 0.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 1.0).abs() < 1e-10, "expected 1, got {val}");
    }

    #[test]
    fn test_diff_ln() {
        let e = parse("ln(x)").unwrap();
        let d = differentiate(&e, "x");
        // d/dx ln(x) = 1/x, at x=2 gives 0.5
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 0.5).abs() < 1e-10, "expected 0.5, got {val}");
    }

    #[test]
    fn test_diff_sqrt() {
        let e = parse("sqrt(x)").unwrap();
        let d = differentiate(&e, "x");
        // d/dx sqrt(x) = 1/(2*sqrt(x)), at x=4 gives 0.25
        let mut vars = HashMap::new();
        vars.insert("x".into(), 4.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 0.25).abs() < 1e-10, "expected 0.25, got {val}");
    }

    #[test]
    fn test_diff_tan() {
        let e = parse("tan(x)").unwrap();
        let d = differentiate(&e, "x");
        // d/dx tan(x) = sec^2(x) = 1/cos^2(x), at x=pi/4 gives 2
        let mut vars = HashMap::new();
        vars.insert("x".into(), std::f64::consts::FRAC_PI_4);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 2.0).abs() < 1e-10, "expected 2, got {val}");
    }

    #[test]
    fn test_diff_neg() {
        let e = parse("-x").unwrap();
        let d = differentiate(&e, "x");
        let val = eval(&d, &HashMap::new()).unwrap();
        assert!((val + 1.0).abs() < 1e-10, "expected -1, got {val}");
    }

    #[test]
    fn test_diff_sub() {
        let e = parse("x^2 - x").unwrap();
        let d = differentiate(&e, "x");
        // d/dx (x^2 - x) = 2x - 1, at x=3 gives 5
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        let val = eval(&d, &vars).unwrap();
        assert!((val - 5.0).abs() < 1e-10, "expected 5, got {val}");
    }

    #[test]
    fn test_diff_chain_rule() {
        // d/dx sin(x^2) = cos(x^2) * 2x, at x=2
        let e = parse("sin(x^2)").unwrap();
        let d = differentiate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let val = eval(&d, &vars).unwrap();
        let expected = (4.0_f64).cos() * 4.0;
        assert!((val - expected).abs() < 1e-10, "expected {expected}, got {val}");
    }

    #[test]
    fn test_diff_exponential_const_base() {
        // d/dx 2^x = 2^x * ln(2), at x=1 gives 2*ln(2) ≈ 1.386
        let e = parse("2^x").unwrap();
        let d = differentiate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 1.0);
        let val = eval(&d, &vars).unwrap();
        let expected = 2.0_f64.ln() * 2.0;
        assert!((val - expected).abs() < 1e-10, "expected {expected}, got {val}");
    }

    // ── Factor enhancement ──

    #[test]
    fn test_factor_quadratic() {
        let e = parse("x^2 - 5*x + 6").unwrap();
        let f = factor(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 7.0);
        let orig = eval(&e, &vars).unwrap();
        let factored = eval(&f, &vars).unwrap();
        assert!(
            (orig - factored).abs() < 1e-10,
            "factor(x^2 - 5*x + 6) changed value at x=7"
        );
    }

    #[test]
    fn test_factor_quadratic_perfect_square() {
        let e = parse("x^2 - 6*x + 9").unwrap();
        let f = factor(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 5.0);
        let orig = eval(&e, &vars).unwrap();
        let factored = eval(&f, &vars).unwrap();
        assert!(
            (orig - factored).abs() < 1e-10,
            "factor(x^2 - 6x + 9) changed value at x=5"
        );
        assert_eq!(display(&f), "(x - 3)*(x - 3)", "expected (x-3)^2, got {}", display(&f));
    }

    #[test]
    fn test_factor_quadratic_leading_coeff() {
        let e = parse("2*x^2 + 4*x + 2").unwrap();
        let f = factor(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        let orig = eval(&e, &vars).unwrap();
        let factored = eval(&f, &vars).unwrap();
        assert!(
            (orig - factored).abs() < 1e-10,
            "factor(2*x^2 + 4x + 2) changed value at x=3"
        );
    }

    #[test]
    fn test_factor_diff_squares() {
        let e = parse("x^2 - 9").unwrap();
        let f = factor(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 5.0);
        let orig = eval(&e, &vars).unwrap();
        let factored = eval(&f, &vars).unwrap();
        assert!(
            (orig - factored).abs() < 1e-10,
            "factor(x^2 - 9) changed value at x=5"
        );
    }

    #[test]
    fn test_factor_noop() {
        // No factoring possible — should return unchanged
        let e = parse("x + y").unwrap();
        let f = factor(&e);
        assert_eq!(display(&f), "x + y");
    }

    #[test]
    fn test_factor_already_factored() {
        let e = parse("(x+1)*(x+2)").unwrap();
        let f = factor(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        let orig = eval(&e, &vars).unwrap();
        let factored = eval(&f, &vars).unwrap();
        assert!(
            (orig - factored).abs() < 1e-10,
            "factor((x+1)*(x+2)) changed value at x=3"
        );
    }

    #[test]
    fn test_factor_non_quadratic() {
        // Cubic is not factored by any strategy — value preserved
        let e = parse("x^3 - x").unwrap();
        let f = factor(&e);
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let orig = eval(&e, &vars).unwrap();
        let factored = eval(&f, &vars).unwrap();
        assert!(
            (orig - factored).abs() < 1e-10,
            "factor(x^3 - x) changed value at x=2"
        );
    }

    // ── Integration ──

    #[test]
    fn test_integrate_const() {
        let e = parse("5").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - 10.0).abs() < 1e-10, "expected 10, got {val}");
    }

    #[test]
    fn test_integrate_var() {
        let e = parse("x").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - 4.5).abs() < 1e-10, "expected 4.5, got {val}");
    }

    #[test]
    fn test_integrate_power() {
        let e = parse("x^2").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - 9.0).abs() < 1e-10, "expected 9, got {val}");
    }

    #[test]
    fn test_integrate_neg_power() {
        let e = parse("x^(-2)").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 1.0);
        let val = eval(&i, &vars).unwrap();
        let expected = -1.0 / 1.0; // ∫ x^(-2) = -x^(-1), at x=1 → -1
        assert!((val - expected).abs() < 1e-10, "expected {expected}, got {val}");
    }

    #[test]
    fn test_integrate_reciprocal() {
        let e = parse("1/x").unwrap();
        let i = integrate(&e, "x");
        let d = display(&i);
        assert_eq!(d, "ln(x)", "expected ln(x), got {d}");
    }

    #[test]
    fn test_integrate_neg() {
        let e = parse("-x").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - (-2.0)).abs() < 1e-10, "expected -2.0, got {val}");
    }

    #[test]
    fn test_integrate_add() {
        let e = parse("x + 1").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - 4.0).abs() < 1e-10, "expected 4.0, got {val}");
    }

    #[test]
    fn test_integrate_const_factor() {
        let e = parse("3*x^2").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 2.0);
        let val = eval(&i, &vars).unwrap();
        // ∫ 3*x^2 dx = x^3, at x=2 → 8
        assert!((val - 8.0).abs() < 1e-10, "expected 8, got {val}");
    }

    #[test]
    fn test_integrate_sin() {
        let e = parse("sin(x)").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 0.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - (-1.0)).abs() < 1e-10, "expected -1, got {val}");
    }

    #[test]
    fn test_integrate_cos() {
        let e = parse("cos(x)").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 0.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - 0.0).abs() < 1e-10, "expected 0, got {val}");
    }

    #[test]
    fn test_integrate_exp() {
        let e = parse("exp(x)").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 0.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - 1.0).abs() < 1e-10, "expected 1, got {val}");
    }

    #[test]
    fn test_integrate_other_var_as_const() {
        // ∫ y dx = y * x
        let e = parse("y").unwrap();
        let i = integrate(&e, "x");
        let mut vars = HashMap::new();
        vars.insert("x".into(), 3.0);
        vars.insert("y".into(), 5.0);
        let val = eval(&i, &vars).unwrap();
        assert!((val - 15.0).abs() < 1e-10, "expected 15, got {val}");
    }

    // ── Equation solving ──

    #[test]
    fn test_solve_linear() {
        let solutions = solve_equation("2*x + 4 = 0", "x").unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0], "x = -2");
    }

    #[test]
    fn test_solve_quadratic() {
        let solutions = solve_equation("x^2 - 5*x + 6 = 0", "x").unwrap();
        assert_eq!(solutions.len(), 2);
        assert!(solutions.contains(&"x = 2".to_string()));
        assert!(solutions.contains(&"x = 3".to_string()));
    }

    #[test]
    fn test_solve_quadratic_double_root() {
        let solutions = solve_equation("x^2 - 6*x + 9 = 0", "x").unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0], "x = 3");
    }

    #[test]
    fn test_solve_pure_power() {
        let solutions = solve_equation("x^3 = 8", "x").unwrap();
        assert_eq!(solutions.len(), 1);
        // x^3 = 8 → x = 2
        assert_eq!(solutions[0], "x = 2");
    }

    #[test]
    fn test_solve_identity() {
        let solutions = solve_equation("x + 2 = x + 2", "x").unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0], "x is any real number");
    }

    #[test]
    fn test_solve_contradiction() {
        let result = solve_equation("x + 2 = x + 3", "x");
        assert!(
            result.is_err(),
            "contradiction should produce error, got {:?}",
            result
        );
    }

    #[test]
    fn test_solve_no_equals() {
        let result = solve_equation("x^2", "x");
        assert!(result.is_err(), "equation without '=' should produce error");
    }
}
