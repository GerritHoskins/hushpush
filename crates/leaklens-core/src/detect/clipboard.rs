//! Structural clipboard formats; free-form mentions are deliberately ignored.
use super::{Category, Confidence, Detector, Match, Severity};
use regex::Regex;
use std::{ops::Range, sync::LazyLock};

#[derive(Default)]
pub struct ClipboardDetector;

#[derive(Default)]
pub struct ClipboardFieldDetector;

static FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:\"(?:clipboard|pasteboard|clip_text|primaryClip|copiedText)\"|'(?:clipboard|pasteboard|clip_text|primaryClip|copiedText)'|\b(?:clipboard|pasteboard|clip_text|primaryClip|copiedText))[ \t]*[:=][ \t]*"#).unwrap()
});
static CLIP_DATA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\bClipData\s*\{\s*text/plain\s+"#).unwrap());
static SERVICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bClipboardService\s*:\s*text[ \t]*=[ \t]*").unwrap());
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*\[?(?:\d{4}-\d{2}-\d{2}[T ]|\d{2}-\d{2} )?\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?\]?").unwrap()
});
static LOG_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(?:\d+\s+){0,2}[VDIWEF]\s+[\w.-]+:|(?:TRACE|DEBUG|INFO|WARN|ERROR|FATAL)(?:\s+[\w.-]+:)?|\[(?:TRACE|DEBUG|INFO|WARN|ERROR|FATAL)\])$").unwrap()
});

fn log_prefix(prefix: &str) -> bool {
    let remainder = TIMESTAMP
        .find(prefix)
        .map_or(prefix, |stamp| &prefix[stamp.end()..])
        .trim();
    remainder.is_empty() || LOG_PREFIX.is_match(remainder)
}

static UI_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:clipboard|pasteboard|copied|copy|paste|cut)\b").unwrap()
});

static HISTORY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:clipboard|copied|pasteboard)\b").unwrap());

fn meaningful(value: &str) -> bool {
    let value = value.trim();
    let scalar = value.trim_end_matches(['}', ']', ',', ';']).trim_end();
    value.chars().count() >= 4
        && !matches!(
            scalar.to_ascii_lowercase().as_str(),
            "true" | "false" | "null"
        )
        && scalar.parse::<f64>().is_err()
        && !value.starts_with("[REDACTED:")
        && !UI_VALUE.is_match(value)
        && !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:/-".contains(&b))
}

fn value_span(text: &str, start: usize, line_end: usize) -> Option<Range<usize>> {
    let first = *text.as_bytes().get(start)?;
    let span = if matches!(first, b'\'' | b'"') {
        let mut escaped = false;
        let mut end = None;
        for (offset, byte) in text.as_bytes()[start + 1..].iter().enumerate() {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == first {
                end = Some(start + 1 + offset);
                break;
            }
        }
        start + 1..end?
    } else {
        let value = &text[start..line_end];
        let end = [value.find(" #"), value.find(" //")]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(value.len());
        let value = value[..end].trim_end();
        start..start + value.len()
    };
    let value = &text[span.clone()];
    meaningful(value).then_some(span)
}

impl Detector for ClipboardDetector {
    fn id(&self) -> &str {
        "clipboard"
    }
    fn version(&self) -> u32 {
        2
    }
    fn category(&self) -> Category {
        Category::ClipboardRecord
    }
    fn severity(&self) -> Severity {
        Severity::Block
    }

    fn find(&self, text: &str) -> Vec<Match> {
        let mut spans = Vec::new();
        let mut lines = Vec::new();
        let mut offset = 0;
        for raw in text.split_inclusive('\n') {
            let line = raw.trim_end_matches(['\r', '\n']);
            lines.push((offset, line));
            offset += raw.len();
        }
        for (index, &(offset, line)) in lines.iter().enumerate() {
            let end = offset + line.len();
            if TIMESTAMP.is_match(line) && HISTORY.is_match(line) {
                let mut record: Option<Range<usize>> = None;
                for &(next_offset, next) in &lines[index + 1..] {
                    if next.trim().is_empty() || TIMESTAMP.is_match(next) {
                        break;
                    }
                    let content = next.trim_start_matches([' ', '\t']);
                    let content = content.strip_prefix("> ").unwrap_or(content);
                    if next == content {
                        break;
                    }
                    let begin = next_offset + next.len() - content.len();
                    match &mut record {
                        Some(span) => span.end = next_offset + next.len(),
                        None => record = Some(begin..next_offset + next.len()),
                    }
                }
                if let Some(span) = record.filter(|span| meaningful(&text[span.clone()])) {
                    spans.push(span);
                }
            }
            if !line.contains("ClipboardService:") {
                continue;
            }
            for found in CLIP_DATA.find_iter(line) {
                let start = offset + found.end();
                if text
                    .as_bytes()
                    .get(start)
                    .is_some_and(|ch| matches!(ch, b'\'' | b'"'))
                    && let Some(span) = value_span(text, start, end)
                {
                    spans.push(span);
                }
            }
            for found in SERVICE.find_iter(line) {
                let start = offset + found.end();
                if text
                    .as_bytes()
                    .get(start)
                    .is_some_and(|b| matches!(b, b'\'' | b'"'))
                    && let Some(span) = value_span(text, start, end)
                {
                    spans.push(span);
                }
            }
        }
        spans.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
        let mut result: Vec<Match> = Vec::new();
        for span in spans {
            if result
                .last()
                .is_some_and(|previous| previous.span.end > span.start)
            {
                continue;
            }
            result.push(Match {
                span,
                confidence: Confidence::High,
            });
        }
        result
    }
}

// Field syntax cannot distinguish private notes from UI strings.
impl Detector for ClipboardFieldDetector {
    fn id(&self) -> &str {
        "clipboard_field"
    }
    fn version(&self) -> u32 {
        1
    }
    fn category(&self) -> Category {
        Category::ClipboardRecord
    }
    fn severity(&self) -> Severity {
        Severity::Warn
    }
    fn find(&self, text: &str) -> Vec<Match> {
        let mut output = Vec::new();
        let mut offset = 0;
        for raw in text.split_inclusive('\n') {
            let line = raw.trim_end_matches(['\r', '\n']);
            for field in FIELD.find_iter(line) {
                if line[..field.start()]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '.' | '-' | '$'))
                {
                    continue;
                }
                let start = offset + field.end();
                let quoted = line
                    .as_bytes()
                    .get(field.end())
                    .is_some_and(|b| matches!(b, b'"' | b'\''));
                if !quoted && !log_prefix(line[..field.start()].trim()) {
                    continue;
                }
                let Some(span) = value_span(text, start, offset + line.len()) else {
                    continue;
                };
                let value = &text[span.clone()];
                if !quoted
                    && (value.split_whitespace().count() < 3
                        || value.contains(['(', ')', ';', '{', '}', '[', ']', '/', '\\']))
                {
                    continue;
                }
                output.push(Match {
                    span,
                    confidence: Confidence::High,
                });
            }
            offset += raw.len();
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_have_exact_byte_spans_in_every_format() {
        for (prefix, value, suffix) in [
            (
                "é ClipboardService: ClipData { text/plain \"",
                "synthetic payload",
                "\" }",
            ),
            ("ClipboardService: text=\"", "synthetic payload", "\""),
            ("{\"primaryClip\":\"", "synthetic\nmultiline payload", "\"}"),
            ("pasteboard: '", "synthetic \\' quoted payload", "'"),
            (
                "2026-10-27 12:34:56 clipboard record\r\n  ",
                "first synthetic line\r\n  second synthetic line",
                "\r\n\r\nnext",
            ),
            (
                "[12:34:56] copied\n> ",
                "first synthetic line\n> second synthetic line",
                "\n12:35:00 other\n  unrelated",
            ),
        ] {
            let text = format!("{prefix}{value}{suffix}");
            assert_eq!(
                if prefix.contains("primaryClip") || prefix.starts_with("pasteboard") {
                    ClipboardFieldDetector.find(&text)
                } else {
                    ClipboardDetector.find(&text)
                },
                vec![Match {
                    span: prefix.len()..prefix.len() + value.len(),
                    confidence: Confidence::High
                }]
            );
        }
    }

    #[test]
    fn lookalikes_and_scalar_config_are_ignored() {
        for text in [
            "navigator.clipboard.writeText(x)",
            "clipboard.read()",
            "class ClipboardManager {}",
            "copy to clipboard",
            "clipboard_enabled=true",
            "{\"clipboard\": false}",
            "clipboard=123456",
            "clipboard=-12.35",
            "clipboard=",
            "clipboard='abc'",
            "clipboard=true",
            "clipboard=null",
            "clipboard=\"\"",
            "xclipboard=value",
            "my.clipboard=value",
            "clipboard='unterminated",
            "12:34:56 clipboard history\nnot indented",
        ] {
            assert!(ClipboardDetector.find(text).is_empty());
        }
    }

    #[test]
    fn loose_fields_have_separate_policy_and_reject_code() {
        let value = "Remember to call the pharmacy about the refill";
        for prefix in [
            "clipboard: ",
            "  pasteboard = ",
            "2026-10-27 09:15:10 INFO clipboard: ",
            "INFO clipboard: ",
            "10-27 12:34:56.000 1234 1250 I App: clipboard: ",
        ] {
            let text = format!("{prefix}{value}");
            assert!(ClipboardDetector.find(&text).is_empty());
            let found = ClipboardFieldDetector.find(&text);
            assert_eq!(found.len(), 1);
            assert!(found[0].span == (prefix.len()..text.len()));
        }
        for text in [
            "set clipboard=unnamedplus",
            "Clipboard: see docs/clipboard.md",
            "clipboard: enabled",
            "clipboard: see local docs/readme.md",
            "clipboard: https://example.test/a b c",
            "clipboard: invoke (a b c)",
            "clipboard: [one two three]",
            "xclipboard: one two three",
            "copyToClipboard: one two three",
            "let clipboard = one two three;",
            "2026-10-27 09:15:10 const clipboard: one two three",
            "2026-10-27 09:15:10 obj.clipboard: one two three",
        ] {
            assert!(ClipboardFieldDetector.find(text).is_empty());
        }
        assert_eq!(ClipboardFieldDetector.severity(), Severity::Warn);
    }

    #[test]
    fn field_exclusions_and_comment_spans() {
        for value in [
            "Copied to clipboard!",
            "Paste here",
            "please COPY this",
            "Cut selected text",
            "pasteboard enabled",
            "com.apple.UIKit.pboard.general",
            "namespace:very/long-identifier.with_more.components",
            "true # enable integration",
            "false // enable integration",
            "123.45 # numeric setting",
            "enabled // setting",
            "Copies the current selection to the system clipboard",
        ] {
            let text = format!("clipboard: {value}");
            assert!(ClipboardFieldDetector.find(&text).is_empty());
            let text = format!("{{\"clipboard\":\"{value}\"}}");
            if !value.contains(" #") && !value.contains(" //") {
                assert!(ClipboardFieldDetector.find(&text).is_empty());
            }
        }
        for suffix in [" # enable integration", " // enable integration", ""] {
            let text = format!("clipboard: synthetic private note{suffix}");
            let found = ClipboardFieldDetector.find(&text);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].span, 11..33);
        }
        for word in ["copycat", "pasted", "shortcut", "uncut", "clipboardish"] {
            let text = format!("clipboard: a synthetic {word} note");
            assert_eq!(ClipboardFieldDetector.find(&text).len(), 1);
        }
    }

    #[test]
    fn multiple_fields_and_escaped_quotes() {
        let text = r#"{"clipboard":"synthetic \"quote\"", "copiedText":"another value"}"#;
        assert_eq!(ClipboardFieldDetector.find(text).len(), 2);
        assert!(ClipboardDetector.find(text).is_empty());
        assert_eq!(ClipboardDetector.version(), 2);
        assert_eq!(ClipboardDetector.severity(), Severity::Block);
    }
}
