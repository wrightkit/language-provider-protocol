use super::artifact::{compile_artifact, reconstruct_source};
use super::edits::{EditValidation, validate_edits};
use super::parse::parse_document;
use super::text::{Position, Range, SourceText};

const CLEAN: &str = "puzzle clean {\n  target = 40\n  start = 10\n  ops {\n    double: x => x * 2\n    plus1: x => x + 1\n  }\n  solution = [ double, double ]\n}";
const BROKEN: &str = "puzzle broken {\n  target = 40\n  start = 10\n  ops {\n    double: x => x * 2\n    double: x => x + 1\n  }\n  solution = [ double, double, triple ]\n}";
const WARM: &str = "puzzle warm {\n  target = 42\n  start = 10\n  ops {\n    double: x => x * 2\n    plus1: x => x + 1\n  }\n  solution = [ double, double, plus1 ]\n}";

fn pos(line: u32, character: u32) -> Position {
    Position { line, character }
}

fn rng(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
    Range {
        start: pos(start_line, start_char),
        end: pos(end_line, end_char),
    }
}

fn edit(
    start_line: u32,
    start_char: u32,
    end_line: u32,
    end_char: u32,
    text: &str,
) -> (Range, String) {
    (
        rng(start_line, start_char, end_line, end_char),
        text.to_string(),
    )
}

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
fn malformed_op_declaration_skips_following_line() {
    // Reference recovery behavior: a bad op parameter consumes the rest of
    // that line AND the next line of the ops block, so `later` is never
    // declared and the solution reference stays unresolved.
    const SRC: &str = "puzzle t {\n  target = 2\n  start = 0\n  ops {\n    bad: 5 => x + 1\n    later: x => x * 2\n  }\n  solution = [ later ]\n}";
    let out = parse_document(SRC);
    let codes: Vec<&str> = out.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["x-demo/syntax", "x-demo/unresolved-op"]);
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
    // "puzzle αβ {": UTF-16 units: p=0..e=5, space=6, α=7, β=8, space=9,
    // {=10. Bytes: α=7-8, β=9-10 (2 bytes each).
    let src = SourceText::new("puzzle αβ {\n  target = 40\n}");
    assert_eq!(src.byte_of(pos(0, 7)), Some(7)); // α
    assert_eq!(src.byte_of(pos(0, 8)), Some(9)); // β
    assert_eq!(src.byte_of(pos(0, 9)), Some(11)); // space after β
    assert_eq!(src.position_of(9), pos(0, 8));
    assert_eq!(src.position_of(11), pos(0, 9));
    assert!(src.byte_of(pos(0, 11)).is_some());
    assert!(src.byte_of(pos(0, 12)).is_none());
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
    let edits = vec![edit(7, 15, 7, 21, "a"), edit(7, 18, 7, 23, "b")];
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
fn validate_edits_detects_overlap_when_unsorted() {
    // The failing index refers to the request's edit array order, not the
    // document-sorted order.
    let edits = vec![edit(7, 18, 7, 23, "b"), edit(7, 15, 7, 21, "a")];
    match validate_edits(CLEAN, &edits) {
        EditValidation::Invalid {
            reason,
            failing_edit_index,
        } => {
            assert_eq!(reason, "overlappingEdits");
            assert_eq!(failing_edit_index, Some(0));
        }
        EditValidation::Valid => panic!("expected overlappingEdits"),
    }
}

#[test]
fn validate_edits_accepts_rename_edits() {
    let edits = vec![
        edit(4, 4, 4, 10, "twice"),
        edit(7, 15, 7, 21, "twice"),
        edit(7, 23, 7, 29, "twice"),
    ];
    assert!(matches!(
        validate_edits(CLEAN, &edits),
        EditValidation::Valid
    ));
}
