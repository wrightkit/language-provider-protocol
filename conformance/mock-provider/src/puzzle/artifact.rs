//! Artifact formats produced by `lpp/compile` and consumed by
//! `lpp/reconstruct`. Artifact payloads are opaque to LPP; only the mock
//! provider interprets them.

use serde::Deserialize;
use serde_json::{Value, json};
use std::fmt::Write;

use super::Puzzle;

pub(crate) const ARTIFACT_FORMAT: &str = "x-demo/puzzle-eval-v1";
/// Compile-only format: `lpp/reconstruct` does not support it.
pub(crate) const SUMMARY_ARTIFACT_FORMAT: &str = "x-demo/puzzle-summary-v1";
pub(crate) const COMPILE_ARTIFACT_FORMATS: [&str; 2] = [ARTIFACT_FORMAT, SUMMARY_ARTIFACT_FORMAT];

pub(crate) fn summary_artifact(puzzle: &Puzzle, value: i64) -> Value {
    json!({ "name": puzzle.name, "value": value })
}

pub(crate) fn compile_artifact(puzzle: &Puzzle, value: i64) -> Value {
    json!({
        "name": puzzle.name,
        "target": puzzle.target,
        "start": puzzle.start,
        "ops": puzzle
            .ops
            .iter()
            .map(|op| json!({ "name": op.name, "op": op.kind.symbol(), "arg": op.arg }))
            .collect::<Vec<_>>(),
        "solution": puzzle.solution.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "value": value,
    })
}

#[derive(Deserialize)]
struct Sheet {
    name: String,
    target: i64,
    start: i64,
    ops: Vec<SheetOp>,
    solution: Vec<String>,
}

#[derive(Deserialize)]
struct SheetOp {
    name: String,
    op: String,
    arg: i64,
}

/// Canonical source text regenerated from a puzzle evaluation sheet.
pub(crate) fn reconstruct_source(content: &str) -> Option<String> {
    let sheet: Sheet = serde_json::from_str(content).ok()?;
    let mut out = format!(
        "puzzle {} {{\n  target = {}\n  start = {}\n  ops {{\n",
        sheet.name, sheet.target, sheet.start
    );
    for op in &sheet.ops {
        if !["+", "-", "*", "/"].contains(&op.op.as_str()) {
            return None;
        }
        writeln!(out, "    {}: x => x {} {}", op.name, op.op, op.arg).ok()?;
    }
    write!(
        out,
        "  }}\n  solution = [ {} ]\n}}",
        sheet.solution.join(", ")
    )
    .ok()?;
    Some(out)
}
