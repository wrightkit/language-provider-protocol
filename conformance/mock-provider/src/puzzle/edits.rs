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

struct ResolvedEdit {
    request_index: usize,
    start: usize,
    end: usize,
}

pub(crate) fn validate_edits(text: &str, edits: &[(Range, String)]) -> EditValidation {
    let src = SourceText::new(text);

    let mut resolved = Vec::with_capacity(edits.len());
    for (request_index, (range, _)) in edits.iter().enumerate() {
        match (src.byte_of(range.start), src.byte_of(range.end)) {
            (Some(start), Some(end)) if start <= end => resolved.push(ResolvedEdit {
                request_index,
                start,
                end,
            }),
            _ => return EditValidation::invalid("rangeOutOfBounds", Some(request_index)),
        }
    }

    resolved.sort_by_key(|edit| edit.start);
    for pair in resolved.windows(2) {
        if pair[0].end > pair[1].start {
            return EditValidation::invalid("overlappingEdits", Some(pair[1].request_index));
        }
    }

    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    for edit in &resolved {
        result.push_str(&text[cursor..edit.start]);
        result.push_str(&edits[edit.request_index].1);
        cursor = edit.end;
    }
    result.push_str(&text[cursor..]);

    if parse_document(&result).has_errors() {
        return EditValidation::invalid("syntaxError", None);
    }
    EditValidation::Valid
}
