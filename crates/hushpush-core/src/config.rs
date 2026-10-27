//! Working-tree configuration, independent of the objects being scanned.
use crate::{
    detect::{Detector, Severity},
    report::{Finding, sanitize_path},
};
use chrono::NaiveDate;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, path::Path};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyAction {
    Block,
    Warn,
}
impl PolicyAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Warn => "warn",
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DetectorSeverity {
    Block,
    Warn,
    Off,
}
impl DetectorSeverity {
    pub fn severity(self) -> Option<Severity> {
        match self {
            Self::Block => Some(Severity::Block),
            Self::Warn => Some(Severity::Warn),
            Self::Off => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Warn => "warn",
            Self::Off => "off",
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub on_incomplete: PolicyAction,
    pub on_unsupported: PolicyAction,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            on_incomplete: PolicyAction::Block,
            on_unsupported: PolicyAction::Warn,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Email {
    pub severity: DetectorSeverity,
    pub include_reserved_domains: bool,
}
impl Default for Email {
    fn default() -> Self {
        Self {
            severity: DetectorSeverity::Warn,
            include_reserved_domains: false,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Card {
    pub severity: DetectorSeverity,
}
impl Default for Card {
    fn default() -> Self {
        Self {
            severity: DetectorSeverity::Block,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Phone {
    pub severity: DetectorSeverity,
    pub include_reserved: bool,
}
impl Default for Phone {
    fn default() -> Self {
        Self {
            severity: DetectorSeverity::Warn,
            include_reserved: false,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WarnDetector {
    pub severity: DetectorSeverity,
}
impl Default for WarnDetector {
    fn default() -> Self {
        Self {
            severity: DetectorSeverity::Warn,
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Detectors {
    pub email: Email,
    pub card: Card,
    pub phone: Phone,
    pub home_path: WarnDetector,
    pub pem: Card,
    pub clipboard: Card,
    #[serde(alias = "clipboard_loose")]
    pub clipboard_field: WarnDetector,
    pub user_pattern: Option<Card>,
    #[serde(flatten)]
    pub user: std::collections::BTreeMap<String, Card>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub max_text_mib: u64,
    pub max_image_mib: u64,
    pub max_image_megapixels: u64,
    pub ocr_timeout_secs: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_text_mib: 32,
            max_image_mib: 50,
            max_image_megapixels: 50,
            ocr_timeout_secs: 20,
        }
    }
}
impl Limits {
    pub fn text_bytes(&self) -> u64 {
        self.max_text_mib * 1024 * 1024
    }
    pub fn image_bytes(&self) -> u64 {
        self.max_image_mib * 1024 * 1024
    }
    pub fn pixels(&self) -> u64 {
        self.max_image_megapixels * 1_000_000
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(skip)]
    pub user_patterns: Vec<crate::patterns::UserPattern>,
    #[serde(skip)]
    pub deprecated_clipboard_loose: bool,
    pub version: u32,
    pub policy: Policy,
    pub detectors: Detectors,
    pub limits: Limits,
    pub allow: Vec<AllowRule>,
    #[serde(skip)]
    pub origin: Option<String>,
    #[serde(skip)]
    pub sha256: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            user_patterns: Vec::new(),
            deprecated_clipboard_loose: false,
            version: 1,
            policy: Policy::default(),
            detectors: Detectors::default(),
            limits: Limits::default(),
            allow: Vec::new(),
            origin: None,
            sha256: None,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid configuration key: {0}")]
    Key(String),
    #[error("found .leaklens.toml; rename it to .hushpush.toml")]
    LegacyConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowRule {
    pub id: String,
    pub paths: Vec<String>,
    pub detectors: Option<Vec<String>>,
    pub reason: String,
    pub expires: Option<NaiveDate>,
    #[serde(skip)]
    matcher: GlobSet,
}

impl AllowRule {
    /// The caller supplies the display path used in the report.
    pub fn matches(&self, path: &str, detector: &str) -> bool {
        self.matcher.is_match(path)
            && self
                .detectors
                .as_ref()
                .is_none_or(|ids| ids.iter().any(|id| id == detector))
    }

    pub fn expired(&self, today: NaiveDate) -> bool {
        self.expires.is_some_and(|expiry| expiry < today)
    }
}

impl Config {
    pub fn masked_path(&self, path: &str) -> String {
        let path = sanitize_path(path);
        let mut spans: Vec<_> = self
            .user_patterns
            .iter()
            .flat_map(|pattern| pattern.find(&path))
            .map(|m| m.span)
            .collect();
        spans.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
        let mut masked = String::new();
        let mut cursor = 0;
        for span in spans {
            if span.start >= cursor {
                masked.push_str(&path[cursor..span.start]);
                masked.push_str("[REDACTED]");
                cursor = span.end;
            }
        }
        masked.push_str(&path[cursor..]);
        masked
    }
    pub fn detector_list(&self) -> Vec<Box<dyn Detector>> {
        use crate::detect::*;
        let mut list: Vec<Box<dyn Detector>> = vec![
            Box::new(CardDetector),
            Box::new(EmailDetector::new(
                self.detectors.email.include_reserved_domains,
            )),
            Box::new(PhoneDetector::new(self.detectors.phone.include_reserved)),
            Box::new(HomePathDetector),
            Box::new(PemDetector),
            Box::new(ClipboardDetector),
            Box::new(ClipboardFieldDetector),
        ];
        list.extend(
            self.user_patterns
                .iter()
                .cloned()
                .map(|p| Box::new(p) as Box<dyn Detector>),
        );
        list
    }
    pub fn severity_for(&self, detector: &dyn Detector) -> Option<Severity> {
        match detector.id() {
            "email" => self.detectors.email.severity.severity(),
            "card" => self.detectors.card.severity.severity(),
            "phone" => self.detectors.phone.severity.severity(),
            "home_path" => self.detectors.home_path.severity.severity(),
            "pem" => self.detectors.pem.severity.severity(),
            "clipboard" => self.detectors.clipboard.severity.severity(),
            "clipboard_field" => self.detectors.clipboard_field.severity.severity(),
            id => self
                .detectors
                .user
                .get(id)
                .map(|setting| setting.severity)
                .or_else(|| {
                    self.detectors
                        .user_pattern
                        .as_ref()
                        .map(|setting| setting.severity)
                })
                .map_or(Some(detector.severity()), DetectorSeverity::severity),
        }
    }
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let value: toml::Value = text.parse().map_err(|error: toml::de::Error| {
            // Syntax errors never echo the source line or parser diagnostic.
            let start = error.span().map_or(0, |s| s.start.min(text.len()));
            if let Some(context) = allow_syntax_context(&text[..start]) {
                return ConfigError::Key(context);
            }
            let line_start = text[..start].rfind('\n').map_or(0, |n| n + 1);
            let line = text[line_start..].lines().next().unwrap_or("");
            let key = line.split_once('=').map(|(key, _)| key.trim());
            ConfigError::Key(
                key.filter(|key| !key.is_empty())
                    .map(sanitize_path)
                    .unwrap_or_else(|| "config".into()),
            )
        })?;
        let deprecated_clipboard_loose = value
            .get("detectors")
            .and_then(|v| v.get("clipboard_loose"))
            .is_some();
        validate("", &value)?;
        let mut config: Self = value
            .try_into()
            .map_err(|_| ConfigError::Key("config".into()))?;
        config.deprecated_clipboard_loose = deprecated_clipboard_loose;
        for rule in &mut config.allow {
            let mut builder = GlobSetBuilder::new();
            for pattern in &rule.paths {
                builder.add(Glob::new(pattern).map_err(|_| allow_error(&rule.id, "paths"))?);
            }
            rule.matcher = builder
                .build()
                .map_err(|_| allow_error(&rule.id, "paths"))?;
        }
        Ok(config)
    }
    pub fn load(explicit: Option<&Path>, root: &Path, routing: bool) -> Result<Self, ConfigError> {
        let path = if let Some(explicit) = explicit {
            explicit.to_path_buf()
        } else if let Some(root) = crate::source::git::working_tree_root(root, routing) {
            let path = root.join(".hushpush.toml");
            match fs::symlink_metadata(&path) {
                Ok(_) => path,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if fs::symlink_metadata(root.join(".leaklens.toml")).is_ok() {
                        return Err(ConfigError::LegacyConfig);
                    }
                    return Ok(Self::default());
                }
                Err(_) => return Err(ConfigError::Key("config".into())),
            }
        } else {
            return Ok(Self::default());
        };
        let bytes = fs::read(&path).map_err(|_| ConfigError::Key("config".into()))?;
        let text = std::str::from_utf8(&bytes).map_err(|_| ConfigError::Key("config".into()))?;
        let mut config = Self::parse(text)?;
        let checksum = format!("{:x}", Sha256::digest(&bytes));
        let label = if explicit.is_none() {
            ".hushpush.toml".into()
        } else {
            sanitize_path(&path.to_string_lossy())
        };
        config.origin = Some(format!("{label} (sha256 {})", &checksum[..8]));
        config.sha256 = Some(checksum);
        Ok(config)
    }
    pub fn apply_findings(&self, findings: &mut Vec<Finding>) {
        let detectors = self.detector_list();
        findings.retain_mut(|finding| {
            let Some(detector) = detectors.iter().find(|d| d.id() == finding.detector) else {
                return true;
            };
            if let Some(severity) = self.severity_for(detector.as_ref()) {
                finding.severity = severity;
                true
            } else {
                false
            }
        });
    }
}

fn validate(path: &str, value: &toml::Value) -> Result<(), ConfigError> {
    if path == "allow" {
        return validate_allows(value);
    }
    let user_table = path
        .strip_prefix("detectors.user:")
        .is_some_and(valid_allow_id);
    let table = user_table
        || matches!(
            path,
            "" | "policy"
                | "detectors"
                | "detectors.email"
                | "detectors.card"
                | "detectors.phone"
                | "detectors.home_path"
                | "detectors.pem"
                | "detectors.clipboard"
                | "detectors.clipboard_field"
                | "detectors.clipboard_loose"
                | "detectors.user_pattern"
                | "limits"
        );
    if table {
        let entries = value
            .as_table()
            .ok_or_else(|| ConfigError::Key(path.into()))?;
        for (key, value) in entries {
            let allowed = match path {
                "" => matches!(
                    key.as_str(),
                    "version" | "policy" | "detectors" | "limits" | "allow"
                ),
                "policy" => matches!(key.as_str(), "on_incomplete" | "on_unsupported"),
                "detectors" => {
                    matches!(
                        key.as_str(),
                        "email"
                            | "card"
                            | "phone"
                            | "home_path"
                            | "pem"
                            | "clipboard"
                            | "clipboard_field"
                            | "clipboard_loose"
                            | "user_pattern"
                    ) || key.strip_prefix("user:").is_some_and(valid_allow_id)
                }
                "detectors.email" => {
                    matches!(key.as_str(), "severity" | "include_reserved_domains")
                }
                "detectors.phone" => matches!(key.as_str(), "severity" | "include_reserved"),
                "detectors.card"
                | "detectors.home_path"
                | "detectors.pem"
                | "detectors.clipboard"
                | "detectors.clipboard_field"
                | "detectors.clipboard_loose"
                | "detectors.user_pattern" => key == "severity",
                "limits" => matches!(
                    key.as_str(),
                    "max_text_mib" | "max_image_mib" | "max_image_megapixels" | "ocr_timeout_secs"
                ),
                _ if user_table => key == "severity",
                _ => false,
            };
            if !allowed {
                return Err(ConfigError::Key(sanitize_path(&if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                })));
            }
            validate(
                &if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                },
                value,
            )?;
        }
        return Ok(());
    }
    let valid = match path {
        "version" => value.as_integer() == Some(1),
        "policy.on_incomplete" | "policy.on_unsupported" => {
            matches!(value.as_str(), Some("block" | "warn"))
        }
        path if path.starts_with("detectors.") && path.ends_with(".severity") => {
            matches!(value.as_str(), Some("block" | "warn" | "off"))
        }
        "detectors.email.include_reserved_domains" | "detectors.phone.include_reserved" => {
            value.as_bool().is_some()
        }
        "limits.max_text_mib" => value.as_integer().is_some_and(|n| (1..=256).contains(&n)),
        "limits.max_image_mib" | "limits.max_image_megapixels" => {
            value.as_integer().is_some_and(|n| (1..=200).contains(&n))
        }
        "limits.ocr_timeout_secs" => value.as_integer().is_some_and(|n| (1..=120).contains(&n)),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ConfigError::Key(sanitize_path(path)))
    }
}

fn allow_error(id: &str, field: &str) -> ConfigError {
    ConfigError::Key(format!("allow:{id}.{field}"))
}

fn valid_allow_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && !contains_detected_value(id)
}

fn contains_detected_value(text: &str) -> bool {
    !crate::allow::valid_reason(text) && !text.trim().is_empty()
}

fn allow_syntax_context(prefix: &str) -> Option<String> {
    let mut in_allow = false;
    let mut id = None;
    for line in prefix.lines() {
        let trimmed = line.trim();
        if trimmed == "[[allow]]" {
            in_allow = true;
            id = None;
        } else if trimmed.starts_with('[') {
            in_allow = false;
            id = None;
        } else if in_allow
            && let Ok(table) = trimmed.parse::<toml::Table>()
            && let Some(value) = table.get("id").and_then(toml::Value::as_str)
            && valid_allow_id(value)
        {
            id = Some(value.to_owned());
        }
    }
    in_allow.then(|| id.map_or_else(|| "allow.syntax".into(), |id| format!("allow:{id}.syntax")))
}

fn catch_all(glob: &Glob) -> bool {
    let matcher = glob.compile_matcher();
    // Probe varied path shapes so alternate universal glob syntax cannot bypass
    // the scope guard. Broad ambiguous patterns are deliberately rejected.
    let paths = [
        "a",
        "Z",
        "0",
        "_",
        "-",
        ".hidden",
        "README",
        "a.txt",
        "a.log",
        "a.rs",
        "with space",
        "é",
        "文",
        "a/b",
        "a/.hidden",
        ".hidden/b",
        "a/b/c/d/e",
        "docs/fixtures/fixture.txt",
        "src/main.rs",
        "other/no-extension",
    ];
    paths.iter().all(|path| matcher.is_match(path))
        && (b'!'..=b'~')
            .filter(|c| *c != b'/')
            .all(|c| matcher.is_match(char::from(c).to_string()))
}

fn validate_allows(value: &toml::Value) -> Result<(), ConfigError> {
    let rules = value
        .as_array()
        .ok_or_else(|| ConfigError::Key("allow".into()))?;
    let mut ids = HashSet::new();
    for rule in rules {
        let table = rule
            .as_table()
            .ok_or_else(|| ConfigError::Key("allow".into()))?;
        let id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .filter(|id| valid_allow_id(id))
            .ok_or_else(|| ConfigError::Key("allow.id".into()))?;
        if !ids.insert(id) {
            return Err(allow_error(id, "duplicate id"));
        }
        if table.keys().any(|key| {
            !matches!(
                key.as_str(),
                "id" | "paths" | "detectors" | "reason" | "expires"
            )
        }) {
            return Err(allow_error(id, "unknown field"));
        }
        if !table
            .get("reason")
            .and_then(toml::Value::as_str)
            .is_some_and(|reason| !reason.trim().is_empty())
        {
            return Err(allow_error(id, "reason"));
        }
        if contains_detected_value(table["reason"].as_str().unwrap()) {
            return Err(allow_error(id, "reason contains detected data"));
        }
        let paths = table
            .get("paths")
            .and_then(toml::Value::as_array)
            .filter(|paths| !paths.is_empty())
            .ok_or_else(|| allow_error(id, "paths"))?;
        for pattern in paths {
            let pattern = pattern.as_str().ok_or_else(|| allow_error(id, "paths"))?;
            let glob = Glob::new(pattern).map_err(|_| allow_error(id, "paths"))?;
            if pattern.is_empty()
                || matches!(pattern, "*" | "**" | "**/*")
                || catch_all(&glob)
                || contains_detected_value(pattern)
            {
                return Err(allow_error(id, "paths"));
            }
        }
        if let Some(detectors) = table.get("detectors") {
            let detectors = detectors
                .as_array()
                .ok_or_else(|| allow_error(id, "detectors"))?;
            if detectors.iter().any(|detector| {
                !detector.as_str().is_some_and(|id| {
                    matches!(
                        id,
                        "email"
                            | "card"
                            | "phone"
                            | "home_path"
                            | "pem"
                            | "clipboard"
                            | "clipboard_field"
                    ) || id.strip_prefix("user:").is_some_and(valid_allow_id)
                })
            }) {
                return Err(allow_error(id, "detectors"));
            }
        }
        if let Some(expires) = table.get("expires") {
            let date = expires.as_str().ok_or_else(|| allow_error(id, "expires"))?;
            let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| allow_error(id, "expires"))?;
            if date.len() != 10 || parsed.format("%Y-%m-%d").to_string() != date {
                return Err(allow_error(id, "expires"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULE: &str = "[[allow]]\nid = 'docs-fixtures'\npaths = ['docs/fixtures/**']\nreason = 'Synthetic documentation fixtures'\n";

    #[test]
    fn loose_clipboard_has_independent_config() {
        for (setting, severity) in [
            ("off", None),
            ("block", Some(Severity::Block)),
            ("warn", Some(Severity::Warn)),
        ] {
            let config = Config::parse(&format!(
                "version=1\n[detectors.clipboard_field]\nseverity='{setting}'"
            ))
            .unwrap();
            assert_eq!(
                config.severity_for(&crate::detect::ClipboardFieldDetector),
                severity
            );
            assert_eq!(
                config.severity_for(&crate::detect::ClipboardDetector),
                Some(Severity::Block)
            );
        }
        assert_eq!(
            Config::default().severity_for(&crate::detect::ClipboardFieldDetector),
            Some(Severity::Warn)
        );
    }

    #[test]
    fn scoped_allow_matches_display_path_and_detector() {
        let config = Config::parse(&format!(
            "{RULE}detectors = ['email']\nexpires = '2027-01-31'\n"
        ))
        .unwrap();
        let rule = &config.allow[0];
        assert!(rule.matches("docs/fixtures/log.txt", "email"));
        assert!(!rule.matches("docs/fixtures/log.txt", "card"));
        assert!(!rule.matches("src/log.txt", "email"));
        assert!(!rule.expired(NaiveDate::from_ymd_opt(2027, 1, 31).unwrap()));
        assert!(rule.expired(NaiveDate::from_ymd_opt(2027, 2, 1).unwrap()));
    }

    #[test]
    fn omitted_detectors_matches_all() {
        let config = Config::parse(RULE).unwrap();
        assert!(config.allow[0].matches("docs/fixtures/log.txt", "card"));
    }

    #[test]
    fn invalid_allows_name_rule_without_echoing_values() {
        for text in [
            format!("{RULE}{RULE}"),
            RULE.replace("reason = 'Synthetic documentation fixtures'", ""),
            format!("{RULE}expires = 'invalid-private-date'"),
            format!("{RULE}unknown_private_field = 'private-value'"),
            format!("{RULE}expires = '2027-02-30'"),
            format!("{RULE}expires = '2027-1-1'"),
            RULE.replace("docs/fixtures/**", "[private-value"),
        ] {
            let error = Config::parse(&text).unwrap_err().to_string();
            assert!(error.contains("docs-fixtures"));
            assert!(!error.contains("private"));
        }
    }

    #[test]
    fn catch_all_and_invalid_ids_are_rejected() {
        for pattern in ["*", "**", "**/*", "**/**", "{*,**}", "{**,docs/**}", "?*"] {
            let error = Config::parse(&RULE.replace("docs/fixtures/**", pattern))
                .unwrap_err()
                .to_string();
            assert!(error.contains("docs-fixtures"));
        }
        for id in [
            "",
            "Uppercase",
            "with space",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(Config::parse(&RULE.replace("docs-fixtures", id)).is_err());
        }
    }

    #[test]
    fn broad_but_scoped_globs_remain_valid() {
        for pattern in ["docs/**", "**/*.log", "*.txt", "[!x]*", "{docs,src}/**"] {
            assert!(Config::parse(&RULE.replace("docs/fixtures/**", pattern)).is_ok());
        }
    }

    #[test]
    fn malformed_allow_syntax_names_rule_without_echoing_keys_or_values() {
        for suffix in [
            "expires = 'not-terminated",
            "'fixture@example.com' = [",
            "paths = [",
        ] {
            let error = Config::parse(&format!("{RULE}{suffix}"))
                .unwrap_err()
                .to_string();
            assert!(error.contains("docs-fixtures"));
            assert!(!error.contains("fixture@example.com"));
            assert!(!error.contains("not-terminated"));
        }
    }

    #[test]
    fn scope_rules_reject_detected_values_and_unknown_detectors() {
        for value in ["fixture@example.com", "4242424242424242"] {
            for text in [
                RULE.replace("Synthetic documentation fixtures", value),
                RULE.replace("docs/fixtures/**", &format!("fixtures/{value}/**")),
                format!("{RULE}detectors = ['{value}']"),
            ] {
                let error = Config::parse(&text).unwrap_err().to_string();
                assert!(error.contains("docs-fixtures"));
                assert!(!error.contains(value));
            }
        }
        assert!(Config::parse(&format!("{RULE}detectors = ['unknown']")).is_err());
        let error = Config::parse(&RULE.replace("docs-fixtures", "4242424242424242"))
            .unwrap_err()
            .to_string();
        assert!(!error.contains("4242424242424242"));
    }
}
