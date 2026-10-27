//! Repository-local keyed fingerprints; detected values never enter serialized state.

use crate::Sensitive;
use chrono::{NaiveDate, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum AllowError {
    #[error("git is unavailable")]
    GitUnavailable,
    #[error("cannot access repository-local allow storage")]
    Storage,
    #[error("invalid local allow file")]
    InvalidFile,
    #[error("fingerprint must be 16 lowercase hexadecimal characters")]
    Fingerprint,
    #[error("duplicate value fingerprint")]
    Duplicate,
    #[error("allow reason must be non-empty and contain no detected values")]
    Reason,
    #[error("expiry must be an ISO date (YYYY-MM-DD)")]
    Date,
    #[error("value allows are unavailable outside a Git repository")]
    NotRepository,
}

pub fn today() -> NaiveDate {
    Utc::now().date_naive()
}

pub fn parse_date(value: &str) -> Result<NaiveDate, AllowError> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| AllowError::Date)?;
    if date.format("%Y-%m-%d").to_string() != value {
        return Err(AllowError::Date);
    }
    Ok(date)
}

pub fn valid_fingerprint(fp: &str) -> bool {
    fp.len() == 16
        && fp
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn valid_reason(reason: &str) -> bool {
    use crate::detect::Detector;
    !reason.trim().is_empty()
        && crate::detect::EmailDetector::new(true)
            .find(reason)
            .is_empty()
        && crate::detect::CardDetector.find(reason).is_empty()
        && crate::detect::PhoneDetector::new(true)
            .find(reason)
            .is_empty()
        && crate::detect::HomePathDetector.find(reason).is_empty()
        && crate::detect::PemDetector.find(reason).is_empty()
        && crate::detect::ClipboardDetector.find(reason).is_empty()
        && crate::detect::ClipboardFieldDetector
            .find(reason)
            .is_empty()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValueRule {
    pub fp: String,
    pub reason: String,
    pub added: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<NaiveDate>,
}

impl ValueRule {
    pub fn expired(&self) -> bool {
        self.expires.is_some_and(|date| date < today())
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Values {
    #[serde(default)]
    value: Vec<ValueRule>,
}

pub struct LocalStore {
    directory: PathBuf,
    key: Sensitive<[u8; 32]>,
    values: Values,
}

impl LocalStore {
    pub fn discover(root: &Path, routing: bool) -> Result<Option<Self>, AllowError> {
        let Some(directory) = storage_directory(root, routing)? else {
            return Ok(None);
        };
        safe_entry(&directory, true)?;
        if !directory.exists() {
            let builder = fs::DirBuilder::new();
            #[cfg(unix)]
            let builder = {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = builder;
                builder.mode(0o700);
                builder
            };
            builder
                .create(&directory)
                .map_err(|_| AllowError::Storage)?;
        }
        let key_path = directory.join("key");
        safe_entry(&key_path, false)?;
        if !key_path.exists() {
            let mut key = [0; 32];
            getrandom::fill(&mut key).map_err(|_| AllowError::Storage)?;
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&key_path) {
                Ok(mut file) => file.write_all(&key).map_err(|_| AllowError::Storage)?,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(AllowError::Storage),
            }
        }
        safe_entry(&key_path, false)?;
        if fs::metadata(&key_path)
            .map_err(|_| AllowError::Storage)?
            .len()
            != 32
        {
            return Err(AllowError::Storage);
        }
        let key: [u8; 32] = fs::read(&key_path)
            .map_err(|_| AllowError::Storage)?
            .try_into()
            .map_err(|_| AllowError::Storage)?;
        let path = directory.join("allow.toml");
        safe_entry(&path, false)?;
        let values = match fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<Values>(&text).map_err(|_| AllowError::InvalidFile)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Values::default(),
            Err(_) => return Err(AllowError::Storage),
        };
        let mut seen = std::collections::HashSet::new();
        for rule in &values.value {
            if !valid_fingerprint(&rule.fp) || !valid_reason(&rule.reason) || !seen.insert(&rule.fp)
            {
                return Err(AllowError::InvalidFile);
            }
        }
        Ok(Some(Self {
            directory,
            key: Sensitive::new(key),
            values,
        }))
    }

    pub fn fingerprint(&self, detector: &str, value: &Sensitive<String>) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(self.key.expose()).expect("HMAC accepts a 32-byte key");
        mac.update(detector.as_bytes());
        mac.update(&[0]);
        // Normalized material remains private and is never formatted or serialized.
        let normalized = Sensitive::new(match detector {
            "email" | "home_path" => value.expose().to_lowercase(),
            id if id.starts_with("user:") => value.expose().to_lowercase(),
            "pem" => value
                .expose()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect(),
            "card" | "phone" => value
                .expose()
                .chars()
                .filter(char::is_ascii_digit)
                .collect(),
            _ => value.expose().clone(),
        });
        mac.update(normalized.expose().as_bytes());
        mac.finalize().into_bytes()[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn values(&self) -> &[ValueRule] {
        &self.values.value
    }

    pub fn append(
        &mut self,
        fp: &str,
        reason: &str,
        expires: Option<&str>,
    ) -> Result<(), AllowError> {
        if !valid_fingerprint(fp) {
            return Err(AllowError::Fingerprint);
        }
        if !valid_reason(reason) {
            return Err(AllowError::Reason);
        }
        if self.values.value.iter().any(|rule| rule.fp == fp) {
            return Err(AllowError::Duplicate);
        }
        let expires = expires.map(parse_date).transpose()?;
        let rule = ValueRule {
            fp: fp.into(),
            reason: reason.into(),
            added: today(),
            expires,
        };
        let encoded = toml::to_string(&Values {
            value: vec![rule.clone()],
        })
        .map_err(|_| AllowError::Storage)?;
        let path = self.directory.join("allow.toml");
        safe_entry(&path, false)?;
        let mut options = fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|_| AllowError::Storage)?;
        writeln!(file, "\n{encoded}").map_err(|_| AllowError::Storage)?;
        self.values.value.push(rule);
        Ok(())
    }

    pub fn forget(root: &Path, routing: bool) -> Result<(), AllowError> {
        let directory = storage_directory(root, routing)?.ok_or(AllowError::NotRepository)?;
        safe_entry(&directory, true)?;
        if directory.exists() {
            fs::remove_dir_all(directory).map_err(|_| AllowError::Storage)?;
        }
        Ok(())
    }
}

fn safe_entry(path: &Path, directory: bool) -> Result<(), AllowError> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if !metadata.file_type().is_symlink()
                && if directory {
                    metadata.is_dir()
                } else {
                    metadata.is_file()
                } =>
        {
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(AllowError::Storage),
    }
}

fn storage_directory(root: &Path, routing: bool) -> Result<Option<PathBuf>, AllowError> {
    let output = crate::source::git::command(root, routing)
        .map_err(|_| AllowError::Storage)?
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AllowError::GitUnavailable
            } else {
                AllowError::Storage
            }
        })?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8(output.stdout).map_err(|_| AllowError::Storage)?;
    let common = PathBuf::from(text.trim_end_matches(['\r', '\n']));
    if !common.is_absolute() {
        return Err(AllowError::Storage);
    }
    let common = fs::canonicalize(common).map_err(|_| AllowError::Storage)?;
    if let Some(tree) = crate::source::git::working_tree_root(root, routing) {
        let tree = fs::canonicalize(tree).map_err(|_| AllowError::Storage)?;
        // Git's conventional metadata directory is excluded from its working tree.
        if common.starts_with(&tree) && common != tree.join(".git") {
            return Err(AllowError::Storage);
        }
    }
    Ok(Some(common.join("leaklens")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(key: u8) -> LocalStore {
        LocalStore {
            directory: PathBuf::new(),
            key: Sensitive::new([key; 32]),
            values: Values::default(),
        }
    }

    #[test]
    fn fingerprints_normalize_and_separate_detectors_and_repositories() {
        let first = store(1);
        let second = store(2);
        let email = Sensitive::new("SYNTHETIC@EXAMPLE.TEST".into());
        let lower = Sensitive::new("synthetic@example.test".into());
        assert_eq!(
            first.fingerprint("email", &email),
            first.fingerprint("email", &lower)
        );
        assert_ne!(
            first.fingerprint("email", &email),
            second.fingerprint("email", &email)
        );
        assert_ne!(
            first.fingerprint("email", &email),
            first.fingerprint("card", &email)
        );
        let card = Sensitive::new("4111-1111 1111-1111".into());
        assert_eq!(
            first.fingerprint("card", &card),
            first.fingerprint("card", &Sensitive::new("4111111111111111".into()))
        );
        assert!(valid_fingerprint(&first.fingerprint("email", &email)));
    }

    #[test]
    fn t10_fingerprints_follow_each_normalization_contract() {
        let store = store(1);
        for (id, a, b) in [
            ("phone", "+49 (30) 8765-4321", "493087654321"),
            ("home_path", "SyntheticUser", "syntheticuser"),
            ("pem", "HEADER\r\nU1lOVEg=\r\nEND", "HEADERU1lOVEg=END"),
            ("user:family", "Synthetic Family", "synthetic family"),
        ] {
            assert_eq!(
                store.fingerprint(id, &Sensitive::new(a.into())),
                store.fingerprint(id, &Sensitive::new(b.into()))
            );
        }
        let value = Sensitive::new("synthetic private note".into());
        for old_id in ["clipboard", "clipboard_loose"] {
            assert_ne!(
                store.fingerprint("clipboard_field", &value),
                store.fingerprint(old_id, &value)
            );
        }
        assert_ne!(
            store.fingerprint("clipboard", &Sensitive::new("Payload".into())),
            store.fingerprint("clipboard", &Sensitive::new("payload".into()))
        );
        assert_ne!(
            store.fingerprint("clipboard", &Sensitive::new("payload\r\nnext".into())),
            store.fingerprint("clipboard", &Sensitive::new("payload\nnext".into()))
        );
    }

    #[test]
    fn dates_and_fingerprints_are_strict() {
        for invalid in ["2027-1-01", "2027-02-29", "2027-01-01T00:00:00Z"] {
            assert!(parse_date(invalid).is_err());
        }
        assert!(parse_date("2028-02-29").is_ok());
        for invalid in [
            "ABCDEF0123456789",
            "abcdef012345678",
            "abcdef01234567890",
            "abcdef012345678g",
        ] {
            assert!(!valid_fingerprint(invalid));
        }
    }

    #[test]
    fn reasons_cannot_contain_detected_material() {
        for invalid in [
            "",
            "  ",
            "Fixture synthetic@example.test",
            "Fixture 4111 1111 1111 1111",
        ] {
            assert!(!valid_reason(invalid));
        }
        assert!(valid_reason("Synthetic docs fixture"));
    }

    #[test]
    fn local_schema_rejects_unknown_material() {
        assert!(toml::from_str::<Values>("[[value]]\nfp='0123456789abcdef'\nreason='Fixture'\nadded='2026-01-01'\nraw='forbidden'").is_err());
    }
}
