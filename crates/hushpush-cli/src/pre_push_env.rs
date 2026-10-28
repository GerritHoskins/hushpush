pub(crate) fn read() -> Result<(String, String), &'static str> {
    parse(|name| std::env::var(name).ok())
}

fn parse(mut get: impl FnMut(&str) -> Option<String>) -> Result<(String, String), &'static str> {
    let mut required = |name| {
        get(name)
            .filter(|value| !value.trim().is_empty() && !value.chars().any(char::is_control))
            .ok_or(name)
    };
    let from = required("PRE_COMMIT_FROM_REF")?;
    let to = required("PRE_COMMIT_TO_REF")?;
    let remote = required("PRE_COMMIT_REMOTE_NAME")?;
    let _url = required("PRE_COMMIT_REMOTE_URL")?;
    let local_branch = required("PRE_COMMIT_LOCAL_BRANCH")?;
    let remote_branch = required("PRE_COMMIT_REMOTE_BRANCH")?;
    for (name, oid) in [("PRE_COMMIT_FROM_REF", &from), ("PRE_COMMIT_TO_REF", &to)] {
        if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(name);
        }
    }
    for (name, branch) in [
        ("PRE_COMMIT_LOCAL_BRANCH", &local_branch),
        ("PRE_COMMIT_REMOTE_BRANCH", &remote_branch),
    ] {
        if !valid_ref(branch) {
            return Err(name);
        }
    }
    if !valid_ref(&remote) {
        return Err("PRE_COMMIT_REMOTE_NAME");
    }
    // These fields are passed as data to the existing scanner, never to a shell or rev parser.
    Ok((
        remote,
        format!("{local_branch} {to} {remote_branch} {from}\n"),
    ))
}

fn valid_ref(value: &str) -> bool {
    !value.starts_with(['-', '/'])
        && !value.ends_with(['.', '/'])
        && !value.contains("..")
        && !value.contains("//")
        && !value.contains("@{")
        && value != "@"
        && !value
            .chars()
            .any(|c| c.is_whitespace() || "~^:?*[\\".contains(c))
        && value
            .split('/')
            .all(|part| !part.starts_with('.') && !part.ends_with(".lock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> std::collections::BTreeMap<&'static str, String> {
        [
            ("PRE_COMMIT_FROM_REF", "0".repeat(40)),
            ("PRE_COMMIT_TO_REF", "a".repeat(40)),
            ("PRE_COMMIT_REMOTE_NAME", "origin".into()),
            ("PRE_COMMIT_REMOTE_URL", "synthetic-url".into()),
            ("PRE_COMMIT_LOCAL_BRANCH", "HEAD".into()),
            ("PRE_COMMIT_REMOTE_BRANCH", "refs/heads/main".into()),
        ]
        .into()
    }

    #[test]
    fn rejects_every_missing_empty_or_control_variable_without_exposing_values() {
        for name in values().keys() {
            for replacement in [
                None,
                Some(String::new()),
                Some(" ".into()),
                Some("synthetic\nvalue".into()),
            ] {
                let mut env = values();
                if let Some(value) = replacement {
                    env.insert(name, value);
                } else {
                    env.remove(name);
                }
                assert_eq!(parse(|key| env.get(key).cloned()), Err(*name));
            }
        }
    }

    #[test]
    fn rejects_invalid_oids_and_refs() {
        for (name, value) in [
            ("PRE_COMMIT_FROM_REF", "HEAD"),
            ("PRE_COMMIT_TO_REF", "not-an-oid"),
            ("PRE_COMMIT_REMOTE_NAME", "origin injected"),
            ("PRE_COMMIT_LOCAL_BRANCH", "HEAD injected"),
            ("PRE_COMMIT_REMOTE_BRANCH", "refs/heads/a..b"),
        ] {
            let mut env = values();
            env.insert(name, value.into());
            assert_eq!(parse(|key| env.get(key).cloned()), Err(name));
        }
    }

    #[test]
    fn maps_to_native_protocol_with_zero_or_existing_remote_oid() {
        for from in ["0".repeat(40), "b".repeat(40), "0".repeat(64)] {
            let mut env = values();
            let to = "a".repeat(from.len());
            env.insert("PRE_COMMIT_FROM_REF", from.clone());
            env.insert("PRE_COMMIT_TO_REF", to.clone());
            assert_eq!(
                parse(|key| env.get(key).cloned()),
                Ok((
                    "origin".into(),
                    format!("HEAD {to} refs/heads/main {from}\n")
                ))
            );
        }
    }
}
