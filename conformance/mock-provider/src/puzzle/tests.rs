use super::Puzzle;
use super::artifact::{compile_artifact, reconstruct_source};
use super::catalog::{LookupScope, lookup};
use super::edits::{EditValidation, validate_edits};
use super::parse::parse_document;
use super::symbols::{Symbol, is_valid_identifier};
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
fn unknown_bytes_are_syntax_errors_not_identifiers() {
    // '@', '$', and multi-byte characters are not part of the token grammar:
    // they must surface as syntax errors, never parse as identifiers, and
    // must not panic the diagnostic path.
    for src in [
        "puzzle @ {\n  target = 1\n  start = 0\n  ops {}\n  solution = []\n}",
        "puzzle p {\n  target = 1\n  start = 0\n  ops {\n    $: x => x + 1\n  }\n  solution = [ $ ]\n}",
        "puzzle α {\n  target = 1\n  start = 0\n  ops {}\n  solution = []\n}",
    ] {
        let out = parse_document(src);
        assert!(
            out.diagnostics.iter().any(|d| d.severity == "error"),
            "expected a syntax error for {src:?}"
        );
    }
}

#[test]
fn truncated_document_reports_an_error_instead_of_no_puzzle() {
    // A document ending right after `puzzle` or `puzzle <name>` must produce
    // an error diagnostic, not a silent `puzzle: None` — `lpp/compile`
    // assumes no errors implies a puzzle.
    for src in ["puzzle", "puzzle p", "puzzle p "] {
        let out = parse_document(src);
        assert!(out.puzzle.is_none(), "unexpectedly parsed {src:?}");
        assert!(
            out.diagnostics.iter().any(|d| d.severity == "error"),
            "expected a diagnostic for {src:?}"
        );
    }
}

#[test]
fn diagnostics_are_sorted_by_range_start() {
    // A syntax error late in the body plus a missing-section diagnostic at
    // the puzzle name must still come out ordered by range.start (§9.2).
    const SRC: &str = "puzzle p {\n  start = 1\n  ops {\n    d: x => x + 1\n  }\n  solution = [ d ]\n  ???\n}";
    let out = parse_document(SRC);
    let mut sorted = out.diagnostics.clone();
    sorted.sort_by_key(|d| (d.range.start.line, d.range.start.character));
    assert_eq!(
        out.diagnostics.iter().map(|d| &d.code).collect::<Vec<_>>(),
        sorted.iter().map(|d| &d.code).collect::<Vec<_>>(),
        "diagnostics are not sorted by range.start"
    );
}

#[test]
fn arithmetic_failure_is_an_error_not_silence() {
    // Division by zero and i64 overflow make simulate() return None; that
    // must surface as an error diagnostic or compile's "no errors implies
    // resolvable ops" invariant breaks.
    for src in [
        "puzzle p {\n  target = 1\n  start = 1\n  ops {\n    d: x => x / 0\n  }\n  solution = [ d ]\n}",
        "puzzle p {\n  target = 1\n  start = 9223372036854775807\n  ops {\n    d: x => x + 1\n  }\n  solution = [ d ]\n}",
    ] {
        let out = parse_document(src);
        assert!(
            out.diagnostics.iter().any(|d| d.severity == "error"),
            "expected an error diagnostic for {src:?}"
        );
    }
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
fn trailing_newline_phantom_line_is_a_valid_position() {
    // A document ending in '\n' has a phantom empty last line; a position
    // there is valid (spec §6.4) and must not invert the line's byte range.
    let src = SourceText::new("a\n");
    assert_eq!(src.line_text(0), "a");
    assert_eq!(src.line_text(1), "");
    assert_eq!(src.byte_of(pos(1, 0)), Some(2));

    let src = SourceText::new("a\r\n");
    assert_eq!(src.line_text(1), "");
    assert_eq!(src.byte_of(pos(1, 0)), Some(3));

    // An empty line between two newlines stays empty, not inverted.
    let src = SourceText::new("a\n\nb");
    assert_eq!(src.line_text(1), "");
    assert_eq!(src.byte_of(pos(1, 0)), Some(2));
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

#[test]
fn lookup_ranks_tiers_then_catalog_order() {
    // "a": prefix match on "add" and "arithmetic operator" (tier 1); substring
    // on "subtract", "target", "start" (tier 2); ties keep catalog order.
    let entries = lookup("a", None, &LookupScope::All, 20).expect("all scope");
    let ids: Vec<&str> = entries
        .iter()
        .filter_map(|e| e["identity"].as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "x-demo:operator/add",
            "x-demo:enum/operator",
            "x-demo:operator/subtract",
            "x-demo:setting/target",
            "x-demo:setting/start",
        ]
    );
}

#[test]
fn lookup_within_scopes_children() {
    // Enum scope lists member spellings in domain order.
    let members = lookup(
        "",
        None,
        &LookupScope::Enum("x-demo:enum/operator".into()),
        20,
    )
    .expect("known domain");
    let spellings: Vec<&str> = members
        .iter()
        .filter_map(|e| e["spelling"].as_str())
        .collect();
    assert_eq!(spellings, ["+", "-", "*", "/"]);

    // Callable scope lists the callable's parameters in call order.
    let params = lookup(
        "",
        None,
        &LookupScope::Callable("x-demo:operator/add".into()),
        20,
    )
    .expect("known callable");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0]["spelling"], "arg");
    assert_eq!(params[0]["parameter"]["required"], true);
}

#[test]
fn lookup_unknown_within_is_none() {
    // A non-callable identity, an unknown identity, and a leaf settings path
    // each name no scope.
    for scope in [
        LookupScope::Callable("x-demo:keyword/ops".into()),
        LookupScope::Enum("x-demo:setting/target".into()),
        LookupScope::Settings("target".into()),
    ] {
        assert!(lookup("", None, &scope, 20).is_none());
    }
}

// -- Symbol queries (lpp/symbols, definition, references, rename) ----------

/// Parses `text` into a puzzle and returns it with its source text.
fn puzzle_of(text: &str) -> (SourceText<'_>, Puzzle) {
    let src = SourceText::new(text);
    let puzzle = parse_document(text).puzzle.expect("parsed");
    (src, puzzle)
}

/// The symbol under `line:character`, resolved the way the wire handlers do.
fn symbol_at(src: &SourceText<'_>, puzzle: &Puzzle, line: u32, character: u32) -> Option<Symbol> {
    puzzle.symbol_at(src, src.byte_of(pos(line, character)).expect("in document"))
}

#[test]
fn document_symbols_lists_puzzle_then_ops_in_declaration_order() {
    let (_, puzzle) = puzzle_of(CLEAN);
    let symbols = serde_json::to_value(puzzle.document_symbols()).expect("serializes");
    assert_eq!(
        symbols,
        serde_json::json!([
            { "name": "clean", "kind": "puzzle", "range": rng(0, 7, 0, 12) },
            { "name": "double", "kind": "op", "range": rng(4, 4, 4, 10) },
            { "name": "plus1", "kind": "op", "range": rng(5, 4, 5, 9) },
        ])
    );
}

#[test]
fn symbol_at_resolves_puzzle_op_declaration_and_reference() {
    let (src, puzzle) = puzzle_of(CLEAN);

    match symbol_at(&src, &puzzle, 0, 9).expect("puzzle name") {
        Symbol::Puzzle { name } => assert_eq!(name, "clean"),
        Symbol::Op { .. } => panic!("expected puzzle symbol"),
    }
    // An op declaration and each solution reference resolve to the same op.
    for (line, character) in [(4, 6), (7, 16), (7, 25)] {
        match symbol_at(&src, &puzzle, line, character).expect("op") {
            Symbol::Op { name } => assert_eq!(name, "double"),
            Symbol::Puzzle { .. } => panic!("expected op symbol at {line}:{character}"),
        }
    }
    match symbol_at(&src, &puzzle, 5, 5).expect("op") {
        Symbol::Op { name } => assert_eq!(name, "plus1"),
        Symbol::Puzzle { .. } => panic!("expected op symbol"),
    }

    // Keywords, settings, and punctuation are not symbols.
    for (line, character) in [(0, 3), (1, 4), (7, 3), (7, 11)] {
        assert!(symbol_at(&src, &puzzle, line, character).is_none());
    }
}

#[test]
fn declaration_range_points_at_the_declaration() {
    let (src, puzzle) = puzzle_of(CLEAN);
    let at = |line, character| symbol_at(&src, &puzzle, line, character).expect("symbol");

    // The puzzle name is its own declaration.
    assert_eq!(puzzle.declaration_range(&at(0, 9)), Some(rng(0, 7, 0, 12)));
    // An op declaration and its solution references share one declaration.
    for (line, character) in [(4, 6), (7, 16), (7, 25)] {
        assert_eq!(
            puzzle.declaration_range(&at(line, character)),
            Some(rng(4, 4, 4, 10))
        );
    }
}

#[test]
fn occurrences_list_declaration_first_then_references() {
    let (_, puzzle) = puzzle_of(CLEAN);
    assert_eq!(
        puzzle.occurrences("double", true),
        vec![rng(4, 4, 4, 10), rng(7, 15, 7, 21), rng(7, 23, 7, 29)]
    );
    assert_eq!(
        puzzle.occurrences("double", false),
        vec![rng(7, 15, 7, 21), rng(7, 23, 7, 29)]
    );
    // A declared-but-unreferenced op lists only its declaration.
    assert_eq!(puzzle.occurrences("plus1", true), vec![rng(5, 4, 5, 9)]);
    // Unknown names have no occurrences.
    assert!(puzzle.occurrences("triple", true).is_empty());
}

#[test]
fn rename_ranges_are_sorted_when_solution_precedes_ops() {
    // `solution` may precede `ops` in a document; rename edits must still be
    // sorted by range.start (§15.2) even though occurrences lists the
    // declaration first.
    const SRC: &str =
        "puzzle p {\n  target = 2\n  start = 1\n  solution = [ d ]\n  ops {\n    d: x => x + 1\n  }\n}";
    let (src, puzzle) = puzzle_of(SRC);
    let symbol = symbol_at(&src, &puzzle, 5, 4).expect("op d");
    let ranges = puzzle.rename_ranges(&symbol, true);
    let mut sorted = ranges.clone();
    sorted.sort_by_key(|r| (r.start.line, r.start.character));
    assert_eq!(ranges, sorted, "rename ranges are not sorted");
}

#[test]
fn rename_ranges_for_op_cover_declaration_and_references() {
    let (src, puzzle) = puzzle_of(CLEAN);
    let symbol = symbol_at(&src, &puzzle, 7, 16).expect("symbol");
    // Op renames edit every document that mentions the op, positioned or not.
    for is_position_doc in [true, false] {
        assert_eq!(
            puzzle.rename_ranges(&symbol, is_position_doc),
            vec![rng(4, 4, 4, 10), rng(7, 15, 7, 21), rng(7, 23, 7, 29)]
        );
    }
}

#[test]
fn rename_ranges_for_puzzle_name_only_edit_the_position_document() {
    let (src, puzzle) = puzzle_of(CLEAN);
    let symbol = symbol_at(&src, &puzzle, 0, 9).expect("symbol");
    assert_eq!(puzzle.rename_ranges(&symbol, true), vec![rng(0, 7, 0, 12)]);
    assert!(puzzle.rename_ranges(&symbol, false).is_empty());
}

#[test]
fn rename_collides_only_with_another_op() {
    let (src, puzzle) = puzzle_of(CLEAN);
    let op = symbol_at(&src, &puzzle, 7, 16).expect("op");
    let puzzle_name = symbol_at(&src, &puzzle, 0, 9).expect("puzzle name");

    assert!(puzzle.rename_collides(&op, "plus1"));
    // Renaming to the current name is not a collision.
    assert!(!puzzle.rename_collides(&op, "double"));
    assert!(!puzzle.rename_collides(&op, "triple"));
    // Puzzle-name renames never collide with ops.
    assert!(!puzzle.rename_collides(&puzzle_name, "plus1"));
}

#[test]
fn identifier_rules_match_the_lexer() {
    assert!(is_valid_identifier("double"));
    assert!(is_valid_identifier("_x9"));
    assert!(!is_valid_identifier("9x"));
    assert!(!is_valid_identifier(""));
    assert!(!is_valid_identifier("a-b"));
}

#[test]
fn resolve_ids_omits_unmapped_ids_and_empty_domains() {
    use super::spellings::resolve_ids;
    use std::collections::BTreeMap;
    let enums = BTreeMap::from([
        (
            "Status".to_string(),
            vec!["STUNNED".into(), "HASTED".into()],
        ),
        ("Team".to_string(), vec!["ALL".into()]),
    ]);
    let result = resolve_ids(&["applyImpulse".into(), "noSuchCall".into()], &enums);
    assert_eq!(
        result,
        serde_json::json!({
            "calls": { "applyImpulse": "shove" },
            "enums": { "Status": { "STUNNED": "stunned" } }
        })
    );
}
