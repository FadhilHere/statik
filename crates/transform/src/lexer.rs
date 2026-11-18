//! Tokenizer for SPSS-style transformation expressions.

use crate::parser::ParseError;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Num(f64),
    Str(String),
    /// Identifier as written (variable or function name, e.g. `MEAN.2`).
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Pow,
    LParen,
    RParen,
    Comma,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Not,
    To,
}

impl Tok {
    /// Short human-readable description for error messages.
    pub(crate) fn describe(&self) -> String {
        let s = match self {
            Tok::Num(n) => return format!("number {n}"),
            Tok::Str(s) => return format!("string '{s}'"),
            Tok::Ident(s) => return format!("'{s}'"),
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Pow => "**",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::Comma => ",",
            Tok::Eq => "=",
            Tok::Ne => "<>",
            Tok::Lt => "<",
            Tok::Gt => ">",
            Tok::Le => "<=",
            Tok::Ge => ">=",
            Tok::And => "AND",
            Tok::Or => "OR",
            Tok::Not => "NOT",
            Tok::To => "TO",
        };
        format!("'{s}'")
    }
}

/// A token and the byte offset where it starts.
#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub tok: Tok,
    pub pos: usize,
}

/// Reserved words that can never be variable names (SPSS).
pub(crate) fn keyword(word: &str) -> Option<Tok> {
    Some(match word.to_ascii_uppercase().as_str() {
        "AND" => Tok::And,
        "OR" => Tok::Or,
        "NOT" => Tok::Not,
        "EQ" => Tok::Eq,
        "NE" => Tok::Ne,
        "LT" => Tok::Lt,
        "GT" => Tok::Gt,
        "LE" => Tok::Le,
        "GE" => Tok::Ge,
        "TO" => Tok::To,
        "ALL" | "BY" | "WITH" => Tok::Ident(word.to_string()),
        _ => return None,
    })
}

/// True for reserved words (`AND`, `TO`, `ALL`, ...), which are not valid variable names.
pub(crate) fn is_reserved(word: &str) -> bool {
    keyword(word).is_some()
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic()
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '#' | '@')
}

pub(crate) fn tokenize(src: &str) -> Result<Vec<Token>, ParseError> {
    let err = |pos: usize, message: String| ParseError { pos, message };
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    while let Some(&(pos, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let at = |i: usize| bytes.get(i).copied();
        let tok = if c.is_ascii_digit() || (c == '.' && at(pos + 1).is_some_and(|b| b.is_ascii_digit())) {
            // Number: digits [. digits] [e [+-] digits]  or  . digits [...]
            let mut end = pos;
            while at(end).is_some_and(|b| b.is_ascii_digit()) {
                end += 1;
            }
            if at(end) == Some(b'.') {
                end += 1;
                while at(end).is_some_and(|b| b.is_ascii_digit()) {
                    end += 1;
                }
            }
            if matches!(at(end), Some(b'e' | b'E')) {
                let mut e = end + 1;
                if matches!(at(e), Some(b'+' | b'-')) {
                    e += 1;
                }
                if !at(e).is_some_and(|b| b.is_ascii_digit()) {
                    return Err(err(pos, format!("Invalid number '{}'", &src[pos..e.min(src.len())])));
                }
                while at(e).is_some_and(|b| b.is_ascii_digit()) {
                    e += 1;
                }
                end = e;
            }
            let text = &src[pos..end];
            // A number running straight into a name ("3x") is a mistake, not two tokens.
            if src[end..].chars().next().is_some_and(is_ident_char) {
                let stop = src[end..].find(|c: char| !is_ident_char(c)).map_or(src.len(), |i| end + i);
                return Err(err(pos, format!("Invalid number '{}'", &src[pos..stop])));
            }
            let v: f64 = text.parse().map_err(|_| err(pos, format!("Invalid number '{text}'")))?;
            if !v.is_finite() {
                return Err(err(pos, format!("Number out of range '{text}'")));
            }
            while chars.peek().is_some_and(|&(i, _)| i < end) {
                chars.next();
            }
            Tok::Num(v)
        } else if is_ident_start(c) {
            let mut end = pos;
            while let Some(&(i, ch)) = chars.peek() {
                if !is_ident_char(ch) {
                    break;
                }
                // A trailing period is not part of a name.
                if ch == '.' && !src[i + 1..].chars().next().is_some_and(is_ident_char) {
                    break;
                }
                end = i + ch.len_utf8();
                chars.next();
            }
            let word = &src[pos..end];
            keyword(word).unwrap_or_else(|| Tok::Ident(word.to_string()))
        } else if c == '\'' || c == '"' {
            chars.next();
            let mut s = String::new();
            loop {
                match chars.next() {
                    None => return Err(err(pos, "Unterminated string".into())),
                    Some((_, ch)) if ch == c => {
                        if chars.peek().is_some_and(|&(_, n)| n == c) {
                            chars.next();
                            s.push(c);
                        } else {
                            break;
                        }
                    }
                    Some((_, ch)) => s.push(ch),
                }
            }
            out.push(Token { tok: Tok::Str(s), pos });
            continue;
        } else {
            chars.next();
            let next = chars.peek().map(|&(_, n)| n);
            let mut two = |t: Tok| {
                chars.next();
                t
            };
            match (c, next) {
                ('*', Some('*')) => two(Tok::Pow),
                ('<', Some('>')) => two(Tok::Ne),
                ('<', Some('=')) => two(Tok::Le),
                ('>', Some('=')) => two(Tok::Ge),
                ('~', Some('=')) => two(Tok::Ne),
                ('+', _) => Tok::Plus,
                ('-', _) => Tok::Minus,
                ('*', _) => Tok::Star,
                ('/', _) => Tok::Slash,
                ('(', _) => Tok::LParen,
                (')', _) => Tok::RParen,
                (',', _) => Tok::Comma,
                ('=', _) => Tok::Eq,
                ('<', _) => Tok::Lt,
                ('>', _) => Tok::Gt,
                ('&', _) => Tok::And,
                ('|', _) => Tok::Or,
                ('~', _) => Tok::Not,
                _ => return Err(err(pos, format!("Unexpected character '{}'", c.escape_default()))),
            }
        };
        out.push(Token { tok, pos });
    }
    Ok(out)
}
