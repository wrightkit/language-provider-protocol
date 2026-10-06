//! Symbol semantics for `x-demo-lang`: the queries behind `lpp/symbols`,
//! `lpp/definition`, `lpp/references`, and `lpp/rename`.
//!
//! Each query answers a language question with a typed result; the handlers
//! in `methods.rs` shape those results into wire envelopes and refusals.

use serde::Serialize;

use super::Puzzle;
use super::text::{Range, SourceText};

/// `kind` values for `lpp/symbols` entries: the language's declaration
/// vocabulary.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum SymbolKind {
    Puzzle,
    Op,
}

/// A symbol declared in a document, as listed by `lpp/symbols`.
#[derive(Debug, Serialize)]
pub(crate) struct DocumentSymbol {
    name: String,
    kind: SymbolKind,
    range: Range,
}

/// The symbol a position resolves to: the puzzle's name, or an op via its
/// declaration or a solution reference.
#[derive(Debug, Clone)]
pub(crate) enum Symbol {
    Puzzle { name: String },
    Op { name: String },
}

impl Symbol {
    /// The name the symbol binds to.
    pub(crate) fn name(&self) -> &str {
        match self {
            Symbol::Puzzle { name } | Symbol::Op { name } => name,
        }
    }
}

impl Puzzle {
    /// The symbol under `byte`: the puzzle name, an op declaration, or a
    /// solution reference, in that order.
    pub(crate) fn symbol_at(&self, src: &SourceText<'_>, byte: usize) -> Option<Symbol> {
        if src.contains_byte(self.name_range, byte) {
            return Some(Symbol::Puzzle {
                name: self.name.clone(),
            });
        }
        self.ops
            .iter()
            .map(|op| (op.name_range, &op.name))
            .chain(self.solution.iter().map(|e| (e.range, &e.name)))
            .find(|(range, _)| src.contains_byte(*range, byte))
            .map(|(_, name)| Symbol::Op { name: name.clone() })
    }

    /// The symbols this puzzle declares, in declaration order: the puzzle
    /// name, then each op.
    pub(crate) fn document_symbols(&self) -> Vec<DocumentSymbol> {
        let mut symbols = vec![DocumentSymbol {
            name: self.name.clone(),
            kind: SymbolKind::Puzzle,
            range: self.name_range,
        }];
        symbols.extend(self.ops.iter().map(|op| DocumentSymbol {
            name: op.name.clone(),
            kind: SymbolKind::Op,
            range: op.name_range,
        }));
        symbols
    }

    /// The declaration range `symbol` resolves to: its own range for the
    /// puzzle name, the op's declaration range for an op.
    pub(crate) fn declaration_range(&self, symbol: &Symbol) -> Option<Range> {
        match symbol {
            Symbol::Puzzle { .. } => Some(self.name_range),
            Symbol::Op { name } => self.op(name).map(|op| op.name_range),
        }
    }

    /// Ranges where `name` occurs as an op: its declaration (first, when
    /// `include_declaration`), then solution references in document order.
    pub(crate) fn occurrences(&self, name: &str, include_declaration: bool) -> Vec<Range> {
        let mut ranges = Vec::new();
        if include_declaration {
            if let Some(op) = self.op(name) {
                ranges.push(op.name_range);
            }
        }
        ranges.extend(
            self.solution
                .iter()
                .filter(|entry| entry.name == name)
                .map(|entry| entry.range),
        );
        ranges
    }

    /// Whether renaming `symbol` to `new_name` would collide with an
    /// existing op. Only op renames can collide.
    pub(crate) fn rename_collides(&self, symbol: &Symbol, new_name: &str) -> bool {
        let Symbol::Op { name } = symbol else {
            return false;
        };
        self.op(name).is_some() && name.as_str() != new_name && self.op(new_name).is_some()
    }

    /// The ranges a rename of `symbol` edits in this puzzle. A puzzle-name
    /// rename only edits the document holding the position; an op rename
    /// edits the op's declaration and every solution reference to it.
    pub(crate) fn rename_ranges(&self, symbol: &Symbol, is_position_doc: bool) -> Vec<Range> {
        match symbol {
            Symbol::Puzzle { name } => {
                if is_position_doc && self.name == *name {
                    vec![self.name_range]
                } else {
                    Vec::new()
                }
            }
            Symbol::Op { name } => self.occurrences(name, true),
        }
    }
}

/// `x-demo-lang` identifier: `[A-Za-z_][A-Za-z0-9_]*`.
pub(crate) fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
