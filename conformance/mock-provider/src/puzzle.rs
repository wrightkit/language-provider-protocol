//! The `x-demo-lang` puzzle/equation language: positions, parser, and
//! semantics.
//!
//! `x-demo-lang` is deliberately unlike OPY or DEL: it has no Workshop rules,
//! actions, or settings. A document declares one equation puzzle with a start
//! value, a target value, named arithmetic ops, and a solution that applies
//! ops in sequence. Compiling simulates the solution and emits a puzzle
//! evaluation sheet in the provider's own artifact format.

use serde::{Deserialize, Serialize};

/// LPP position: 0-based line, UTF-16 code units within the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Position {
    pub line: u32,
    pub character: u32,
}

/// Half-open range (start inclusive, end exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Diagnostic {
    pub range: Range,
    pub severity: String,
    pub code: String,
    pub message: String,
    pub source: String,
}

/// A document's text with line-indexing helpers.
pub(crate) struct SourceText<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourceText<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { text, line_starts }
    }

    pub(crate) fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Byte range of a line, excluding the trailing newline and carriage
    /// return.
    fn line_byte_range(&self, line: usize) -> (usize, usize) {
        let start = self.line_starts[line];
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());
        let end = end.saturating_sub(1);
        let end = if self.text.as_bytes().get(end) == Some(&b'\n') {
            if self.text.as_bytes().get(end.saturating_sub(1)) == Some(&b'\r') {
                end - 1
            } else {
                end
            }
        } else {
            end + 1
        };
        (start, end)
    }

    pub(crate) fn line_text(&self, line: usize) -> &'a str {
        let (start, end) = self.line_byte_range(line);
        &self.text[start..end]
    }

    /// Byte offset of a position, or `None` when the position is outside the
    /// document (a position at the end of a line is valid).
    pub(crate) fn byte_of(&self, pos: Position) -> Option<usize> {
        let line = pos.line as usize;
        if line >= self.line_count() {
            return None;
        }
        let line_text = self.line_text(line);
        let mut byte = 0;
        let mut units = 0u32;
        for (i, ch) in line_text.char_indices() {
            if units == pos.character {
                byte = i;
                break;
            }
            let len = ch.len_utf16() as u32;
            if units + len > pos.character {
                // Inside a supplementary-plane character: invalid.
                return None;
            }
            units += len;
            byte = i + ch.len_utf8();
        }
        if units != pos.character && byte == line_text.len() {
            return None;
        }
        Some(self.line_starts[line] + byte)
    }

    /// LPP position of a byte offset.
    pub(crate) fn position_of(&self, byte: usize) -> Position {
        let byte = byte.min(self.text.len());
        let line = match self.line_starts.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let (start, _) = self.line_byte_range(line);
        let units = self.text[start..byte]
            .chars()
            .map(char::len_utf16)
            .sum::<usize>();
        Position {
            line: line as u32,
            character: units as u32,
        }
    }

    /// True when `pos` is a valid position inside `range`.
    pub(crate) fn contains_byte(&self, range: Range, byte: usize) -> bool {
        let start = self.byte_of(range.start);
        let end = self.byte_of(range.end);
        match (start, end) {
            (Some(s), Some(e)) => s <= byte && byte < e,
            _ => false,
        }
    }
}

/// Binary arithmetic op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpKind {
    Add,
    Sub,
    Mul,
    Div,
}

impl OpKind {
    fn symbol(self) -> &'static str {
        match self {
            OpKind::Add => "+",
            OpKind::Sub => "-",
            OpKind::Mul => "*",
            OpKind::Div => "/",
        }
    }

    fn apply(self, lhs: i64, rhs: i64) -> Option<i64> {
        match self {
            OpKind::Add => lhs.checked_add(rhs),
            OpKind::Sub => lhs.checked_sub(rhs),
            OpKind::Mul => lhs.checked_mul(rhs),
            OpKind::Div => (rhs != 0).then(|| lhs / rhs),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Op {
    pub name: String,
    pub name_range: Range,
    pub kind: OpKind,
    pub arg: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct SolutionEntry {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub(crate) struct Puzzle {
    pub name: String,
    pub name_range: Range,
    pub target: i64,
    pub start: i64,
    pub ops: Vec<Op>,
    pub solution: Vec<SolutionEntry>,
    pub solution_range: Range,
}

impl Puzzle {
    pub(crate) fn op(&self, name: &str) -> Option<&Op> {
        self.ops.iter().find(|op| op.name == name)
    }

    /// Simulate the solution. Returns `None` when any op is unresolved or an
    /// arithmetic operation overflows.
    pub(crate) fn simulate(&self) -> Option<i64> {
        let mut value = self.start;
        for entry in &self.solution {
            let op = self.op(&entry.name)?;
            value = op.kind.apply(value, op.arg)?;
        }
        Some(value)
    }
}

#[derive(Debug)]
pub(crate) struct ParseOutput {
    pub puzzle: Option<Puzzle>,
    pub diagnostics: Vec<Diagnostic>,
}

fn syntax_error(range: Range, message: String) -> Diagnostic {
    Diagnostic {
        range,
        severity: "error".to_string(),
        code: "x-demo/syntax".to_string(),
        message,
        source: "x-demo-lang".to_string(),
    }
}

fn missing_section(range: Range, section: &str) -> Diagnostic {
    Diagnostic {
        range,
        severity: "error".to_string(),
        code: "x-demo/missing-section".to_string(),
        message: format!("puzzle is missing section '{section}'"),
        source: "x-demo-lang".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
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

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                tokens.push(Token {
                    kind: TokenKind::Newline,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b'{' => {
                tokens.push(Token {
                    kind: TokenKind::LBrace,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b'}' => {
                tokens.push(Token {
                    kind: TokenKind::RBrace,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b'[' => {
                tokens.push(Token {
                    kind: TokenKind::LBracket,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b']' => {
                tokens.push(Token {
                    kind: TokenKind::RBracket,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b':' => {
                tokens.push(Token {
                    kind: TokenKind::Colon,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b',' => {
                tokens.push(Token {
                    kind: TokenKind::Comma,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b'=' => {
                if bytes.get(i + 1) == Some(&b'>') {
                    tokens.push(Token {
                        kind: TokenKind::Arrow,
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Eq,
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                }
            }
            b'*' => {
                tokens.push(Token {
                    kind: TokenKind::Star,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b'+' => {
                tokens.push(Token {
                    kind: TokenKind::Plus,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b'-' => {
                if bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit()) {
                    let (n, end) = read_number(text, i);
                    tokens.push(Token {
                        kind: TokenKind::Number(n),
                        start: i,
                        end,
                    });
                    i = end;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Minus,
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                }
            }
            b'/' => {
                tokens.push(Token {
                    kind: TokenKind::Slash,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            b if b.is_ascii_digit() => {
                let (n, end) = read_number(text, i);
                tokens.push(Token {
                    kind: TokenKind::Number(n),
                    start: i,
                    end,
                });
                i = end;
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Ident(text[start..i].to_string()),
                    start,
                    end: i,
                });
            }
            other => {
                // Unknown character: emit a single-token placeholder so the
                // parser can report it as a syntax error.
                tokens.push(Token {
                    kind: TokenKind::Ident(format!("<unknown:{other}>")),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
        }
    }
    tokens
}

fn read_number(text: &str, i: usize) -> (i64, usize) {
    let bytes = text.as_bytes();
    let mut end = i;
    let negative = bytes[end] == b'-';
    if negative {
        end += 1;
    }
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    let digits = &text[i + usize::from(negative)..end];
    let value = digits.parse::<i64>().unwrap_or(0);
    (if negative { -value } else { value }, end)
}

struct Parser<'a> {
    src: SourceText<'a>,
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    /// Skip tokens until after the next newline (line-based recovery).
    fn recover_line(&mut self) {
        while let Some(tok) = self.advance() {
            if tok.kind == TokenKind::Newline {
                break;
            }
        }
    }

    fn error(&mut self, token: &Token, message: String) {
        let range = Range {
            start: self.src.position_of(token.start),
            end: self.src.position_of(token.end),
        };
        self.diagnostics.push(syntax_error(range, message));
        self.recover_line();
    }

    fn expect_ident(&mut self) -> Option<Token> {
        let next = self.peek().cloned()?;
        if matches!(next.kind, TokenKind::Ident(_)) {
            self.pos += 1;
            Some(next)
        } else {
            self.error(
                &next,
                format!("expected identifier, found {}", describe(&next)),
            );
            None
        }
    }

    fn expect_number(&mut self, what: &str) -> Option<(i64, Token)> {
        let next = self.peek().cloned()?;
        if let TokenKind::Number(n) = next.kind {
            self.pos += 1;
            Some((n, next))
        } else {
            self.error(&next, format!("expected {what}"));
            None
        }
    }

    fn expect_punct(&mut self, kind: TokenKind, what: &str) -> bool {
        let next = self.peek().cloned();
        match next {
            Some(tok) if tok.kind == kind => {
                self.pos += 1;
                true
            }
            Some(tok) => {
                self.error(&tok, format!("expected {what}"));
                false
            }
            None => false,
        }
    }
}

fn describe(tok: &Token) -> String {
    match &tok.kind {
        TokenKind::Ident(name) => format!("'{name}'"),
        TokenKind::Number(n) => format!("number {n}"),
        TokenKind::Newline => "end of line".to_string(),
        other => format!("'{other:?}'"),
    }
}

/// Parse a document. Diagnostics are appended to `diagnostics`.
pub(crate) fn parse_document(text: &str) -> ParseOutput {
    let src = SourceText::new(text);
    let tokens = tokenize(text);
    let mut parser = Parser {
        src,
        tokens,
        pos: 0,
        diagnostics: Vec::new(),
    };
    let puzzle = parse_top(&mut parser);
    ParseOutput {
        puzzle,
        diagnostics: parser.diagnostics,
    }
}

fn parse_top(parser: &mut Parser<'_>) -> Option<Puzzle> {
    // Skip leading newlines.
    while matches!(parser.peek(), Some(tok) if tok.kind == TokenKind::Newline) {
        parser.advance();
    }
    let Some(first) = parser.peek().cloned() else {
        parser.diagnostics.push(syntax_error(
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
            "empty document: expected 'puzzle <name> {{ ... }}'".to_string(),
        ));
        return None;
    };
    let TokenKind::Ident(name) = &first.kind else {
        parser.error(
            &first,
            format!("expected 'puzzle', found {}", describe(&first)),
        );
        return None;
    };
    if name != "puzzle" {
        parser.error(&first, format!("expected 'puzzle', found '{}'", name));
        return None;
    }
    parser.advance();
    let name_tok = parser.expect_ident()?;
    let TokenKind::Ident(puzzle_name) = name_tok.kind else {
        unreachable!()
    };
    let name_range = Range {
        start: parser.src.position_of(name_tok.start),
        end: parser.src.position_of(name_tok.end),
    };
    if !parser.expect_punct(TokenKind::LBrace, "'{'") {
        return None;
    }

    let mut puzzle = Puzzle {
        name: puzzle_name,
        name_range,
        target: 0,
        start: 0,
        ops: Vec::new(),
        solution: Vec::new(),
        solution_range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
    };
    let mut has_target = false;
    let mut has_start = false;
    let mut has_ops = false;
    let mut has_solution = false;

    loop {
        let Some(tok) = parser.peek() else {
            parser.diagnostics.push(syntax_error(
                name_range,
                "unterminated puzzle: missing '}'".to_string(),
            ));
            break;
        };
        match &tok.kind {
            TokenKind::Newline => {
                parser.advance();
            }
            TokenKind::RBrace => {
                parser.advance();
                break;
            }
            TokenKind::Ident(word) => match word.as_str() {
                "target" => {
                    parser.advance();
                    if parser.expect_punct(TokenKind::Eq, "'='") {
                        if let Some((n, _)) = parser.expect_number("a target value") {
                            has_target = true;
                            puzzle.target = n;
                        }
                    }
                }
                "start" => {
                    parser.advance();
                    if parser.expect_punct(TokenKind::Eq, "'='") {
                        if let Some((n, _)) = parser.expect_number("a start value") {
                            has_start = true;
                            puzzle.start = n;
                        }
                    }
                }
                "ops" => {
                    parser.advance();
                    has_ops = parse_ops(parser, &mut puzzle);
                }
                "solution" => {
                    parser.advance();
                    has_solution = parse_solution(parser, &mut puzzle);
                }
                other => {
                    let message = format!("unexpected '{}' in puzzle body", other);
                    let range = Range {
                        start: parser.src.position_of(tok.start),
                        end: parser.src.position_of(tok.end),
                    };
                    parser.diagnostics.push(syntax_error(range, message));
                    parser.recover_line();
                }
            },
            _ => {
                let message = format!("unexpected {} in puzzle body", describe(tok));
                let range = Range {
                    start: parser.src.position_of(tok.start),
                    end: parser.src.position_of(tok.end),
                };
                parser.diagnostics.push(syntax_error(range, message));
                parser.recover_line();
            }
        }
    }

    // Trailing content after the closing brace is an error.
    while let Some(tok) = parser.peek() {
        if tok.kind == TokenKind::Newline {
            parser.advance();
            continue;
        }
        let range = Range {
            start: parser.src.position_of(tok.start),
            end: parser.src.position_of(tok.end),
        };
        parser.diagnostics.push(syntax_error(
            range,
            format!("unexpected content after puzzle: {}", describe(tok)),
        ));
        parser.recover_line();
    }

    if !has_target {
        parser
            .diagnostics
            .push(missing_section(puzzle.name_range, "target"));
    }
    if !has_start {
        parser
            .diagnostics
            .push(missing_section(puzzle.name_range, "start"));
    }
    if !has_ops {
        parser
            .diagnostics
            .push(missing_section(puzzle.name_range, "ops"));
    }
    if !has_solution {
        parser
            .diagnostics
            .push(missing_section(puzzle.name_range, "solution"));
    }

    // Duplicate op names.
    let mut seen = std::collections::HashSet::new();
    for op in &puzzle.ops {
        if !seen.insert(op.name.clone()) {
            parser.diagnostics.push(Diagnostic {
                range: op.name_range,
                severity: "error".to_string(),
                code: "x-demo/duplicate-op".to_string(),
                message: format!("duplicate op name '{}'", op.name),
                source: "x-demo-lang".to_string(),
            });
        }
    }

    // Unresolved solution references.
    for entry in &puzzle.solution {
        if puzzle.op(&entry.name).is_none() {
            parser.diagnostics.push(Diagnostic {
                range: entry.range,
                severity: "error".to_string(),
                code: "x-demo/unresolved-op".to_string(),
                message: format!("unresolved op reference '{}'", entry.name),
                source: "x-demo-lang".to_string(),
            });
        }
    }

    // Warnings only when there are no errors.
    let has_errors = parser.diagnostics.iter().any(|d| d.severity == "error");
    if !has_errors {
        if puzzle.solution.is_empty() {
            parser.diagnostics.push(Diagnostic {
                range: puzzle.solution_range,
                severity: "warning".to_string(),
                code: "x-demo/empty-solution".to_string(),
                message: "solution is empty".to_string(),
                source: "x-demo-lang".to_string(),
            });
        } else if let Some(value) = puzzle.simulate() {
            if value != puzzle.target {
                parser.diagnostics.push(Diagnostic {
                    range: puzzle.solution_range,
                    severity: "warning".to_string(),
                    code: "x-demo/target-not-reached".to_string(),
                    message: format!(
                        "solution does not reach target: expected {}, reached {}",
                        puzzle.target, value
                    ),
                    source: "x-demo-lang".to_string(),
                });
            }
        }
    }

    Some(puzzle)
}

fn parse_ops(parser: &mut Parser<'_>, puzzle: &mut Puzzle) -> bool {
    if !parser.expect_punct(TokenKind::LBrace, "'{'") {
        return false;
    }
    loop {
        let Some(tok) = parser.peek() else {
            parser.diagnostics.push(syntax_error(
                puzzle.name_range,
                "unterminated ops block: missing '}'".to_string(),
            ));
            return true;
        };
        match &tok.kind {
            TokenKind::Newline => {
                parser.advance();
            }
            TokenKind::RBrace => {
                parser.advance();
                return true;
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                let name_tok = tok.clone();
                parser.advance();
                parser.expect_punct(TokenKind::Colon, "':'");
                // The parameter name is required to be `x`.
                if let Some(param) = parser.expect_ident() {
                    let TokenKind::Ident(param_name) = param.kind else {
                        unreachable!()
                    };
                    if param_name != "x" {
                        let range = Range {
                            start: parser.src.position_of(param.start),
                            end: parser.src.position_of(param.end),
                        };
                        parser.diagnostics.push(syntax_error(
                            range,
                            format!("op parameter must be 'x', found '{param_name}'"),
                        ));
                        parser.recover_line();
                        continue;
                    }
                } else {
                    parser.recover_line();
                    continue;
                }
                parser.expect_punct(TokenKind::Arrow, "'=>'");
                if let Some(param) = parser.expect_ident() {
                    let TokenKind::Ident(param_name) = param.kind else {
                        unreachable!()
                    };
                    if param_name != "x" {
                        let range = Range {
                            start: parser.src.position_of(param.start),
                            end: parser.src.position_of(param.end),
                        };
                        parser.diagnostics.push(syntax_error(
                            range,
                            format!("op body must apply to 'x', found '{param_name}'"),
                        ));
                        parser.recover_line();
                        continue;
                    }
                } else {
                    parser.recover_line();
                    continue;
                }
                let Some(op_tok) = parser.advance().cloned() else {
                    parser.recover_line();
                    continue;
                };
                let kind = match &op_tok.kind {
                    TokenKind::Star => Some(OpKind::Mul),
                    TokenKind::Plus => Some(OpKind::Add),
                    TokenKind::Minus => Some(OpKind::Sub),
                    TokenKind::Slash => Some(OpKind::Div),
                    _ => {
                        let range = Range {
                            start: parser.src.position_of(op_tok.start),
                            end: parser.src.position_of(op_tok.end),
                        };
                        parser.diagnostics.push(syntax_error(
                            range,
                            "expected arithmetic operator '+', '-', '*', or '/'".to_string(),
                        ));
                        parser.recover_line();
                        continue;
                    }
                };
                let Some((arg, _)) = parser.expect_number("a numeric argument") else {
                    parser.recover_line();
                    continue;
                };
                let name_range = Range {
                    start: parser.src.position_of(name_tok.start),
                    end: parser.src.position_of(name_tok.end),
                };
                puzzle.ops.push(Op {
                    name,
                    name_range,
                    kind: kind.expect("operator kind parsed above"),
                    arg,
                });
            }
            _ => {
                let range = Range {
                    start: parser.src.position_of(tok.start),
                    end: parser.src.position_of(tok.end),
                };
                parser.diagnostics.push(syntax_error(
                    range,
                    format!("unexpected {} in ops block", describe(tok)),
                ));
                parser.recover_line();
            }
        }
    }
}

fn parse_solution(parser: &mut Parser<'_>, puzzle: &mut Puzzle) -> bool {
    if !parser.expect_punct(TokenKind::Eq, "'='") {
        return false;
    }
    let Some(open) = parser.peek().cloned() else {
        return false;
    };
    if open.kind != TokenKind::LBracket {
        parser.error(&open, "expected '['".to_string());
        return false;
    }
    let start_pos = parser.src.position_of(open.start);
    parser.advance();
    loop {
        let Some(tok) = parser.peek() else {
            parser.diagnostics.push(syntax_error(
                Range {
                    start: start_pos,
                    end: start_pos,
                },
                "unterminated solution list: missing ']'".to_string(),
            ));
            return false;
        };
        match &tok.kind {
            TokenKind::RBracket => {
                let end_pos = parser.src.position_of(tok.end);
                puzzle.solution_range = Range {
                    start: start_pos,
                    end: end_pos,
                };
                parser.advance();
                return true;
            }
            TokenKind::Comma => {
                parser.advance();
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                let entry_tok = tok.clone();
                parser.advance();
                let range = Range {
                    start: parser.src.position_of(entry_tok.start),
                    end: parser.src.position_of(entry_tok.end),
                };
                puzzle.solution.push(SolutionEntry { name, range });
            }
            _ => {
                let range = Range {
                    start: parser.src.position_of(tok.start),
                    end: parser.src.position_of(tok.end),
                };
                parser.diagnostics.push(syntax_error(
                    range,
                    format!("unexpected {} in solution list", describe(tok)),
                ));
                parser.recover_line();
            }
        }
    }
}

/// Well-known symbol kinds of `x-demo-lang`.
pub(crate) const KIND_PUZZLE: &str = "puzzle";
pub(crate) const KIND_OP: &str = "op";

/// A symbol resolved at a position.
#[derive(Debug, Clone)]
pub(crate) enum Symbol {
    Puzzle { name: String, range: Range },
    Op { name: String },
}

impl Symbol {
    pub(crate) fn name(&self) -> &str {
        match self {
            Symbol::Puzzle { name, .. } => name,
            Symbol::Op { name } => name,
        }
    }
}

/// Resolve the symbol at a byte offset, if any. A position on a solution
/// entry resolves to the op it names; the caller resolves the declaration
/// from the name.
pub(crate) fn symbol_at(src: &SourceText<'_>, puzzle: &Puzzle, byte: usize) -> Option<Symbol> {
    if src.contains_byte(puzzle.name_range, byte) {
        return Some(Symbol::Puzzle {
            name: puzzle.name.clone(),
            range: puzzle.name_range,
        });
    }
    for op in &puzzle.ops {
        if src.contains_byte(op.name_range, byte) {
            return Some(Symbol::Op {
                name: op.name.clone(),
            });
        }
    }
    for entry in &puzzle.solution {
        if src.contains_byte(entry.range, byte) {
            return Some(Symbol::Op {
                name: entry.name.clone(),
            });
        }
    }
    None
}

/// Validate an identifier per `x-demo-lang` rules.
pub(crate) fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) const ARTIFACT_FORMAT: &str = "x-demo/puzzle-eval-v1";

/// Build the puzzle evaluation artifact content for a parsed puzzle.
pub(crate) fn compile_artifact(puzzle: &Puzzle, value: i64) -> serde_json::Value {
    serde_json::json!({
        "name": puzzle.name,
        "target": puzzle.target,
        "start": puzzle.start,
        "ops": puzzle
            .ops
            .iter()
            .map(|op| serde_json::json!({
                "name": op.name,
                "op": op.kind.symbol(),
                "arg": op.arg,
            }))
            .collect::<Vec<_>>(),
        "solution": puzzle.solution.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "value": value,
    })
}

/// Reconstruct canonical source text from a puzzle evaluation artifact.
/// Returns `None` when the artifact content is malformed.
pub(crate) fn reconstruct_source(content: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    let obj = value.as_object()?;
    let name = obj.get("name")?.as_str()?;
    let target = obj.get("target")?.as_i64()?;
    let start = obj.get("start")?.as_i64()?;
    let ops = obj.get("ops")?.as_array()?;
    let solution = obj.get("solution")?.as_array()?;
    let mut ops_out = Vec::new();
    for op in ops {
        let op = op.as_object()?;
        let name = op.get("name")?.as_str()?;
        let kind = op.get("op")?.as_str()?;
        let arg = op.get("arg")?.as_i64()?;
        let symbol = match kind {
            "+" => "+",
            "-" => "-",
            "*" => "*",
            "/" => "/",
            _ => return None,
        };
        ops_out.push(format!("    {name}: x => x {symbol} {arg}"));
    }
    let mut solution_out = Vec::new();
    for entry in solution {
        solution_out.push(entry.as_str()?.to_string());
    }
    let mut out = String::new();
    out.push_str(&format!("puzzle {name} {{\n"));
    out.push_str(&format!("  target = {target}\n"));
    out.push_str(&format!("  start = {start}\n"));
    out.push_str("  ops {\n");
    for op in ops_out {
        out.push_str(&op);
        out.push('\n');
    }
    out.push_str("  }\n");
    out.push_str(&format!("  solution = [ {} ]\n", solution_out.join(", ")));
    out.push('}');
    Some(out)
}

/// Normative edit application rules for `lpp/validateEdits`.
#[derive(Debug)]
pub(crate) enum EditValidation {
    Valid,
    Invalid {
        reason: &'static str,
        failing_edit_index: Option<usize>,
    },
}

pub(crate) fn validate_edits(text: &str, edits: &[(Range, String)]) -> EditValidation {
    let src = SourceText::new(text);

    // Rule 1: bounds and ordering of each edit range.
    for (i, (range, _)) in edits.iter().enumerate() {
        let start = src.byte_of(range.start);
        let end = src.byte_of(range.end);
        let ok = match (start, end) {
            (Some(s), Some(e)) => s <= e,
            _ => false,
        };
        if !ok {
            return EditValidation::Invalid {
                reason: "rangeOutOfBounds",
                failing_edit_index: Some(i),
            };
        }
    }

    // Rules 2-3: sort by range.start; overlapping edits are invalid. The
    // failing index refers to the original request order.
    let mut order: Vec<usize> = (0..edits.len()).collect();
    order.sort_by_key(|&i| {
        let r = &edits[i].0;
        (r.start.line, r.start.character)
    });
    for pair in order.windows(2) {
        let a = edits[pair[0]].0;
        let b = edits[pair[1]].0;
        let a_end = src.byte_of(a.end).expect("validated above");
        let b_start = src.byte_of(b.start).expect("validated above");
        if a_end > b_start {
            return EditValidation::Invalid {
                reason: "overlappingEdits",
                failing_edit_index: Some(order[pair[1]]),
            };
        }
    }

    // Rule 4: apply.
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    for &i in &order {
        let (range, new_text) = &edits[i];
        let start = src.byte_of(range.start).expect("validated above");
        let end = src.byte_of(range.end).expect("validated above");
        result.push_str(&text[cursor..start]);
        result.push_str(new_text);
        cursor = end;
    }
    result.push_str(&text[cursor..]);

    // Rule 5: the result must parse.
    let output = parse_document(&result);
    let has_errors = output.diagnostics.iter().any(|d| d.severity == "error");
    if has_errors {
        return EditValidation::Invalid {
            reason: "syntaxError",
            failing_edit_index: None,
        };
    }
    EditValidation::Valid
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN: &str = "puzzle clean {\n  target = 40\n  start = 10\n  ops {\n    double: x => x * 2\n    plus1: x => x + 1\n  }\n  solution = [ double, double ]\n}";
    const BROKEN: &str = "puzzle broken {\n  target = 40\n  start = 10\n  ops {\n    double: x => x * 2\n    double: x => x + 1\n  }\n  solution = [ double, double, triple ]\n}";
    const WARM: &str = "puzzle warm {\n  target = 42\n  start = 10\n  ops {\n    double: x => x * 2\n    plus1: x => x + 1\n  }\n  solution = [ double, double, plus1 ]\n}";

    #[test]
    fn parses_clean_puzzle() {
        let out = parse_document(CLEAN);
        assert!(out.diagnostics.is_empty());
        let puzzle = out.puzzle.expect("parsed");
        assert_eq!(puzzle.name, "clean");
        assert_eq!(puzzle.target, 40);
        assert_eq!(puzzle.start, 10);
        assert_eq!(puzzle.simulate(), Some(40));
    }

    #[test]
    fn reports_duplicate_and_unresolved() {
        let out = parse_document(BROKEN);
        let codes: Vec<&str> = out.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["x-demo/duplicate-op", "x-demo/unresolved-op"]);
    }

    #[test]
    fn warns_when_target_not_reached() {
        let out = parse_document(WARM);
        assert_eq!(out.diagnostics.len(), 1);
        let diagnostic = &out.diagnostics[0];
        assert_eq!(diagnostic.severity, "warning");
        assert_eq!(diagnostic.code, "x-demo/target-not-reached");
    }

    #[test]
    fn utf16_positions_are_exact() {
        // "puzzle αβ {" — UTF-16 units: p=0..e=5, space=6, α=7, β=8, space=9,
        // {=10. Bytes: α=7-8, β=9-10 (2 bytes each).
        let src = SourceText::new("puzzle αβ {\n  target = 40\n}");
        assert_eq!(
            src.byte_of(Position {
                line: 0,
                character: 7
            }),
            Some(7) // α
        );
        assert_eq!(
            src.byte_of(Position {
                line: 0,
                character: 8
            }),
            Some(9) // β
        );
        assert_eq!(
            src.byte_of(Position {
                line: 0,
                character: 9
            }),
            Some(11) // space after β
        );
        assert_eq!(
            src.position_of(9),
            Position {
                line: 0,
                character: 8
            }
        );
        assert_eq!(
            src.position_of(11),
            Position {
                line: 0,
                character: 9
            }
        );
        // End of line (character 11) and beyond.
        assert!(
            src.byte_of(Position {
                line: 0,
                character: 11
            })
            .is_some()
        );
        assert!(
            src.byte_of(Position {
                line: 0,
                character: 12
            })
            .is_none()
        );
    }

    #[test]
    fn compile_reconstruct_round_trip() {
        let out = parse_document(CLEAN);
        let puzzle = out.puzzle.expect("parsed");
        let artifact = compile_artifact(&puzzle, puzzle.simulate().expect("simulates"));
        let content = serde_json::to_string(&artifact).expect("serializes");
        let source = reconstruct_source(&content).expect("reconstructs");
        assert_eq!(source, CLEAN);
    }

    #[test]
    fn validate_edits_detects_overlap_in_original_order() {
        let edits = vec![
            (
                Range {
                    start: Position {
                        line: 7,
                        character: 15,
                    },
                    end: Position {
                        line: 7,
                        character: 21,
                    },
                },
                "a".to_string(),
            ),
            (
                Range {
                    start: Position {
                        line: 7,
                        character: 18,
                    },
                    end: Position {
                        line: 7,
                        character: 23,
                    },
                },
                "b".to_string(),
            ),
        ];
        match validate_edits(CLEAN, &edits) {
            EditValidation::Invalid {
                reason,
                failing_edit_index,
            } => {
                assert_eq!(reason, "overlappingEdits");
                assert_eq!(failing_edit_index, Some(1));
            }
            EditValidation::Valid => panic!("expected overlappingEdits"),
        }
    }

    #[test]
    fn validate_edits_accepts_rename_edits() {
        let edits = vec![
            (
                Range {
                    start: Position {
                        line: 4,
                        character: 4,
                    },
                    end: Position {
                        line: 4,
                        character: 10,
                    },
                },
                "twice".to_string(),
            ),
            (
                Range {
                    start: Position {
                        line: 7,
                        character: 15,
                    },
                    end: Position {
                        line: 7,
                        character: 21,
                    },
                },
                "twice".to_string(),
            ),
            (
                Range {
                    start: Position {
                        line: 7,
                        character: 23,
                    },
                    end: Position {
                        line: 7,
                        character: 29,
                    },
                },
                "twice".to_string(),
            ),
        ];
        assert!(matches!(
            validate_edits(CLEAN, &edits),
            EditValidation::Valid
        ));
    }
}
