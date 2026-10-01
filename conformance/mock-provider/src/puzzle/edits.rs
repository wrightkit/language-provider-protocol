//! Normative edit application rules for `lpp/validateEdits` (spec 16.3):
//! bounds checks, overlap detection, application, and re-parsing.

use super::parse::parse_document;
use super::text::{Range, SourceText};

#[derive(Debug)]
pub(crate) enum EditValidation {
    Valid,
    Invalid {
        reason: &'static str,
        failing_edit_index: Option<usize>,
    },
}

impl EditValidation {
    fn invalid(reason: &'static str, failing_edit_index: Option<usize>) -> Self {
        Self::Invalid {
            reason,
            failing_edit_index,
        }
    }
}

pub(crate) fn validate_edits(text: &str, edits: &[(Range, String)]) -> EditValidation {
    let src = SourceText::new(text);

    // Bounds and ordering of each edit range.
    let mut resolved = Vec::with_capacity(edits.len());
    for (i, (range, _)) in edits.iter().enumerate() {
        match (src.byte_of(range.start), src.byte_of(range.end)) {
            (Some(start), Some(end)) if start <= end => resolved.push((start, end)),
            _ => return EditValidation::invalid("rangeOutOfBounds", Some(i)),
        }
    }

    // Overlap detection in document order; the failing index refers to the
    // original request order.
    let mut order: Vec<usize> = (0..edits.len()).collect();
    order.sort_by_key(|&i| resolved[i].0);
    for pair in order.windows(2) {
        if resolved[pair[0]].1 > resolved[pair[1]].0 {
            return EditValidation::invalid("overlappingEdits", Some(pair[1]));
        }
    }

    // Apply in document order.
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    for &i in &order {
        result.push_str(&text[cursor..resolved[i].0]);
        result.push_str(&edits[i].1);
        cursor = resolved[i].1;
    }
    result.push_str(&text[cursor..]);

    if parse_document(&result).has_errors() {
        return EditValidation::invalid("syntaxError", None);
    }
    EditValidation::Valid
}
