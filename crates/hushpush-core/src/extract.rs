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
    let before = &text[..span.start];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let col_start = text[line_start..span.start].chars().count() + 1;
    let col_end = col_start + text[span].chars().count();
    Location::Text {
        line,
        col_start,
        col_end,
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
}
