use std::collections::HashSet;

use super::lex::{Token, TokenKind, describe, tokenize};
use super::text::{Position, Range, SourceText};
use super::{Diagnostic, Op, ParseOutput, Puzzle, SimError, SolutionEntry, diagnostic};

pub(crate) fn parse_document(text: &str) -> ParseOutput {
    let src = SourceText::new(text);
    let mut parser = Parser {
        tokens: tokenize(&src, text),
        pos: 0,
        diagnostics: Vec::new(),
    };
    let puzzle = parse_top(&mut parser);
    sort_diagnostics_by_range_start(&mut parser.diagnostics);
    ParseOutput {
        puzzle,
        diagnostics: parser.diagnostics,
    }
}

fn sort_diagnostics_by_range_start(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by_key(|d| (d.range.start.line, d.range.start.character));
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
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

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Some(tok) if tok.kind == TokenKind::Newline) {
            self.advance();
        }
    }

    fn recover_line(&mut self) {
        while let Some(tok) = self.advance() {
            if tok.kind == TokenKind::Newline {
                break;
            }
        }
    }

    fn eof_error(&mut self, what: &str) {
        let at = self
            .tokens
            .last()
            .map(|t| Range::point(t.range.end))
            .unwrap_or_else(|| Range::point(Position::ORIGIN));
        self.error_at(at, format!("unexpected end of input: expected {what}"));
    }

    fn error_at(&mut self, range: Range, message: impl Into<String>) {
        self.diagnostics
            .push(diagnostic(range, "error", "x-demo/syntax", message.into()));
    }

    fn error(&mut self, tok: &Token, message: impl Into<String>) {
        self.error_at(tok.range, message);
        self.recover_line();
    }

    fn unexpected(&mut self, tok: &Token, context: &str) {
        match &tok.kind {
            TokenKind::Ident(word) => {
                self.error(tok, format!("unexpected '{word}' in {context}"));
            }
            _ => self.error(tok, format!("unexpected {} in {context}", describe(tok))),
        }
    }

    fn expect_ident(&mut self) -> Option<(String, Token)> {
        let tok = self.peek().cloned()?;
        let TokenKind::Ident(name) = &tok.kind else {
            self.error(
                &tok,
                format!("expected identifier, found {}", describe(&tok)),
            );
            return None;
        };
        let name = name.clone();
        self.pos += 1;
        Some((name, tok))
    }

    fn expect_number(&mut self, what: &str) -> Option<i64> {
        let tok = self.peek().cloned()?;
        match tok.kind {
            TokenKind::Number(n) => {
                self.pos += 1;
                Some(n)
            }
            TokenKind::BadNumber => {
                self.error(&tok, "integer literal out of range");
                None
            }
            _ => {
                self.error(&tok, format!("expected {what}"));
                None
            }
        }
    }

    fn expect_punct(&mut self, kind: TokenKind, what: &str) -> bool {
        match self.peek().cloned() {
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

    fn expect_x(&mut self, what: &str) -> bool {
        match self.expect_ident() {
            Some((name, _)) if name == "x" => true,
            Some((name, tok)) => {
                self.error(&tok, format!("{what}, found '{name}'"));
                false
            }
            None => false,
        }
    }
}

fn parse_top(parser: &mut Parser) -> Option<Puzzle> {
    parser.skip_newlines();
    let Some(first) = parser.peek().cloned() else {
        parser.error_at(
            Range::point(Position::ORIGIN),
            "empty document: expected 'puzzle <name> { ... }'",
        );
        return None;
    };
    match &first.kind {
        TokenKind::Ident(name) if name == "puzzle" => {
            parser.advance();
        }
        _ => {
            let found = describe(&first);
            parser.error(&first, format!("expected 'puzzle', found {found}"));
            return None;
        }
    }
    if parser.peek().is_none() {
        parser.eof_error("a puzzle name");
        return None;
    }
    let (name, name_tok) = parser.expect_ident()?;
    let mut puzzle = Puzzle {
        name,
        name_range: name_tok.range,
        target: 0,
        start: 0,
        ops: Vec::new(),
        solution: Vec::new(),
        solution_range: Range::point(Position::ORIGIN),
    };
    if parser.peek().is_none() {
        parser.eof_error("'{'");
        return None;
    }
    if !parser.expect_punct(TokenKind::LBrace, "'{'") {
        return None;
    }

    let mut has_target = false;
    let mut has_start = false;
    let mut has_ops = false;
    let mut has_solution = false;

    loop {
        let Some(tok) = parser.peek().cloned() else {
            parser.error_at(puzzle.name_range, "unterminated puzzle: missing '}'");
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
            TokenKind::Ident(word) => {
                parser.advance();
                match word.as_str() {
                    "target" | "start" => {
                        if parser.expect_punct(TokenKind::Eq, "'='") {
                            if let Some(n) = parser.expect_number(&format!("a {word} value")) {
                                if word == "target" {
                                    has_target = true;
                                    puzzle.target = n;
                                } else {
                                    has_start = true;
                                    puzzle.start = n;
                                }
                            }
                        }
                    }
                    "ops" => has_ops = parse_ops(parser, &mut puzzle),
                    "solution" => has_solution = parse_solution(parser, &mut puzzle),
                    _ => parser.unexpected(&tok, "puzzle body"),
                }
            }
            _ => parser.unexpected(&tok, "puzzle body"),
        }
    }

    while let Some(tok) = parser.peek().cloned() {
        if tok.kind == TokenKind::Newline {
            parser.advance();
        } else {
            let message = format!("unexpected content after puzzle: {}", describe(&tok));
            parser.error(&tok, message);
        }
    }

    for (section, present) in [
        ("target", has_target),
        ("start", has_start),
        ("ops", has_ops),
        ("solution", has_solution),
    ] {
        if !present {
            parser.diagnostics.push(diagnostic(
                puzzle.name_range,
                "error",
                "x-demo/missing-section",
                format!("puzzle is missing section '{section}'"),
            ));
        }
    }

    let mut seen = HashSet::new();
    for op in &puzzle.ops {
        if !seen.insert(&op.name) {
            parser.diagnostics.push(diagnostic(
                op.name_range,
                "error",
                "x-demo/duplicate-op",
                format!("duplicate op name '{}'", op.name),
            ));
        }
    }

    for entry in &puzzle.solution {
        if puzzle.op(&entry.name).is_none() {
            parser.diagnostics.push(diagnostic(
                entry.range,
                "error",
                "x-demo/unresolved-op",
                format!("unresolved op reference '{}'", entry.name),
            ));
        }
    }

    if !parser.diagnostics.iter().any(|d| d.severity == "error") {
        let warning = if puzzle.solution.is_empty() {
            Some(("x-demo/empty-solution", "solution is empty".to_string()))
        } else {
            match puzzle.simulate() {
                Ok(value) if value != puzzle.target => Some((
                    "x-demo/target-not-reached",
                    format!(
                        "solution does not reach target: expected {}, reached {}",
                        puzzle.target, value
                    ),
                )),
                Ok(_) | Err(SimError::UnknownOp) => None,
                Err(SimError::Arithmetic) => {
                    parser.diagnostics.push(diagnostic(
                        puzzle.solution_range,
                        "error",
                        "x-demo/simulation-failed",
                        "solution cannot be evaluated: division by zero or integer overflow"
                            .to_string(),
                    ));
                    None
                }
            }
        };
        if let Some((code, message)) = warning {
            parser
                .diagnostics
                .push(diagnostic(puzzle.solution_range, "warning", code, message));
        }
    }

    Some(puzzle)
}

fn parse_ops(parser: &mut Parser, puzzle: &mut Puzzle) -> bool {
    if !parser.expect_punct(TokenKind::LBrace, "'{'") {
        return false;
    }
    loop {
        let Some(tok) = parser.peek().cloned() else {
            parser.error_at(puzzle.name_range, "unterminated ops block: missing '}'");
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
            TokenKind::Ident(_) => {
                parser.advance();
                parse_op(parser, puzzle, &tok);
            }
            _ => parser.unexpected(&tok, "ops block"),
        }
    }
}

fn parse_op(parser: &mut Parser, puzzle: &mut Puzzle, name_tok: &Token) {
    let TokenKind::Ident(name) = &name_tok.kind else {
        return;
    };
    let name = name.clone();
    if !parser.expect_punct(TokenKind::Colon, "':'") {
        return;
    }
    if !parser.expect_x("op parameter must be 'x'") {
        return;
    }
    if !parser.expect_punct(TokenKind::Arrow, "'=>'") {
        return;
    }
    if !parser.expect_x("op body must apply to 'x'") {
        return;
    }
    let Some(op_tok) = parser.advance().cloned() else {
        parser.recover_line();
        return;
    };
    let Some(kind) = op_tok.kind.as_op() else {
        parser.error(
            &op_tok,
            "expected arithmetic operator '+', '-', '*', or '/'",
        );
        return;
    };
    let Some(arg) = parser.expect_number("a numeric argument") else {
        return;
    };
    puzzle.ops.push(Op {
        name,
        name_range: name_tok.range,
        kind,
        arg,
    });
}

fn parse_solution(parser: &mut Parser, puzzle: &mut Puzzle) -> bool {
    if !parser.expect_punct(TokenKind::Eq, "'='") {
        return false;
    }
    let Some(open) = parser.peek().cloned() else {
        return false;
    };
    let start = open.range.start;
    if !parser.expect_punct(TokenKind::LBracket, "'['") {
        return false;
    }
    loop {
        let Some(tok) = parser.peek().cloned() else {
            parser.error_at(
                Range::point(start),
                "unterminated solution list: missing ']'",
            );
            return false;
        };
        match &tok.kind {
            TokenKind::RBracket => {
                puzzle.solution_range = Range {
                    start,
                    end: tok.range.end,
                };
                parser.advance();
                return true;
            }
            TokenKind::Comma => {
                parser.advance();
            }
            TokenKind::Ident(name) => {
                puzzle.solution.push(SolutionEntry {
                    name: name.clone(),
                    range: tok.range,
                });
                parser.advance();
            }
            _ => parser.unexpected(&tok, "solution list"),
        }
    }
}
