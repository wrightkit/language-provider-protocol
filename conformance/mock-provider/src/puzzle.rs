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
mod symbols;
mod text;

pub(crate) use artifact::{
    ARTIFACT_FORMAT, COMPILE_ARTIFACT_FORMATS, SUMMARY_ARTIFACT_FORMAT, compile_artifact,
    reconstruct_source, summary_artifact,
};
pub(crate) use catalog::{LookupScope, lookup};
pub(crate) use edits::{EditValidation, validate_edits};
pub(crate) use parse::parse_document;
pub(crate) use symbols::{Symbol, is_valid_identifier};
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
enum OpKind {
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
struct Op {
    name: String,
    name_range: Range,
    kind: OpKind,
    arg: i64,
}

#[derive(Debug, Clone)]
struct SolutionEntry {
    name: String,
    range: Range,
}

#[derive(Debug, Clone)]
pub(crate) struct Puzzle {
    name: String,
    name_range: Range,
    target: i64,
    start: i64,
    ops: Vec<Op>,
    solution: Vec<SolutionEntry>,
    solution_range: Range,
}

impl Puzzle {
    fn op(&self, name: &str) -> Option<&Op> {
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
