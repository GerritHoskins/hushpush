# Limitations and security boundaries


OCR regions on the same visual row are joined for detection while preserving their character boxes. Image redaction makes up to three additional masking passes when changed OCR segmentation reveals another finding; the saved copy must still pass a final rescan. Persistent findings or failed rescans prevent keeping the copy.

A run with no findings does **not** mean an artifact is safe to publish. hushpush misses:
- national phone numbers without a country code
- clipboard field keys other than the documented keys
- unsupported formats: PDF (`pdf`), archives (`archive`), MP4/MOV (`video`), and GIF/WebP/BMP/TIFF/HEIC/AVIF (`image-format-unsupported`)
- non-Latin text in images (text home-path usernames accept Unicode letters and marks)
- rotated, stylized or low-resolution text
- free-form personal prose (names, addresses, messages)
- values split across physical lines or images
- encoded content beyond the supported JSON string escapes
- secrets other than those listed above, so run a secret scanner as well

### Security boundaries

PNG and JPEG dimensions are checked before decoding against the configured pixel budget; oversized images are incomplete (`image-too-large`). Decode failures are incomplete (`image-decode-failed`). Supported JPEG EXIF orientation is applied before OCR and redaction, and copies are PNG pixels without original metadata. Empty text is scanned; invalid UTF-8 is scanned lossily with partial coverage.

Directory scans list symlinks without following them, including loops and links outside the scan root. Nested `.git` files and directories are skipped while files in submodule and linked worktrees are scanned. Git scans classify LFS pointer blobs as unsupported; the external LFS object is never fetched. Printed paths and previews neutralize terminal, bidi and zero-width controls.

Redaction refuses symlink output paths and ancestors, checks again immediately before publishing the verified temporary copy, and refuses overwrite. If a matched value remains in a replacement marker or surrounding text, or substitutions reveal new findings, text redaction conservatively masks the whole copy (empty when even the opaque marker matches); review every copy for lost context. These checks assume a trusted local workspace: a hostile process concurrently changing directories can still race a path-based filesystem operation. Redaction is not a sandbox against other processes with write access to the output tree.

Decoder, parser and text-redaction fuzz targets run in a digest-pinned Docker nightly environment; see [fuzz instructions](../fuzz/README.md). A short smoke run or a clean rescan does not establish real-world detection accuracy or guarantee safe publication.
