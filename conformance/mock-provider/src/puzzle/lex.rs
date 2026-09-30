//! Tokenizer for `x-demo-lang`.
//!
//! `name: x => x * 2` inside an `ops { }` block declares an op; a `solution =
//! [ a, b ]` list applies ops in sequence. Whitespace separates tokens; only
//! newlines are significant.

use super::text::SourceText;
use super::{OpKind, Range};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum TokenKind {
    Ident(String),
    Number(i64),
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Eq,
    Arrow, // =>
    Star,
    Plus,
    Minus,
    Slash,
    Newline,
}

impl TokenKind {
    /// The arithmetic operator an op body applies to `x`, if this token is one.
    pub(super) fn as_op(&self) -> Option<OpKind> {
        Some(match self {
            TokenKind::Star => OpKind::Mul,
            TokenKind::Plus => OpKind::Add,
            TokenKind::Minus => OpKind::Sub,
            TokenKind::Slash => OpKind::Div,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct Token {
    pub kind: TokenKind,
    /// LPP range covering the token's byte offsets.
    pub range: Range,
}

const PUNCT: &[(u8, TokenKind)] = &[
    (b'{', TokenKind::LBrace),
    (b'}', TokenKind::RBrace),
    (b'[', TokenKind::LBracket),
    (b']', TokenKind::RBracket),
    (b':', TokenKind::Colon),
    (b',', TokenKind::Comma),
    (b'*', TokenKind::Star),
    (b'+', TokenKind::Plus),
    (b'/', TokenKind::Slash),
];

pub(super) fn tokenize(src: &SourceText<'_>, text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let kind = match bytes[i] {
            b' ' | b'\t' | b'\r' => {
                i += 1;
                continue;
            }
            b'\n' => {
                i += 1;
                TokenKind::Newline
            }
            b'=' => {
                i += 1;
                if bytes.get(i) == Some(&b'>') {
                    i += 1;
                    TokenKind::Arrow
                } else {
                    TokenKind::Eq
                }
            }
            b'-' if bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit()) => {
                let (n, end) = read_number(text, i);
                i = end;
                TokenKind::Number(n)
            }
            b'-' => {
                i += 1;
                TokenKind::Minus
            }
            b if b.is_ascii_digit() => {
                let (n, end) = read_number(text, i);
                i = end;
                TokenKind::Number(n)
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                TokenKind::Ident(text[start..i].to_string())
            }
            b => match PUNCT.iter().find(|(p, _)| *p == b) {
                Some((_, kind)) => {
                    i += 1;
                    kind.clone()
                }
                None => {
                    i += 1;
                    TokenKind::Ident(format!("<unknown:{b}>"))
                }
            },
        };
        tokens.push(Token {
            kind,
            range: src.range_of(start, i),
        });
    }
    tokens
}

fn read_number(text: &str, i: usize) -> (i64, usize) {
    let bytes = text.as_bytes();
    let negative = bytes[i] == b'-';
    let mut end = i + usize::from(negative);
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    let digits = &text[i + usize::from(negative)..end];
    let value = digits.parse::<i64>().unwrap_or(0);
    (if negative { -value } else { value }, end)
}

/// Human-readable token description for parse diagnostics.
pub(super) fn describe(tok: &Token) -> String {
    match &tok.kind {
        TokenKind::Ident(name) => format!("'{name}'"),
        TokenKind::Number(n) => format!("number {n}"),
        TokenKind::Newline => "end of line".to_string(),
        other => format!("'{other:?}'"),
    }
}
