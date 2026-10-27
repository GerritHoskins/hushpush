//! Working-tree configuration, independent of the objects being scanned.
use crate::{
    detect::Severity,
    report::{Finding, sanitize_path},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

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
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Detectors {
    pub email: Email,
    pub card: Card,
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
    pub version: u32,
    pub policy: Policy,
    pub detectors: Detectors,
    pub limits: Limits,
    #[serde(skip)]
    pub origin: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            policy: Policy::default(),
            detectors: Detectors::default(),
            limits: Limits::default(),
            origin: None,
        }
    }
}
#[derive(Debug, thiserror::Error)]
#[error("invalid configuration key: {0}")]
pub struct ConfigError(String);

impl Config {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let value: toml::Value = text.parse().map_err(|error: toml::de::Error| {
            // Syntax errors never echo the source line or parser diagnostic.
            let start = error.span().map_or(0, |s| s.start.min(text.len()));
            let line_start = text[..start].rfind('\n').map_or(0, |n| n + 1);
            let line = text[line_start..].lines().next().unwrap_or("");
            let key = line.split_once('=').map(|(key, _)| key.trim());
            ConfigError(
                key.filter(|key| !key.is_empty())
                    .map(sanitize_path)
                    .unwrap_or_else(|| "config".into()),
            )
        })?;
        validate("", &value)?;
        value.try_into().map_err(|_| ConfigError("config".into()))
    }
    pub fn load(explicit: Option<&Path>, root: &Path, routing: bool) -> Result<Self, ConfigError> {
        let path = if let Some(explicit) = explicit {
            explicit.to_path_buf()
        } else if let Some(root) = crate::source::git::working_tree_root(root, routing) {
            let path = root.join(".leaklens.toml");
            match fs::symlink_metadata(&path) {
                Ok(_) => path,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Self::default());
                }
                Err(_) => return Err(ConfigError("config".into())),
            }
        } else {
            return Ok(Self::default());
        };
        let bytes = fs::read(&path).map_err(|_| ConfigError("config".into()))?;
        let text = std::str::from_utf8(&bytes).map_err(|_| ConfigError("config".into()))?;
        let mut config = Self::parse(text)?;
        let checksum = format!("{:x}", Sha256::digest(&bytes));
        let label = if explicit.is_none() {
            ".leaklens.toml".into()
        } else {
            sanitize_path(&path.to_string_lossy())
        };
        config.origin = Some(format!("{label} (sha256 {})", &checksum[..8]));
        Ok(config)
    }
    pub fn apply_findings(&self, findings: &mut Vec<Finding>) {
        findings.retain_mut(|finding| {
            let severity = match finding.detector {
                "email" => self.detectors.email.severity,
                "card" => self.detectors.card.severity,
                _ => return true,
            };
            if let Some(severity) = severity.severity() {
                finding.severity = severity;
                true
            } else {
                false
            }
        });
    }
}

fn validate(path: &str, value: &toml::Value) -> Result<(), ConfigError> {
    let table = matches!(
        path,
        "" | "policy" | "detectors" | "detectors.email" | "detectors.card" | "limits"
    );
    if table {
        let entries = value.as_table().ok_or_else(|| ConfigError(path.into()))?;
        for (key, value) in entries {
            let allowed = match path {
                "" => matches!(key.as_str(), "version" | "policy" | "detectors" | "limits"),
                "policy" => matches!(key.as_str(), "on_incomplete" | "on_unsupported"),
                "detectors" => matches!(key.as_str(), "email" | "card"),
                "detectors.email" => {
                    matches!(key.as_str(), "severity" | "include_reserved_domains")
                }
                "detectors.card" => key == "severity",
                "limits" => matches!(
                    key.as_str(),
                    "max_text_mib" | "max_image_mib" | "max_image_megapixels" | "ocr_timeout_secs"
                ),
                _ => false,
            };
            if !allowed {
                return Err(ConfigError(sanitize_path(&if path.is_empty() {
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
        "detectors.email.severity" | "detectors.card.severity" => {
            matches!(value.as_str(), Some("block" | "warn" | "off"))
        }
        "detectors.email.include_reserved_domains" => value.as_bool().is_some(),
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
        Err(ConfigError(sanitize_path(path)))
    }
}
