//! The `x-demo-lang` puzzle language: a document declares one equation puzzle
//! (target, start, named arithmetic ops, and a solution applying ops in
//! sequence). The language is deliberately unlike OPY and DEL so the mock
//! provider cannot accidentally satisfy two-language assumptions in LPP.

use serde::Serialize;

mod artifact;
mod catalog;
mod edits;
mod lex;
mod parse;
mod text;

pub(crate) use artifact::{
    ARTIFACT_FORMAT, COMPILE_ARTIFACT_FORMATS, SUMMARY_ARTIFACT_FORMAT, compile_artifact,
    reconstruct_source, summary_artifact,
};
pub(crate) use catalog::{LookupScope, lookup};
pub(crate) use edits::{EditValidation, validate_edits};
pub(crate) use parse::parse_document;
pub(crate) use text::{Position, Range, SourceText};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Diagnostic {
    pub range: Range,
    pub severity: String,
    pub code: String,
    pub message: String,
    pub source: String,
}

pub(crate) fn diagnostic(
    range: Range,
    severity: &str,
    code: &str,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        range,
        severity: severity.to_string(),
        code: code.to_string(),
        message: message.into(),
        source: "x-demo-lang".to_string(),
    }
}

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

impl ParseOutput {
    /// The puzzle when the document parsed without error diagnostics.
    pub(crate) fn into_puzzle(self) -> Option<Puzzle> {
        match self.puzzle {
            Some(puzzle) if !self.has_errors() => Some(puzzle),
            _ => None,
        }
    }

    pub(crate) fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == "error")
    }
}

pub(crate) const KIND_PUZZLE: &str = "puzzle";
pub(crate) const KIND_OP: &str = "op";

#[derive(Debug, Clone)]
pub(crate) enum Symbol {
    Puzzle { name: String, range: Range },
    Op { name: String },
}

impl Symbol {
    pub(crate) fn name(&self) -> &str {
        match self {
            Symbol::Puzzle { name, .. } | Symbol::Op { name } => name,
        }
    }
}

/// The symbol under `byte`: the puzzle name, an op declaration, or a solution
/// reference, in that order.
pub(crate) fn symbol_at(src: &SourceText<'_>, puzzle: &Puzzle, byte: usize) -> Option<Symbol> {
    if src.contains_byte(puzzle.name_range, byte) {
        return Some(Symbol::Puzzle {
            name: puzzle.name.clone(),
            range: puzzle.name_range,
        });
    }
    puzzle
        .ops
        .iter()
        .map(|op| (op.name_range, &op.name))
        .chain(puzzle.solution.iter().map(|e| (e.range, &e.name)))
        .find(|(range, _)| src.contains_byte(*range, byte))
        .map(|(_, name)| Symbol::Op { name: name.clone() })
}

pub(crate) fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
