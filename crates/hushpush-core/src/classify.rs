//! Classification of source formats.

pub const MAX_TEXT_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentKind {
    Image,
    LfsPointer,
    Binary,
    Text,
    Unsupported(crate::report::SkipReason),
}

pub fn classify(bytes: &[u8]) -> ContentKind {
    if bytes.starts_with(b"version https://git-lfs.github.com/spec/v1") {
        ContentKind::LfsPointer
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(b"\xff\xd8\xff") {
        ContentKind::Image
    } else if bytes.starts_with(b"%PDF-") {
        ContentKind::Unsupported(crate::report::SkipReason::Pdf)
    } else if bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(b"BM")
        || bytes.starts_with(b"II\x2a\0")
        || bytes.starts_with(b"MM\0\x2a")
        || bytes.starts_with(b"II\x2b\0")
        || bytes.starts_with(b"MM\0\x2b")
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
    {
        ContentKind::Unsupported(crate::report::SkipReason::ImageFormatUnsupported)
    } else if bytes.get(4..8) == Some(b"ftyp") {
        let image_brand = bytes
            .get(8..)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|brand| {
                matches!(
                    brand,
                    b"heic"
                        | b"heix"
                        | b"hevc"
                        | b"hevx"
                        | b"heim"
                        | b"heis"
                        | b"mif1"
                        | b"msf1"
                        | b"avif"
                        | b"avis"
                )
            });
        ContentKind::Unsupported(if image_brand {
            crate::report::SkipReason::ImageFormatUnsupported
        } else {
            crate::report::SkipReason::Video
        })
    } else if bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
        || bytes.starts_with(b"\x1f\x8b")
        || bytes.starts_with(b"7z\xbc\xaf\x27\x1c")
    {
        ContentKind::Unsupported(crate::report::SkipReason::Archive)
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
    #[test]
    fn unsupported_magic_precedes_binary_and_text() {
        use crate::report::SkipReason::*;
        for (bytes, reason) in [
            (b"%PDF-1.7".as_slice(), Pdf),
            (b"GIF89a", ImageFormatUnsupported),
            (b"GIF87a", ImageFormatUnsupported),
            (b"RIFFxxxxWEBP", ImageFormatUnsupported),
            (b"BM", ImageFormatUnsupported),
            (b"II\x2a\0", ImageFormatUnsupported),
            (b"MM\0\x2a", ImageFormatUnsupported),
            (b"\0\0\0\x18ftypheic", ImageFormatUnsupported),
            (b"\0\0\0\x18ftypavif", ImageFormatUnsupported),
            (b"\0\0\0\x18ftypisom", Video),
            (b"\0\0\0\x18ftypqt  ", Video),
            (b"PK\x03\x04", Archive),
            (b"\x1f\x8b", Archive),
            (b"7z\xbc\xaf\x27\x1c", Archive),
        ] {
            assert_eq!(classify(bytes), ContentKind::Unsupported(reason));
        }
    }
}
