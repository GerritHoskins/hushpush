//! Detection of likely personal data.

use std::{ops::Range, sync::LazyLock};

use regex::Regex;
mod clipboard;
mod extra;
pub use clipboard::{ClipboardDetector, ClipboardFieldDetector};
pub use extra::{HomePathDetector, PemDetector, PhoneDetector};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Category {
    Email,
    PaymentCard,
    Phone,
    HomePath,
    PrivateKey,
    ClipboardRecord,
    UserPattern,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Block,
    Warn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Match {
    pub span: Range<usize>,
    pub confidence: Confidence,
}

pub trait Detector {
    fn id(&self) -> &str;
    fn version(&self) -> u32;
    fn category(&self) -> Category;
    fn severity(&self) -> Severity;
    fn find(&self, text: &str) -> Vec<Match>;
}

#[derive(Default)]
pub struct EmailDetector {
    pub include_reserved_domains: bool,
}

impl EmailDetector {
    pub fn new(include_reserved_domains: bool) -> Self {
        Self {
            include_reserved_domains,
        }
    }
}

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)[a-z0-9_%+-]+",
        r"(?:\.[a-z0-9_%+-]+)*@",
        r"(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}"
    ))
    .expect("static email regex")
});

fn reserved_domain(domain: &str) -> bool {
    let domain = domain.to_ascii_lowercase();
    [
        "example.com",
        "example.net",
        "example.org",
        "example",
        "test",
        "invalid",
        "localhost",
    ]
    .iter()
    .any(|reserved| domain == *reserved || domain.ends_with(&format!(".{reserved}")))
}

impl Detector for EmailDetector {
    fn id(&self) -> &str {
        "email"
    }
    fn version(&self) -> u32 {
        3
    }
    fn category(&self) -> Category {
        Category::Email
    }
    fn severity(&self) -> Severity {
        Severity::Warn
    }

    fn find(&self, text: &str) -> Vec<Match> {
        EMAIL
            .find_iter(text)
            .filter(|found| {
                let before = text[..found.start()].chars().next_back();
                let after = text[found.end()..].chars().next();
                // Reject partial addresses rather than salvaging a valid-looking suffix.
                let invalid_before =
                    before.is_some_and(|c| c.is_alphanumeric() || "._%+-".contains(c));
                let invalid_after = after.is_some_and(|c| c.is_alphanumeric() || "@-_".contains(c))
                    || text[found.end()..]
                        .strip_prefix("..")
                        .is_some_and(|tail| tail.chars().next().is_some_and(char::is_alphanumeric));
                !invalid_before
                    && !invalid_after
                    && (self.include_reserved_domains
                        || !reserved_domain(found.as_str().split_once('@').unwrap().1))
            })
            .map(|found| Match {
                span: found.range(),
                confidence: Confidence::High,
            })
            .collect()
    }
}

#[derive(Default)]
pub struct CardDetector;

static CARD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9]+(?:[ -][0-9]+)*").expect("static card regex"));

fn known_iin(digits: &[u32]) -> bool {
    let first_two = digits[0] * 10 + digits[1];
    let first_three = first_two * 10 + digits[2];
    let first_four = first_three * 10 + digits[3];
    digits[0] == 4
        || (51..=55).contains(&first_two)
        || (2221..=2720).contains(&first_four)
        || matches!(first_two, 34 | 37 | 65)
        || first_four == 6011
        || (644..=649).contains(&first_three)
}

fn luhn(digits: &[u32]) -> bool {
    digits
        .iter()
        .rev()
        .enumerate()
        .map(|(index, digit)| {
            if index % 2 == 0 {
                *digit
            } else {
                let doubled = digit * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            }
        })
        .sum::<u32>()
        .is_multiple_of(10)
}

impl Detector for CardDetector {
    fn id(&self) -> &str {
        "card"
    }
    fn version(&self) -> u32 {
        2
    }
    fn category(&self) -> Category {
        Category::PaymentCard
    }
    fn severity(&self) -> Severity {
        Severity::Block
    }

    fn find(&self, text: &str) -> Vec<Match> {
        let mut matches = Vec::new();
        for run in CARD.find_iter(text) {
            let mut offset = run.start();
            let groups: Vec<_> = run
                .as_str()
                .split([' ', '-'])
                .map(|group| {
                    let span = offset..offset + group.len();
                    offset = span.end + 1;
                    span
                })
                .collect();
            let mut start = 0;
            while start < groups.len() {
                let begin = groups[start].start;
                if text[..begin]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric)
                {
                    start += 1;
                    continue;
                }
                let mut digits = Vec::with_capacity(19);
                let mut separator = None;
                let mut best = None;
                for (end, group) in groups.iter().enumerate().skip(start) {
                    if digits.len() + group.len() > 19 {
                        break;
                    }
                    if end > start {
                        let current = text.as_bytes()[group.start - 1];
                        if separator.is_some_and(|previous| previous != current) {
                            break;
                        }
                        separator = Some(current);
                    }
                    digits.extend(
                        text[group.clone()]
                            .bytes()
                            .map(|byte| u32::from(byte - b'0')),
                    );
                    if digits.len() < 13
                        || text[group.end..]
                            .chars()
                            .next()
                            .is_some_and(char::is_alphanumeric)
                        || !known_iin(&digits)
                        || !luhn(&digits)
                    {
                        continue;
                    }
                    best = Some((
                        end,
                        Match {
                            span: begin..group.end,
                            confidence: if separator.is_some() || digits.len() == 16 {
                                Confidence::High
                            } else {
                                Confidence::Medium
                            },
                        },
                    ));
                }
                // Keep the longest valid window at the earliest available start.
                if let Some((end, found)) = best {
                    matches.push(found);
                    start = end + 1;
                } else {
                    start += 1;
                }
            }
        }
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REVIEW_LOG: &str = concat!(
        "L1 2026-10-27 4242424242424242\n",
        "L2 qty 3 4242 4242 4242 4242\n",
        "L3 card 4242 4242 4242 4242 exp 12 28\n",
        "L4 email=jane.doe@corp.example.com\n",
        "L5 4242 4242 4242 4242\n",
    );

    #[test]
    fn review_log_has_exact_spans() {
        assert_eq!(CardDetector.version(), 2);
        assert_eq!(EmailDetector::default().version(), 3);
        assert_eq!(
            CardDetector.find(REVIEW_LOG),
            [14..30, 40..59, 68..87, 136..155].map(|span| Match {
                span,
                confidence: Confidence::High
            })
        );
        assert_eq!(
            EmailDetector::new(true).find(REVIEW_LOG),
            vec![Match {
                span: 107..132,
                confidence: Confidence::High
            }]
        );
    }

    #[test]
    fn email_practical_local_part_and_prefix_boundaries() {
        for prefix in ["email=", "email!", "email/", "email:", "é="] {
            let text = format!("{prefix}jane.doe@corp.example.com");
            assert_eq!(
                EmailDetector::new(true).find(&text),
                vec![Match {
                    span: prefix.len()..prefix.len() + 25,
                    confidence: Confidence::High
                }]
            );
        }
        assert_eq!(
            EmailDetector::new(true)
                .find("a_b%tag+more-name@corp.example.com")
                .len(),
            1
        );
        for prefix in [".", "..", "é"] {
            let text = format!("{prefix}jane.doe@corp.example.com");
            assert!(EmailDetector::new(true).find(&text).is_empty());
        }
    }

    #[test]
    fn card_windows_are_greedy_and_non_overlapping() {
        for (text, spans) in [
            ("4242424242424242 0 0 0", vec![(0, 20)]),
            (
                "4242 4242 4242 4242 4242 4242 4242 4242",
                vec![(0, 19), (20, 39)],
            ),
            ("a1 4242 4242 4242 4242 2z", vec![(3, 22)]),
            ("1-4242 4242 4242 4242-2", vec![(2, 21)]),
            ("é 4242-4242-4242-4242!", vec![(3, 22)]),
        ] {
            assert_eq!(
                CardDetector.find(text),
                spans
                    .into_iter()
                    .map(|(start, end)| Match {
                        span: start..end,
                        confidence: Confidence::High
                    })
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn email_spans_and_reserved_domains() {
        let text = "é jane.doe@corp.example.com";
        let matches = EmailDetector::new(true).find(text);
        assert_eq!(
            matches,
            vec![Match {
                span: 3..text.len(),
                confidence: Confidence::High
            }]
        );
        for domain in [
            "example.com",
            "corp.example.com",
            "example.net",
            "example.org",
            "EXAMPLE.ORG",
            "a.example",
            "a.test",
            "a.invalid",
            "a.localhost",
        ] {
            assert!(
                EmailDetector::default()
                    .find(&format!("fake@{domain}"))
                    .is_empty()
            );
        }
        assert_eq!(
            EmailDetector::default()
                .find("fake@synthetic.company")
                .len(),
            1
        );
    }

    #[test]
    fn email_followed_by_ellipsis() {
        for text in ["jane@corp.com..", "jane@corp.com..."] {
            assert_eq!(
                EmailDetector::default().find(text),
                vec![Match {
                    span: 0..13,
                    confidence: Confidence::High
                }]
            );
        }
        for text in ["jane@corp.com..org", "jane@corp.com..123"] {
            assert!(EmailDetector::default().find(text).is_empty());
        }
    }

    #[test]
    fn email_rejects_malformed_addresses() {
        for text in [
            "user@localhost",
            ".user@corp.example.com",
            "user..name@corp.example.com",
            "user.@corp.example.com",
            "user@-corp.example.com",
            "user@corp..example.com",
            "user@corp.c",
            "user@corp.com123",
        ] {
            assert!(EmailDetector::new(true).find(text).is_empty());
        }
    }

    #[test]
    fn cards_have_byte_spans_and_confidence() {
        for (value, confidence) in [
            ("4242 4242 4242 4242", Confidence::High),
            ("5555-5555-5555-4444", Confidence::High),
            ("378282246310005", Confidence::Medium),
            ("4242424242424242", Confidence::High),
            ("6011111111111117", Confidence::High),
            ("2223003122003222", Confidence::High),
        ] {
            let text = format!("é {value}!");
            assert_eq!(
                CardDetector.find(&text),
                vec![Match {
                    span: 3..3 + value.len(),
                    confidence
                }]
            );
        }
    }

    #[test]
    fn cards_reject_invalid_and_embedded_numbers() {
        for text in [
            "4242 4242 4242 4241",
            "5555-5555-5555-4445",
            "14242424242424242",
            "424242424242424212345",
            "a4242424242424242f",
            "é4242424242424242",
            "4242424242424242z",
            "v4.24.24.24",
            "2026-10-27 12:34:56",
            "1234567890128",
            "4242  4242 4242 4242",
            "order 1234 5678 9012 3456",
            "ID 1234 5678 9012 3456 7890",
            "+1 555 0100 4242",
            "v4242.4242.4242.4242",
            "4242-4242 4242-4242",
            "a4242 4242 4242 4242",
            "4242 4242 4242 4242z",
        ] {
            assert!(CardDetector.find(text).is_empty());
        }
    }

    #[test]
    fn card_iin_and_length_boundaries() {
        fn synthetic_card(prefix: &str, length: usize) -> String {
            let body = format!("{prefix}{}", "0".repeat(length - prefix.len() - 1));
            (0..=9)
                .map(|check| format!("{body}{check}"))
                .find(|candidate| {
                    luhn(
                        &candidate
                            .chars()
                            .map(|c| c.to_digit(10).unwrap())
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap()
        }
        for prefix in [
            "4", "51", "55", "2221", "2720", "34", "37", "6011", "644", "649", "65",
        ] {
            assert_eq!(CardDetector.find(&synthetic_card(prefix, 16)).len(), 1);
        }
        for prefix in [
            "50", "56", "2220", "2721", "33", "38", "6010", "6012", "643", "6500", "66",
        ] {
            // 6500 remains Discover because the two-digit 65 prefix is accepted.
            assert_eq!(
                CardDetector.find(&synthetic_card(prefix, 16)).len(),
                usize::from(prefix == "6500")
            );
        }
        for length in [12, 13, 19, 20] {
            assert_eq!(
                CardDetector.find(&synthetic_card("4", length)).len(),
                usize::from((13..=19).contains(&length))
            );
        }
    }
}
