//! Classification of source formats.

pub const MAX_TEXT_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentKind {
    Image,
    LfsPointer,
    Binary,
    Text,
}

pub fn classify(bytes: &[u8]) -> ContentKind {
    if bytes.starts_with(b"version https://git-lfs.github.com/spec/v1") {
        ContentKind::LfsPointer
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(b"\xff\xd8\xff") {
        ContentKind::Image
    } else if bytes.iter().take(8192).any(|byte| *byte == 0) {
        ContentKind::Binary
    } else {
        ContentKind::Text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_not_extension_and_exact_binary_probe_limit() {
        assert_eq!(classify(b"\x89PNG\r\n\x1a\n\0"), ContentKind::Image);
        assert_eq!(classify(b"\xff\xd8\xff\0"), ContentKind::Image);
        let mut bytes = vec![b'x'; 8193];
        bytes[8192] = 0;
        assert_eq!(classify(&bytes), ContentKind::Text);
        bytes[8191] = 0;
        assert_eq!(classify(&bytes), ContentKind::Binary);
    }
}
