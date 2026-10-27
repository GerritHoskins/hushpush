use std::fmt;

/// Content that is masked unless explicitly exposed.
///
/// Sensitive content cannot enter a serialized report accidentally.
/// ```compile_fail
/// use hushpush_core::Sensitive;
/// fn serializable<T: serde::Serialize>(_: &T) {}
/// serializable(&Sensitive::new(String::from("synthetic-marker")));
/// ```
#[derive(Clone)]
pub struct Sensitive<T>(T);

impl<T> Sensitive<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl<T> fmt::Display for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

#[cfg(test)]
mod tests {
    use super::Sensitive;

    #[test]
    fn formatting_masks_content_even_in_derived_debug() {
        #[derive(Debug)]
        struct Record {
            value: Sensitive<String>,
        }

        let planted = "synthetic-sensitive-marker";
        let record = Record {
            value: Sensitive::new(planted.to_owned()),
        };
        for output in [
            format!("{}", record.value),
            format!("{:?}", record.value),
            format!("{:#?}", record.value),
            format!("{record:?}"),
            format!("{record:#?}"),
        ] {
            assert!(output.contains("[redacted]"));
            assert!(!output.contains(planted));
        }
        assert_eq!(format!("{}", record.value), "[redacted]");
        assert_eq!(format!("{:?}", record.value), "[redacted]");
        assert_eq!(format!("{:#?}", record.value), "[redacted]");
    }

    #[test]
    fn exposure_is_explicit_and_clone_preserves_content() {
        let value = Sensitive::new(String::from("synthetic-marker"));
        assert_eq!(value.clone().expose(), value.expose());
    }

    #[test]
    fn formatting_does_not_require_inner_formatting_traits() {
        struct Opaque;
        let value = Sensitive::new(Opaque);
        assert_eq!(
            format!("{value} {value:?} {value:#?}"),
            "[redacted] [redacted] [redacted]"
        );
    }
}
