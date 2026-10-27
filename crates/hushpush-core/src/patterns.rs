//! User-owned detectors loaded exclusively from external configuration.

use crate::{
    Sensitive,
    detect::{Category, Confidence, Detector, Match, Severity},
};
use regex::{Regex, RegexBuilder};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

const REGEX_SIZE_LIMIT: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
#[error("invalid user patterns key: {0}")]
pub struct PatternError(&'static str);

pub struct LoadedPatterns {
    pub patterns: Vec<UserPattern>,
    pub warnings: Vec<&'static str>,
}

#[derive(Clone, Debug)]
pub struct UserPattern {
    id: String,
    expressions: Vec<(Sensitive<Regex>, bool)>,
    severity: Severity,
}

impl Detector for UserPattern {
    fn id(&self) -> &str {
        &self.id
    }
    fn version(&self) -> u32 {
        1
    }
    fn category(&self) -> Category {
        Category::UserPattern
    }
    fn severity(&self) -> Severity {
        self.severity
    }
    fn find(&self, text: &str) -> Vec<Match> {
        let mut spans = Vec::new();
        for (expression, whole_word) in &self.expressions {
            for found in expression.expose().find_iter(text) {
                if found.is_empty() {
                    continue;
                }
                if *whole_word
                    && (text[..found.start()].chars().next_back().is_some_and(word)
                        || text[found.end()..].chars().next().is_some_and(word))
                {
                    continue;
                }
                spans.push(found.range());
            }
        }
        spans.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
        spans.dedup();
        spans
            .into_iter()
            .map(|span| Match {
                span,
                confidence: Confidence::High,
            })
            .collect()
    }
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn parse(text: &str) -> Result<Vec<UserPattern>, PatternError> {
    let document: toml::Value = text.parse().map_err(|_| PatternError("syntax"))?;
    let table = document.as_table().ok_or(PatternError("patterns"))?;
    if table
        .keys()
        .any(|key| !matches!(key.as_str(), "version" | "pattern"))
    {
        return Err(PatternError("unknown field"));
    }
    if table.get("version").and_then(toml::Value::as_integer) != Some(1) {
        return Err(PatternError("version"));
    }
    let Some(entries) = table.get("pattern") else {
        return Ok(Vec::new());
    };
    let entries = entries.as_array().ok_or(PatternError("pattern"))?;
    let mut ids = HashSet::new();
    let mut patterns = Vec::new();
    for entry in entries {
        let entry = entry.as_table().ok_or(PatternError("pattern"))?;
        if entry
            .keys()
            .any(|key| !matches!(key.as_str(), "id" | "literals" | "regex" | "severity"))
        {
            return Err(PatternError("pattern.unknown field"));
        }
        let id = entry
            .get("id")
            .and_then(toml::Value::as_str)
            .ok_or(PatternError("pattern.id"))?;
        if !(1..=40).contains(&id.len())
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !ids.insert(id)
        {
            return Err(PatternError("pattern.id"));
        }
        let severity = match entry.get("severity").and_then(toml::Value::as_str) {
            Some("block") => Severity::Block,
            Some("warn") => Severity::Warn,
            _ => return Err(PatternError("pattern.severity")),
        };
        let mut expressions = Vec::new();
        if let Some(literals) = entry.get("literals") {
            let literals = literals
                .as_array()
                .ok_or(PatternError("pattern.literals"))?;
            for literal in literals {
                let literal = literal
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or(PatternError("pattern.literals"))?;
                let expression = RegexBuilder::new(&regex::escape(literal))
                    .case_insensitive(true)
                    .size_limit(REGEX_SIZE_LIMIT)
                    .build()
                    .map_err(|_| PatternError("pattern.literals"))?;
                expressions.push((Sensitive::new(expression), true));
            }
        }
        if let Some(expression) = entry.get("regex") {
            let expression = expression
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or(PatternError("pattern.regex"))?;
            let expression = RegexBuilder::new(expression)
                .size_limit(REGEX_SIZE_LIMIT)
                .build()
                .map_err(|_| PatternError("pattern.regex"))?;
            expressions.push((Sensitive::new(expression), false));
        }
        if expressions.is_empty() {
            return Err(PatternError("pattern.literals or pattern.regex"));
        }
        patterns.push(UserPattern {
            id: format!("user:{id}"),
            expressions,
            severity,
        });
    }
    Ok(patterns)
}

fn default_path() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("hushpush/patterns.toml"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|path| path.join("hushpush/patterns.toml"))
    }
}

pub fn load(repository_root: &Path) -> Result<LoadedPatterns, PatternError> {
    let explicit = std::env::var_os("HUSHPUSH_PATTERNS");
    let path = explicit.as_ref().map(PathBuf::from).or_else(default_path);
    let Some(path) = path else {
        return Ok(LoadedPatterns {
            patterns: Vec::new(),
            warnings: Vec::new(),
        });
    };
    load_path(&path, repository_root, explicit.is_some())
}

fn load_path(path: &Path, root: &Path, required: bool) -> Result<LoadedPatterns, PatternError> {
    let canonical = match path.canonicalize() {
        Ok(path) => path,
        Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LoadedPatterns {
                patterns: Vec::new(),
                warnings: Vec::new(),
            });
        }
        Err(_) => return Err(PatternError("file")),
    };
    let root = root
        .canonicalize()
        .map_err(|_| PatternError("repository root"))?;
    if canonical.starts_with(&root) {
        return Err(PatternError("file must be outside repository"));
    }
    let metadata = fs::metadata(&canonical).map_err(|_| PatternError("file"))?;
    if !metadata.is_file() {
        return Err(PatternError("file"));
    }
    #[cfg(unix)]
    let warnings = {
        use std::os::unix::fs::PermissionsExt;
        let mut warnings = Vec::new();
        if metadata.permissions().mode() & 0o044 != 0 {
            warnings.push("User patterns file is group- or world-readable");
        }
        warnings
    };
    #[cfg(not(unix))]
    let warnings = Vec::new();
    let text = Sensitive::new(fs::read_to_string(&canonical).map_err(|_| PatternError("file"))?);
    Ok(LoadedPatterns {
        patterns: parse(text.expose())?,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const PATTERNS: &str = "version = 1\n[[pattern]]\nid = 'family-names'\nliterals = ['Jane Doe', 'Åsa']\nregex = '(?i)\\bproject-falcon\\b'\nseverity = 'block'\n";

    #[test]
    fn literals_are_case_insensitive_whole_words_with_byte_spans() {
        let patterns = parse(PATTERNS).unwrap();
        let detector = &patterns[0];
        assert_eq!(detector.id(), "user:family-names");
        assert_eq!(detector.version(), 1);
        assert_eq!(detector.severity(), Severity::Block);
        let text = "é JANE DOE / åsa PROJECT-FALCON Jane Doex xJane Doe _Jane Doe";
        assert_eq!(
            detector
                .find(text)
                .iter()
                .map(|m| m.span.clone())
                .collect::<Vec<_>>(),
            vec![3..11, 14..18, 19..33]
        );
    }

    #[test]
    fn regex_has_own_flags_and_multiline_byte_spans() {
        let source = "version=1\n[[pattern]]\nid='test'\nregex='(?s)begin.*?end'\nseverity='warn'";
        let patterns = parse(source).unwrap();
        assert_eq!(patterns[0].find("é begin\r\nend!")[0].span, 3..13);
        assert!(patterns[0].find("BEGINend").is_empty());
        assert_eq!(patterns[0].severity(), Severity::Warn);
    }

    #[test]
    fn invalid_data_is_never_echoed_in_errors() {
        for source in [
            PATTERNS.replace("family-names", "private-value"),
            PATTERNS.replace("family-names", "Private-Value"),
            PATTERNS.replace("'block'", "'private-value'"),
            PATTERNS.replace("(?i)\\bproject-falcon\\b", "[private-value"),
            PATTERNS.replace("version = 1", "version = 'private-value'"),
            "version = 'private-value".into(),
        ] {
            if let Err(error) = parse(&source) {
                assert!(!error.to_string().contains("private-value"));
            }
        }
        assert!(parse(&format!("{PATTERNS}{PATTERNS}")).is_err());
        assert!(parse(&PATTERNS.replace("family-names", &"a".repeat(41))).is_err());
        assert!(parse("version=1\n[[pattern]]\nid='x'\nseverity='block'").is_err());
        assert!(
            parse("version=1\n[[pattern]]\nid='x'\nregex='a{100000000}'\nseverity='block'")
                .is_err()
        );
    }

    #[test]
    fn debug_masks_literal_and_regex_contents() {
        let patterns = parse(PATTERNS).unwrap();
        let debug = format!("{:?}", patterns[0]);
        assert!(!debug.contains("Jane Doe"));
        assert!(!debug.contains("project-falcon"));
        assert!(debug.contains("[redacted]"));
    }

    #[test]
    fn external_synthetic_file_loads_without_exposing_contents() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let directory = root.parent().unwrap().join(format!(
            "hushpush-patterns-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("patterns.toml");
        fs::write(&path, PATTERNS).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let loaded = load_path(&path, root, true).unwrap();
        assert_eq!(loaded.patterns.len(), 1);
        #[cfg(unix)]
        assert_eq!(loaded.warnings.len(), 1);
        #[cfg(not(unix))]
        assert!(loaded.warnings.is_empty());
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn repository_files_are_rejected_before_parsing() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let error = match load_path(&root.join("Cargo.toml"), root, true) {
            Err(error) => error,
            Ok(_) => panic!("repository patterns accepted"),
        };
        assert_eq!(
            error.to_string(),
            "invalid user patterns key: file must be outside repository"
        );
        assert!(
            load_path(&root.join("missing-user-patterns.toml"), root, false)
                .unwrap()
                .patterns
                .is_empty()
        );
        assert!(load_path(&root.join("missing-user-patterns.toml"), root, true).is_err());
    }
}
