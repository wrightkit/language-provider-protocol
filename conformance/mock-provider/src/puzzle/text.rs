use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub(crate) const ORIGIN: Self = Position {
        line: 0,
        character: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub(crate) fn point(pos: Position) -> Self {
        Range {
            start: pos,
            end: pos,
        }
    }
}

pub(crate) struct SourceText<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourceText<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { text, line_starts }
    }

    fn line_byte_range(&self, line: usize) -> (usize, usize) {
        let start = self.line_starts[line];
        let mut end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());
        if end > start && self.text.as_bytes()[end - 1] == b'\n' {
            end -= 1;
            if end > start && self.text.as_bytes()[end - 1] == b'\r' {
                end -= 1;
            }
        }
        (start, end)
    }

    pub(crate) fn line_text(&self, line: usize) -> &'a str {
        let (start, end) = self.line_byte_range(line);
        &self.text[start..end]
    }

    pub(crate) fn byte_of(&self, pos: Position) -> Option<usize> {
        let start = *self.line_starts.get(pos.line as usize)?;
        let line_text = self.line_text(pos.line as usize);
        let mut units = 0u32;
        let mut byte = line_text.len();
        for (i, ch) in line_text.char_indices() {
            if units == pos.character {
                byte = i;
                break;
            }
            if units + ch.len_utf16() as u32 > pos.character {
                return None;
            }
            units += ch.len_utf16() as u32;
        }
        (units == pos.character).then(|| start + byte)
    }

    pub(crate) fn position_of(&self, byte: usize) -> Position {
        let byte = byte.min(self.text.len());
        let line = match self.line_starts.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let (start, end) = self.line_byte_range(line);
        let units = self.text[start..byte.min(end)]
            .chars()
            .map(char::len_utf16)
            .sum::<usize>();
        Position {
            line: line as u32,
            character: units as u32,
        }
    }

    pub(crate) fn range_of(&self, start: usize, end: usize) -> Range {
        Range {
            start: self.position_of(start),
            end: self.position_of(end),
        }
    }

    pub(crate) fn contains_byte(&self, range: Range, byte: usize) -> bool {
        match (self.byte_of(range.start), self.byte_of(range.end)) {
            (Some(start), Some(end)) => start <= byte && byte < end,
            _ => false,
        }
    }
}
