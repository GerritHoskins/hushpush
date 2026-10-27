//! Extraction of text and locations.

use std::{borrow::Cow, ops::Range};

use crate::report::Location;

pub fn decode_text(bytes: &[u8]) -> (Cow<'_, str>, bool) {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => (Cow::Borrowed(text), false),
        Err(_) => (String::from_utf8_lossy(bytes), true),
    }
}

pub fn text_location(text: &str, span: Range<usize>) -> Location {
    TextLocations::new(text).location(span)
}

/// Index once per decoded file, including scalar-column corrections for long lines.
pub(crate) struct TextLocations {
    lines: Vec<usize>,
    corrections: Vec<(usize, usize)>,
}
impl TextLocations {
    pub(crate) fn new(text: &str) -> Self {
        let mut lines = vec![0];
        let mut corrections = Vec::new();
        let mut extra = 0;
        for (offset, ch) in text.char_indices() {
            if ch == '\n' {
                lines.push(offset + 1);
            }
            if ch.len_utf8() > 1 {
                extra += ch.len_utf8() - 1;
                corrections.push((offset + ch.len_utf8(), extra));
            }
        }
        Self { lines, corrections }
    }
    fn scalars_before(&self, offset: usize) -> usize {
        let index = self.corrections.partition_point(|(end, _)| *end <= offset);
        offset - index.checked_sub(1).map_or(0, |i| self.corrections[i].1)
    }
    pub(crate) fn location(&self, span: Range<usize>) -> Location {
        let line = self.lines.partition_point(|start| *start <= span.start);
        let start = self.scalars_before(span.start);
        Location::Text {
            line,
            col_start: start - self.scalars_before(self.lines[line - 1]) + 1,
            col_end: self.scalars_before(span.end) - self.scalars_before(self.lines[line - 1]) + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_lossy_decode_and_exclusive_unicode_columns() {
        let (text, partial) = decode_text(b"\xef\xbb\xbfstart\r\n\xff \xf0\x9f\x98\x80 marker");
        assert!(partial);
        assert!(!text.starts_with('\u{feff}'));
        let start = text.find("marker").unwrap();
        assert_eq!(
            text_location(&text, start..start + 6),
            Location::Text {
                line: 2,
                col_start: 5,
                col_end: 11
            }
        );
        assert!(!decode_text(b"\xef\xbb\xbfvalid").1);
    }
    #[test]
    fn indexed_locations_match_scalar_columns_across_lines() {
        let text = "???a\n?\nlast";
        let index = TextLocations::new(text);
        for (start, ch) in text.char_indices() {
            let before = &text[..start];
            let line_start = before.rfind('\n').map_or(0, |n| n + 1);
            let column = text[line_start..start].chars().count() + 1;
            assert_eq!(
                index.location(start..start + ch.len_utf8()),
                Location::Text {
                    line: before.bytes().filter(|b| *b == b'\n').count() + 1,
                    col_start: column,
                    col_end: column + 1,
                }
            );
        }
    }
}
