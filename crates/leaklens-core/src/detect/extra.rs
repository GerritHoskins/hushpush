//! Structural detectors for telephone numbers, home directories, and private keys.
use std::sync::LazyLock;

use regex::Regex;

use super::{CardDetector, Category, Confidence, Detector, Match, Severity};

#[derive(Default)]
pub struct PhoneDetector {
    pub include_reserved: bool,
}

impl PhoneDetector {
    pub fn new(include_reserved: bool) -> Self {
        Self { include_reserved }
    }
}

static PHONE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\+1[ .-]?(?:\([2-9][0-9]{2}\)[ ]?|[2-9][0-9]{2}[-. ])[2-9][0-9]{2}[-. ][0-9]{4}",
        r"|\+1[ ]555-01[0-9]{2}-[0-9]{4}",
        r"|\+[0-9](?:[0-9 .()\-]*[0-9])?",
        r"|\([2-9][0-9]{2}\)[ ]?[0-9]{3}[-. ][0-9]{4}",
        r"|[2-9][0-9]{2}[-. ][0-9]{3}[-. ][0-9]{4}"
    ))
    .expect("static phone regex")
});

static NON_PHONE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:[0-9]{4}[-. /][0-9]{1,2}[-. /][0-9]{1,2}",
        r"|[0-9]{1,2}[-. /][0-9]{1,2}[-. /][0-9]{4}",
        r"|[0-9]{1,3}(?:\.[0-9]{1,3}){3}",
        r"|[0-9]{1,2}:[0-9]{2}(?::[0-9]{2})?)$"
    ))
    .expect("static non-phone regex")
});

fn reserved_phone(digits: &str) -> bool {
    // Include the deliberately fictional S1 generator forms as well as NANP's
    // reserved seven-digit subscriber numbers, irrespective of the area code.
    let nanp = digits
        .strip_prefix('1')
        .filter(|_| digits.len() == 11)
        .unwrap_or(digits);
    (nanp.len() == 10 && nanp[3..].starts_with("55501"))
        || (digits.len() == 12 && digits.starts_with("155501"))
        || (digits.len() == 10 && digits.starts_with("55501"))
}

impl Detector for PhoneDetector {
    fn id(&self) -> &str {
        "phone"
    }
    fn version(&self) -> u32 {
        1
    }
    fn category(&self) -> Category {
        Category::Phone
    }
    fn severity(&self) -> Severity {
        Severity::Warn
    }
    fn find(&self, text: &str) -> Vec<Match> {
        let cards = CardDetector.find(text);
        PHONE
            .find_iter(text)
            .filter_map(|found| {
                let span = found.range();
                let before = text[..span.start].chars().next_back();
                let after = text[span.end..].chars().next();
                if before.is_some_and(char::is_alphanumeric)
                    || after.is_some_and(char::is_alphanumeric)
                    || before == Some('+')
                    || cards
                        .iter()
                        .any(|card| span.start < card.span.end && card.span.start < span.end)
                {
                    return None;
                }
                // Dotted version/IP tokens must not yield a valid-looking suffix.
                let dotted_suffix = before == Some('.')
                    && text[..span.start - 1]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_ascii_digit());
                let dotted_prefix = after == Some('.')
                    && text[span.end + 1..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit());
                if dotted_suffix || dotted_prefix || after == Some(':') {
                    return None;
                }
                let value = found.as_str();
                let digits: String = value.chars().filter(char::is_ascii_digit).collect();
                if !(8..=15).contains(&digits.len())
                    || NON_PHONE.is_match(value.trim_start_matches('+'))
                {
                    return None;
                }
                if !value.starts_with('+') {
                    let subscriber = &digits[3..];
                    if !subscriber.starts_with(|c: char| ('2'..='9').contains(&c))
                        && !digits.starts_with("55501")
                    {
                        return None;
                    }
                } else {
                    let body = &value[1..];
                    let first = body
                        .split(|c: char| !c.is_ascii_digit())
                        .next()
                        .unwrap_or("");
                    if first.len() > 3 && first.len() != digits.len() {
                        return None;
                    }
                    if value.matches('(').count() != value.matches(')').count() {
                        return None;
                    }
                    let mut open = false;
                    for character in body.chars() {
                        match character {
                            '(' if open => return None,
                            '(' => open = true,
                            ')' if !open => return None,
                            ')' => open = false,
                            _ => {}
                        }
                    }
                    let groups: Vec<_> = body.split('.').collect();
                    if groups.len() > 1
                        && groups.iter().all(|g| g.bytes().all(|b| b.is_ascii_digit()))
                        && groups.iter().filter(|g| g.len() >= 3).count() < 2
                    {
                        return None;
                    }
                }
                if !self.include_reserved && reserved_phone(&digits) {
                    return None;
                }
                Some(Match {
                    span,
                    confidence: Confidence::Medium,
                })
            })
            .collect()
    }
}

#[derive(Default)]
pub struct HomePathDetector;

static HOME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:/(?:Users|home)/|[a-z]:[/\\]+Users[/\\]+)([^/\\\r\n"']+)[/\\]"#)
        .expect("static home path regex")
});

impl Detector for HomePathDetector {
    fn id(&self) -> &str {
        "home_path"
    }
    fn version(&self) -> u32 {
        1
    }
    fn category(&self) -> Category {
        Category::HomePath
    }
    fn severity(&self) -> Severity {
        Severity::Warn
    }
    fn find(&self, text: &str) -> Vec<Match> {
        HOME.captures_iter(text)
            .filter_map(|captures| {
                let whole = captures.get(0).unwrap();
                if text[..whole.start()]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric)
                {
                    return None;
                }
                let name = captures.get(1).unwrap();
                if name.as_str().starts_with("[REDACTED:HOME_USER]")
                    || matches!(
                        name.as_str().to_lowercase().as_str(),
                        "public"
                            | "default"
                            | "default user"
                            | "all users"
                            | "runner"
                            | "user"
                            | "username"
                            | "you"
                            | "me"
                            | "$user"
                            | "%username%"
                            | "${user}"
                            | "<user>"
                            | "{user}"
                            | "*"
                            | "..."
                    )
                {
                    return None;
                }
                Some(Match {
                    span: name.range(),
                    confidence: Confidence::High,
                })
            })
            .collect()
    }
}

#[derive(Default)]
pub struct PemDetector;

static PEM_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"-----BEGIN[ \t]+((?:(?:RSA|EC|DSA|OPENSSH|ENCRYPTED)[ \t]+)?PRIVATE[ \t]+KEY)[ \t]*-----",
    )
    .expect("static PEM header regex")
});
static PEM_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"-----END[ \t]+((?:(?:RSA|EC|DSA|OPENSSH|ENCRYPTED)[ \t]+)?PRIVATE[ \t]+KEY)[ \t]*-----",
    )
    .expect("static PEM end regex")
});

impl Detector for PemDetector {
    fn id(&self) -> &str {
        "pem"
    }
    fn version(&self) -> u32 {
        1
    }
    fn category(&self) -> Category {
        Category::PrivateKey
    }
    fn severity(&self) -> Severity {
        Severity::Block
    }
    fn find(&self, text: &str) -> Vec<Match> {
        let mut output = Vec::new();
        for header in PEM_HEADER.captures_iter(text) {
            let whole = header.get(0).unwrap();
            if output
                .last()
                .is_some_and(|m: &Match| m.span.end > whole.start())
                || text[..whole.start()]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric)
            {
                continue;
            }
            let kind: String = header[1].split_whitespace().collect();
            let tail = &text[whole.end()..];
            let next_header = PEM_HEADER.find(tail).map_or(tail.len(), |m| m.start());
            let matching_end = PEM_END
                .captures_iter(&tail[..next_header])
                .find(|end| end[1].split_whitespace().collect::<String>() == kind);
            let end = if let Some(end) = matching_end {
                whole.end() + end.get(0).unwrap().end()
            } else {
                let mut end = whole.end();
                let mut offset = whole.end();
                let mut metadata = false;
                let mut body = false;
                for (index, line) in tail.split_inclusive('\n').enumerate() {
                    let content = line.trim_end_matches(['\r', '\n']);
                    if index == 0 && content.is_empty() {
                        offset += line.len();
                        continue;
                    }
                    if !body
                        && (content.starts_with("Proc-Type:") || content.starts_with("DEK-Info:"))
                    {
                        metadata = true;
                    } else if !body && metadata && content.is_empty() {
                        metadata = false;
                    } else if (content.len() >= 16
                        || (content.len() >= 4 && content.ends_with('=')))
                        && content
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
                    {
                        body = true;
                    } else {
                        break;
                    }
                    end = offset + content.len();
                    offset += line.len();
                }
                end
            };
            output.push(Match {
                span: whole.start()..end,
                confidence: Confidence::High,
            });
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(detector: &dyn Detector, text: &str) -> Vec<std::ops::Range<usize>> {
        detector.find(text).into_iter().map(|m| m.span).collect()
    }

    #[test]
    fn phone_formats_exact_byte_spans() {
        for value in [
            "+49 30 2345 6789",
            "+442079460958",
            "+1 (212) 234-5678",
            "(212) 234-5678",
            "212-234-5678",
            "212.234.5678",
            "+1 555-0123-4567",
            "(555) 012-3456",
        ] {
            let text = format!("é {value}!");
            assert_eq!(
                spans(&PhoneDetector::new(true), &text),
                vec![3..3 + value.len()]
            );
        }
        assert_eq!(PhoneDetector::default().version(), 1);
        assert_eq!(PhoneDetector::default().severity(), Severity::Warn);
    }

    #[test]
    fn phone_boundaries_nonphones_and_card_precedence() {
        for value in [
            "1234567890",
            "a212-234-5678",
            "212-234-5678z",
            "é212-234-5678",
            "2026-10-27",
            "27-10-2026",
            "12:34:56",
            "192.168.100.123",
            "v212.234.5678",
            "1.212.234.5678",
            "4242 4242 4242 4242",
            "+4242 4242 4242 4242",
            "+1234567",
            "+1234567890123456",
            "(212) 134-5678",
            "212-034-5678",
            "+49 (30 23456789",
            "+1.2.3.4567",
            "+26-10-2026",
            "+2026-10-27",
            "+49 )30( 23456789",
            "+49 ((30)) 23456789",
        ] {
            assert!(PhoneDetector::new(true).find(value).is_empty());
        }
    }

    #[test]
    fn phone_sentence_punctuation_and_adjacent_records() {
        assert_eq!(
            spans(&PhoneDetector::new(true), "212-234-5678."),
            vec![0..12]
        );
        assert_eq!(
            spans(&PhoneDetector::new(true), "+1 212-234-5678 +1 212-345-6789"),
            vec![0..15, 16..31]
        );
        assert_eq!(
            spans(&PhoneDetector::new(true), "+1 212-234-5678 42"),
            vec![0..15]
        );
    }

    #[test]
    fn phone_and_home_redaction_bytes_leave_the_surrounding_text() {
        for (detector, text, expected, replacement) in [
            (
                &PhoneDetector::new(true) as &dyn Detector,
                "before\r\n212-234-5678\r\nafter",
                "before\r\n[REDACTED:PHONE]\r\nafter",
                "[REDACTED:PHONE]",
            ),
            (
                &HomePathDetector as &dyn Detector,
                "before\r\n/Users/Synthetic.Person/file\r\nafter",
                "before\r\n/Users/[REDACTED:HOME_USER]/file\r\nafter",
                "[REDACTED:HOME_USER]",
            ),
        ] {
            let found = detector.find(text);
            assert_eq!(found.len(), 1);
            let mut output = text.as_bytes().to_vec();
            output.splice(found[0].span.clone(), replacement.bytes());
            assert_eq!(output, expected.as_bytes());
        }
    }

    #[test]
    fn reserved_phones_are_opt_in() {
        for value in [
            "212-555-0100",
            "212-555-0199",
            "+1 212 555 0123",
            "+1 555-0199-1234",
            "(555) 019-1234",
        ] {
            assert!(PhoneDetector::default().find(value).is_empty());
            assert_eq!(PhoneDetector::new(true).find(value).len(), 1);
        }
        for value in ["212-555-0099", "212-555-0200"] {
            assert_eq!(PhoneDetector::default().find(value).len(), 1);
        }
    }

    #[test]
    fn home_paths_match_name_only() {
        for value in [
            "/Users/Synthetic.Person/notes",
            "/home/Synthetic.Person/notes",
            "C:\\Users\\Synthetic.Person\\notes",
            "z:/users/Synthetic.Person/notes",
            "C:\\\\Users\\\\Synthetic.Person\\\\notes",
        ] {
            let start = value.find("Synthetic.Person").unwrap();
            assert_eq!(spans(&HomePathDetector, value), vec![start..start + 16]);
        }
        assert_eq!(spans(&HomePathDetector, "/home/Élodie/x"), vec![6..13]);
    }

    #[test]
    fn home_paths_ignore_placeholders_and_service_accounts() {
        for name in [
            "Public",
            "Default",
            "Default User",
            "All Users",
            "runner",
            "user",
            "username",
            "you",
            "me",
            "$USER",
            "%USERNAME%",
            "${USER}",
            "<user>",
            "{user}",
            "*",
            "...",
        ] {
            assert!(
                HomePathDetector
                    .find(&format!("/Users/{name}/x"))
                    .is_empty()
            );
        }
        for value in [
            "/home/",
            "/Users/Person",
            "xC:\\Users\\Person\\x",
            "https://host/home/Person/x",
        ] {
            assert!(HomePathDetector.find(value).is_empty());
        }
    }

    #[test]
    fn pem_all_private_types_and_noisy_spacing() {
        for kind in [
            "PRIVATE KEY",
            "RSA PRIVATE KEY",
            "EC PRIVATE KEY",
            "DSA PRIVATE KEY",
            "OPENSSH PRIVATE KEY",
            "ENCRYPTED PRIVATE KEY",
            "RSA  PRIVATE\tKEY",
        ] {
            let value = format!("-----BEGIN {kind}-----\r\nQUJDREVGRw==\r\n-----END {kind}-----");
            let text = format!("é\r\n{value}\r\nnext");
            assert_eq!(spans(&PemDetector, &text), vec![4..4 + value.len()]);
        }
    }

    #[test]
    fn pem_truncation_and_mismatched_end_preserve_surrounding_bytes() {
        let header = "-----BEGIN RSA PRIVATE KEY-----";
        let text = format!("before\r\n{header}\r\nQUJDRA==\r\nZWZnaA==\r\n!after\r\n");
        let found = PemDetector.find(&text);
        assert_eq!(found[0].span, 8..8 + header.len() + 2 + 8 + 2 + 8);
        let mut output = text.as_bytes().to_vec();
        output.splice(
            found[0].span.clone(),
            b"[REDACTED:PRIVATE_KEY]".iter().copied(),
        );
        assert_eq!(output, b"before\r\n[REDACTED:PRIVATE_KEY]\r\n!after\r\n");
        let mismatched = format!("{header}\nQUJDRA==\n-----END EC PRIVATE KEY-----");
        assert_eq!(spans(&PemDetector, &mismatched), vec![0..header.len() + 9]);
        assert_eq!(spans(&PemDetector, header), vec![0..header.len()]);
        for tail in ["after", "ordinary prose", "status is completed"] {
            let value = format!("{header}\nQUJDRA==\n{tail}\n");
            assert_eq!(spans(&PemDetector, &value), vec![0..header.len() + 9]);
        }
    }

    #[test]
    fn pem_public_material_is_excluded() {
        for kind in [
            "PUBLIC KEY",
            "CERTIFICATE",
            "CERTIFICATE REQUEST",
            "RSA PUBLIC KEY",
        ] {
            let value = format!("-----BEGIN {kind}-----\nQUJDRA==\n-----END {kind}-----");
            assert!(PemDetector.find(&value).is_empty());
        }
    }
}
