use serde::Serialize;

use super::Puzzle;
use super::text::{Range, SourceText};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum SymbolKind {
    Puzzle,
    Op,
}

#[derive(Debug, Serialize)]
pub(crate) struct DocumentSymbol {
    name: String,
    kind: SymbolKind,
    range: Range,
}

#[derive(Debug, Clone)]
pub(crate) enum Symbol {
    Puzzle { name: String },
    Op { name: String },
}

impl Puzzle {
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

    pub(crate) fn declaration_range(&self, symbol: &Symbol) -> Option<Range> {
        match symbol {
            Symbol::Puzzle { .. } => Some(self.name_range),
            Symbol::Op { name } => self.op(name).map(|op| op.name_range),
        }
    }

    fn references<'a>(&'a self, name: &'a str) -> impl Iterator<Item = Range> + 'a {
        self.solution
            .iter()
            .filter(move |entry| entry.name == name)
            .map(|entry| entry.range)
    }

    pub(crate) fn occurrences_in_document_order(&self, name: &str) -> Vec<Range> {
        let mut ranges: Vec<Range> = self
            .op(name)
            .map(|op| op.name_range)
            .into_iter()
            .chain(self.references(name))
            .collect();
        ranges.sort_by_key(|r| (r.start.line, r.start.character));
        ranges
    }

    pub(crate) fn symbol_occurrences(
        &self,
        symbol: &Symbol,
        include_declaration: bool,
    ) -> Vec<Range> {
        match symbol {
            Symbol::Puzzle { .. } => include_declaration
                .then_some(self.name_range)
                .into_iter()
                .collect(),
            Symbol::Op { name } => self
                .declaration_range(symbol)
                .filter(|_| include_declaration)
                .into_iter()
                .chain(self.references(name))
                .collect(),
        }
    }

    pub(crate) fn rename_collides(&self, symbol: &Symbol, new_name: &str) -> bool {
        let Symbol::Op { name } = symbol else {
            return false;
        };
        self.op(name).is_some() && name.as_str() != new_name && self.op(new_name).is_some()
    }

    pub(crate) fn rename_ranges(&self, symbol: &Symbol, is_position_doc: bool) -> Vec<Range> {
        match symbol {
            Symbol::Puzzle { name } => {
                if is_position_doc && self.name == *name {
                    vec![self.name_range]
                } else {
                    Vec::new()
                }
            }
            Symbol::Op { name } => self.occurrences_in_document_order(name),
        }
    }
}

pub(crate) fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
