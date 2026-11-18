//! Parser for SPSS-style expressions (precedence climbing).
//!
//! Precedence, loosest first: `OR`, `AND`, `NOT`, comparisons, `+ -`, `* /`, unary `-`,
//! `**` (left-associative; its right operand may carry a unary minus), then calls/parentheses.

use crate::functions;
use crate::lexer::{Tok, Token, is_reserved, tokenize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    /// Unary minus.
    Neg,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Not,
    /// Variable range `a TO b` (only inside statistical function arguments).
    To,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(String),
    /// Variable name as written.
    Var(String),
    Unary(Op, Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    /// Upper-cased function name including any `.n` suffix (e.g. `MEAN.2`).
    Call(String, Vec<Expr>),
}

/// Syntax error; `pos` is a byte offset into the source (`src.len()` = end of input).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub pos: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at position {})", self.message, self.pos)
    }
}

impl std::error::Error for ParseError {}

/// Maximum nesting of parentheses / calls / unary operators (bounds parser recursion).
const MAX_NESTING: usize = 200;
/// Maximum depth of the resulting tree (bounds evaluation recursion, e.g. `1+1+1+...`).
const MAX_DEPTH: usize = 500;

/// Parse an expression.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, i: 0, end: src.len(), nesting: 0 };
    let (e, _) = p.or()?;
    if let Some(t) = p.peek() {
        return Err(ParseError { pos: t.pos, message: format!("Unexpected {}", t.tok.describe()) });
    }
    Ok(e)
}

/// Binding powers (higher binds tighter).
const NOT_BP: u8 = 3;
const NEG_BP: u8 = 7;

/// Infix operator and its binding power.
fn infix(t: &Tok) -> Option<(Op, u8)> {
    Some(match t {
        Tok::Or => (Op::Or, 1),
        Tok::And => (Op::And, 2),
        Tok::Eq => (Op::Eq, 4),
        Tok::Ne => (Op::Ne, 4),
        Tok::Lt => (Op::Lt, 4),
        Tok::Gt => (Op::Gt, 4),
        Tok::Le => (Op::Le, 4),
        Tok::Ge => (Op::Ge, 4),
        Tok::Plus => (Op::Add, 5),
        Tok::Minus => (Op::Sub, 5),
        Tok::Star => (Op::Mul, 6),
        Tok::Slash => (Op::Div, 6),
        Tok::Pow => (Op::Pow, 8),
        _ => return None,
    })
}

/// An expression with its tree depth.
type Parsed = Result<(Expr, usize), ParseError>;

struct Parser {
    toks: Vec<Token>,
    i: usize,
    end: usize,
    nesting: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.i)
    }

    fn peek_tok(&self) -> Option<&Tok> {
        self.peek().map(|t| &t.tok)
    }

    /// Position of the next token (end of input if none).
    fn pos(&self) -> usize {
        self.peek().map_or(self.end, |t| t.pos)
    }

    fn err<T>(&self, pos: usize, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError { pos, message: message.into() })
    }

    /// Consume the next token if it is one of `ops`, returning its operator.
    fn eat(&mut self, ops: &[(Tok, Op)]) -> Option<Op> {
        let t = self.peek_tok()?;
        let op = ops.iter().find(|(k, _)| k == t)?.1;
        self.i += 1;
        Some(op)
    }

    /// Run `f` one nesting level deeper.
    fn nested(&mut self, pos: usize, f: impl FnOnce(&mut Self) -> Parsed) -> Parsed {
        if self.nesting >= MAX_NESTING {
            return self.err(pos, "Expression is nested too deeply");
        }
        self.nesting += 1;
        let r = f(self);
        self.nesting -= 1;
        r
    }

    fn unary(&self, op: Op, (e, d): (Expr, usize), pos: usize) -> Parsed {
        if d + 1 > MAX_DEPTH {
            return self.err(pos, "Expression is too complex");
        }
        Ok((Expr::Unary(op, Box::new(e)), d + 1))
    }

    fn binary(&self, op: Op, (l, dl): (Expr, usize), (r, dr): (Expr, usize), pos: usize) -> Parsed {
        let d = dl.max(dr) + 1;
        if d > MAX_DEPTH {
            return self.err(pos, "Expression is too complex");
        }
        Ok((Expr::Binary(op, Box::new(l), Box::new(r)), d))
    }

    /// Full expression (loosest precedence).
    fn or(&mut self) -> Parsed {
        self.expr(0)
    }

    /// Precedence climbing: parse an operand, then every infix operator whose binding
    /// power is at least `min_bp` (all binary operators are left-associative).
    fn expr(&mut self, min_bp: u8) -> Parsed {
        let mut lhs = self.prefix(min_bp)?;
        loop {
            let Some(t) = self.peek() else { return Ok(lhs) };
            let pos = t.pos;
            let Some((op, bp)) = infix(&t.tok) else { return Ok(lhs) };
            if bp < min_bp {
                return Ok(lhs);
            }
            self.i += 1;
            let rhs = self.expr(bp + 1)?;
            lhs = self.binary(op, lhs, rhs, pos)?;
        }
    }

    /// Operand with any prefix operators: `NOT x`, `-x`, `+x`, or a primary.
    fn prefix(&mut self, min_bp: u8) -> Parsed {
        let pos = self.pos();
        match self.peek_tok() {
            // `NOT` takes a comparison-level operand; like SPSS it cannot appear inside
            // a comparison or arithmetic operand without parentheses.
            Some(Tok::Not) if min_bp <= NOT_BP => {
                self.i += 1;
                let e = self.nested(pos, |p| p.expr(NOT_BP))?;
                self.unary(Op::Not, e, pos)
            }
            // Unary minus binds looser than `**` (`-2**2` = -4) but may also start the
            // right operand of `**` (`2**-1`), where it covers only a primary.
            Some(Tok::Minus | Tok::Plus) => {
                let neg = self.peek_tok() == Some(&Tok::Minus);
                self.i += 1;
                let e = self.nested(pos, |p| p.expr(NEG_BP.max(min_bp)))?;
                if neg { self.unary(Op::Neg, e, pos) } else { Ok(e) }
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Parsed {
        let pos = self.pos();
        let Some(tok) = self.peek_tok().cloned() else {
            return self.err(pos, "Unexpected end of expression");
        };
        self.i += 1;
        match tok {
            Tok::Num(v) => Ok((Expr::Num(v), 1)),
            Tok::Str(s) => Ok((Expr::Str(s), 1)),
            Tok::LParen => {
                let e = self.nested(pos, Self::or)?;
                self.expect_rparen()?;
                Ok(e)
            }
            Tok::Ident(name) if self.peek_tok() == Some(&Tok::LParen) => {
                self.i += 1;
                self.nested(pos, |p| p.call(&name, pos))
            }
            Tok::Ident(name) if is_reserved(&name) => {
                self.err(pos, format!("Reserved word {} cannot be used here", name.to_uppercase()))
            }
            Tok::Ident(name) => Ok((Expr::Var(name), 1)),
            t => self.err(pos, format!("Expected an expression but found {}", t.describe())),
        }
    }

    fn expect_rparen(&mut self) -> Result<(), ParseError> {
        if self.peek_tok() == Some(&Tok::RParen) {
            self.i += 1;
            Ok(())
        } else {
            self.err(self.pos(), "Expected ')'")
        }
    }

    /// Function call; the name and `(` are already consumed.
    fn call(&mut self, name: &str, name_pos: usize) -> Parsed {
        let fref = match functions::resolve(name) {
            Ok(f) => f,
            Err(m) => return self.err(name_pos, m),
        };
        let mut args = Vec::new();
        let mut depth = 0;
        if self.peek_tok() != Some(&Tok::RParen) {
            loop {
                let (arg, d) = self.or()?;
                let to_pos = self.pos();
                let arg = if self.eat(&[(Tok::To, Op::To)]).is_some() {
                    if !fref.func.is_statistical() {
                        return self.err(to_pos, format!("TO is not allowed in arguments of {}", fref.func.name()));
                    }
                    let Expr::Var(first) = arg else {
                        return self.err(to_pos, "TO must be preceded by a variable name");
                    };
                    let last_pos = self.pos();
                    match self.peek_tok().cloned() {
                        Some(Tok::Ident(last)) if !is_reserved(&last) => {
                            self.i += 1;
                            Expr::Binary(Op::To, Box::new(Expr::Var(first)), Box::new(Expr::Var(last)))
                        }
                        _ => return self.err(last_pos, "Expected a variable name after TO"),
                    }
                } else {
                    arg
                };
                depth = depth.max(d + 1);
                args.push(arg);
                if self.peek_tok() == Some(&Tok::Comma) {
                    self.i += 1;
                } else {
                    break;
                }
            }
        }
        self.expect_rparen()?;
        let (min, max) = fref.func.arity();
        let n = args.len();
        if n < min || max.is_some_and(|m| n > m) {
            let fname = fref.func.name();
            let want = match max {
                Some(m) if m == min => format!("{min} argument{}", if min == 1 { "" } else { "s" }),
                Some(m) => format!("{min} to {m} arguments"),
                None => format!("at least {min} argument{}", if min == 1 { "" } else { "s" }),
            };
            return self.err(name_pos, format!("Function {fname} expects {want}, got {n}"));
        }
        Ok((Expr::Call(name.to_uppercase(), args), depth + 1))
    }
}
