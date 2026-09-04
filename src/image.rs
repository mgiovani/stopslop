//! Byte-level metadata extraction for `Lang::Image`'s four container formats. This is the whole
//! I/O-free byte layer image rules read: `ImageDoc::parse` never touches a filesystem path
//! (`walk.rs` reads the bytes), and its output is immutable once built, same as `ProseDoc`.
//!
//! Format is decided by magic bytes, never the file extension: a `.png` whose bytes are really a
//! JPEG parses as a JPEG. `Lang::from_path` only sees the extension, so a mismatch between the
//! two is exactly the tell some rules exist to catch.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
}

#[derive(Debug, Clone)]
pub struct MetaField {
    /// tEXt/iTXt keyword, a segment name ("Exif", "XMP", "c2pa", "IPTC"), or a raw PNG chunk type
    /// ("caBX", "eXIf").
    pub key: String,
    /// UTF-8-lossy text, or a `printable` extract for a binary payload. Empty when `compressed`.
    pub value: String,
    /// Byte offset where the containing chunk/segment record starts in the file: the length
    /// prefix for PNG and JPEG, the FourCC for WebP (whichever field each format's walk reads
    /// first when it finds the record).
    pub offset: usize,
    /// zTXt, or an iTXt with its compression flag set: the keyword decoded above is readable, but
    /// the value is zlib-compressed and `value` is left empty rather than the compressed bytes.
    pub compressed: bool,
}

#[derive(Debug)]
pub struct ImageDoc {
    pub format: ImageFormat,
    pub fields: Vec<MetaField>,
}

/// Hard cap on collected fields per file. A hostile PNG can carry millions of zero-length chunks;
/// this bounds the `Vec` regardless of how many the walk finds, in the same spirit as
/// `MAX_FIELD_BYTES` bounding each one.
const MAX_FIELDS: usize = 256;

/// Hard cap on one field's value, in bytes, truncated (not dropped) at a UTF-8 char boundary. A
/// C2PA manifest or an embedded ComfyUI workflow graph can run to megabytes; no rule needs more
/// than a bounded prefix to see the tell it's looking for.
const MAX_FIELD_BYTES: usize = 64 * 1024;

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

impl ImageDoc {
    pub fn parse(bytes: &[u8]) -> Option<ImageDoc> {
        if bytes.starts_with(&PNG_SIGNATURE) {
            Some(ImageDoc {
                format: ImageFormat::Png,
                fields: parse_png(bytes),
            })
        } else if bytes.starts_with(&[0xFF, 0xD8]) {
            Some(ImageDoc {
                format: ImageFormat::Jpeg,
                fields: parse_jpeg(bytes),
            })
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(ImageDoc {
                format: ImageFormat::WebP,
                fields: parse_webp(bytes),
            })
        } else {
            None
        }
    }
}

fn push_field(
    fields: &mut Vec<MetaField>,
    key: String,
    value: String,
    offset: usize,
    compressed: bool,
) {
    fields.push(MetaField {
        key,
        value: truncate_value(value),
        offset,
        compressed,
    });
}

fn truncate_value(mut s: String) -> String {
    if s.len() > MAX_FIELD_BYTES {
        let mut end = MAX_FIELD_BYTES;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

/// Latin-1 is a direct byte-to-codepoint mapping for 0..=255, unlike UTF-8 -- this is what PNG
/// `tEXt`/`zTXt` keywords and values are specified to carry.
fn latin1_lossy(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Splits `data` at its first `0x00`, returning `(before, after)`, or `None` when there is no
/// null byte -- a malformed field the caller skips rather than misreads.
fn split_null(data: &[u8]) -> Option<(&[u8], &[u8])> {
    let pos = data.iter().position(|&b| b == 0)?;
    Some((&data[..pos], &data[pos + 1..]))
}

/// PNG chunk walk: `length` (u32 BE, data only), 4-byte ASCII type, `length` bytes of data, then
/// a 4-byte CRC this parser never validates (the metadata tell doesn't depend on it). Every
/// bound comes from `checked_add` + `slice::get`, so a hostile length can only shorten the walk,
/// never panic or allocate past what `bytes` already holds.
fn parse_png(bytes: &[u8]) -> Vec<MetaField> {
    let mut fields = Vec::new();
    let mut pos = PNG_SIGNATURE.len();
    while fields.len() < MAX_FIELDS {
        let chunk_start = pos;
        let Some(len_bytes) = bytes.get(pos..pos + 4) else {
            break;
        };
        let len = u32::from_be_bytes(len_bytes.try_into().unwrap()) as usize;
        let type_start = pos + 4;
        let Some(type_bytes) = bytes.get(type_start..type_start + 4) else {
            break;
        };
        if !type_bytes.iter().all(u8::is_ascii_alphabetic) {
            break;
        }
        let chunk_type = std::str::from_utf8(type_bytes).unwrap();
        let data_start = type_start + 4;
        let Some(data_end) = data_start.checked_add(len) else {
            break;
        };
        let Some(data) = bytes.get(data_start..data_end) else {
            break;
        };
        let Some(crc_end) = data_end.checked_add(4) else {
            break;
        };
        if bytes.get(data_end..crc_end).is_none() {
            break;
        }
        if chunk_type == "IEND" {
            break;
        }
        match chunk_type {
            "tEXt" => {
                if let Some((keyword, value)) = split_null(data) {
                    push_field(
                        &mut fields,
                        latin1_lossy(keyword),
                        latin1_lossy(value),
                        chunk_start,
                        false,
                    );
                }
            }
            "zTXt" => {
                if let Some((keyword, _rest)) = split_null(data) {
                    push_field(
                        &mut fields,
                        latin1_lossy(keyword),
                        String::new(),
                        chunk_start,
                        true,
                    );
                }
            }
            "iTXt" => {
                if let Some(field) = parse_itxt(data, chunk_start) {
                    fields.push(field);
                }
            }
            "caBX" | "eXIf" => {
                push_field(
                    &mut fields,
                    chunk_type.to_string(),
                    printable(data),
                    chunk_start,
                    false,
                );
            }
            _ => {}
        }
        pos = crc_end;
    }
    fields
}

/// keyword\0 compression-flag compression-method language-tag\0 translated-keyword\0 text. The
/// text is UTF-8 only when the compression flag is 0; a set flag means it's zlib-compressed, so
/// `value` is left empty (see `MetaField::compressed`'s doc comment).
fn parse_itxt(data: &[u8], offset: usize) -> Option<MetaField> {
    let (keyword, rest) = split_null(data)?;
    let &compression_flag = rest.first()?;
    let after_flags = rest.get(2..)?;
    let (_lang_tag, rest) = split_null(after_flags)?;
    let (_translated_keyword, text) = split_null(rest)?;
    let (value, compressed) = if compression_flag == 1 {
        (String::new(), true)
    } else {
        (String::from_utf8_lossy(text).into_owned(), false)
    };
    Some(MetaField {
        key: latin1_lossy(keyword),
        value: truncate_value(value),
        offset,
        compressed,
    })
}

const JPEG_EXIF_PREFIX: &[u8] = b"Exif\0\0";
const JPEG_XMP_PREFIX: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const JPEG_C2PA_PREFIX: &[u8] = b"JP";
const JPEG_IPTC_PREFIX: &[u8] = b"Photoshop 3.0\0";

/// JPEG segment walk, starting right after SOI (`FFD8`). `0xFF` fill bytes between segments are
/// skipped per the JPEG spec (any number may precede a marker); `0xD0..=0xD9` and `0x01` carry no
/// length and are stepped over whole. Stops at SOS (`FFDA`): everything past it is
/// entropy-coded scan data, not another segment.
fn parse_jpeg(bytes: &[u8]) -> Vec<MetaField> {
    let mut fields = Vec::new();
    let mut pos = 2usize;
    while fields.len() < MAX_FIELDS {
        if bytes.get(pos) != Some(&0xFF) {
            break;
        }
        pos += 1;
        while bytes.get(pos) == Some(&0xFF) {
            pos += 1;
        }
        let Some(&marker) = bytes.get(pos) else {
            break;
        };
        pos += 1;
        if marker == 0xDA {
            break;
        }
        if matches!(marker, 0xD0..=0xD9) || marker == 0x01 {
            continue;
        }
        let seg_start = pos;
        let Some(len_bytes) = bytes.get(pos..pos + 2) else {
            break;
        };
        let seg_len = u16::from_be_bytes(len_bytes.try_into().unwrap()) as usize;
        // seg_len counts its own two bytes, so it can never be smaller than that and still name
        // a valid segment.
        if seg_len < 2 {
            break;
        }
        let payload_start = pos + 2;
        let Some(payload_end) = seg_start.checked_add(seg_len) else {
            break;
        };
        let Some(payload) = bytes.get(payload_start..payload_end) else {
            break;
        };
        match marker {
            0xE1 if payload.starts_with(JPEG_EXIF_PREFIX) => {
                push_field(
                    &mut fields,
                    "Exif".to_string(),
                    printable(&payload[JPEG_EXIF_PREFIX.len()..]),
                    seg_start,
                    false,
                );
            }
            0xE1 if payload.starts_with(JPEG_XMP_PREFIX) => {
                push_field(
                    &mut fields,
                    "XMP".to_string(),
                    String::from_utf8_lossy(&payload[JPEG_XMP_PREFIX.len()..]).into_owned(),
                    seg_start,
                    false,
                );
            }
            0xEB if payload.starts_with(JPEG_C2PA_PREFIX) => {
                push_field(
                    &mut fields,
                    "c2pa".to_string(),
                    printable(&payload[JPEG_C2PA_PREFIX.len()..]),
                    seg_start,
                    false,
                );
            }
            0xED if payload.starts_with(JPEG_IPTC_PREFIX) => {
                push_field(
                    &mut fields,
                    "IPTC".to_string(),
                    printable(&payload[JPEG_IPTC_PREFIX.len()..]),
                    seg_start,
                    false,
                );
            }
            _ => {}
        }
        pos = payload_end;
    }
    fields
}

/// WebP RIFF chunk walk, starting right after the `RIFF` + size + `WEBP` header. Chunk sizes are
/// little-endian (unlike PNG/JPEG's big-endian lengths) and payloads are padded to an even byte
/// that the size field itself does not count.
fn parse_webp(bytes: &[u8]) -> Vec<MetaField> {
    let mut fields = Vec::new();
    let mut pos = 12usize;
    while fields.len() < MAX_FIELDS {
        let chunk_start = pos;
        let Some(fourcc) = bytes.get(pos..pos + 4) else {
            break;
        };
        let fourcc: [u8; 4] = fourcc.try_into().unwrap();
        pos += 4;
        let Some(size_bytes) = bytes.get(pos..pos + 4) else {
            break;
        };
        let size = u32::from_le_bytes(size_bytes.try_into().unwrap()) as usize;
        pos += 4;
        let Some(payload_end) = pos.checked_add(size) else {
            break;
        };
        let Some(payload) = bytes.get(pos..payload_end) else {
            break;
        };
        match &fourcc {
            b"EXIF" => push_field(
                &mut fields,
                "Exif".to_string(),
                String::from_utf8_lossy(payload).into_owned(),
                chunk_start,
                false,
            ),
            b"XMP " => push_field(
                &mut fields,
                "XMP".to_string(),
                String::from_utf8_lossy(payload).into_owned(),
                chunk_start,
                false,
            ),
            _ => {}
        }
        pos = payload_end + (size % 2);
    }
    fields
}

/// Runs of 4+ printable-ASCII bytes (`0x20..=0x7E`), joined by `\n`. This is the whole reason the
/// crate needs no CBOR decoder, no zlib and no EXIF IFD parser: CBOR encodes text as
/// length-prefixed raw UTF-8 and TIFF stores ASCII tag values verbatim, so `trainedAlgorithmicMedia`,
/// a C2PA claim-generator string and an EXIF `Software` value all survive into this extract as
/// literal substrings between the binary framing bytes that break the runs around them.
///
/// ponytail: this can't tell which EXIF tag a string came from -- every run reads the same
/// whether it was tag `0x0131` (Software) or `0x010F` (Make). Upgrade path: parse IFD0 and read
/// tag `0x0131` by its actual offset once a rule needs that precision.
fn printable(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut run_start = None;
    for (i, &b) in bytes.iter().enumerate() {
        // Stop scanning once the cap is reached rather than extracting the whole payload and
        // truncating after: a C2PA manifest is attacker-controlled and can run to megabytes, and
        // building that String only to throw all but 64 KB away is the memory amplification
        // `MAX_FIELD_BYTES` exists to prevent.
        if out.len() >= MAX_FIELD_BYTES {
            break;
        }
        let is_printable = (0x20..=0x7E).contains(&b);
        match (is_printable, run_start) {
            (true, None) => run_start = Some(i),
            (false, Some(start)) => {
                push_printable_run(&mut out, &bytes[start..i]);
                run_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = run_start {
        if out.len() < MAX_FIELD_BYTES {
            push_printable_run(&mut out, &bytes[start..]);
        }
    }
    truncate_value(out)
}

fn push_printable_run(out: &mut String, run: &[u8]) {
    if run.len() < 4 {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    // `run` is ASCII 0x20..=0x7E by construction, so this is always valid UTF-8.
    out.push_str(std::str::from_utf8(run).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Table-free CRC-32 (poly 0xEDB88320), bit by bit, so the test PNGs below carry a real CRC
    /// per chunk without vendoring a lookup table stopslop's own parser never reads.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &b in bytes {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        crc ^ 0xFFFF_FFFF
    }

    fn png(chunks: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = PNG_SIGNATURE.to_vec();
        for (chunk_type, data) in chunks {
            let type_bytes = chunk_type.as_bytes();
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(type_bytes);
            out.extend_from_slice(data);
            let mut crc_input = type_bytes.to_vec();
            crc_input.extend_from_slice(data);
            out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        }
        out
    }

    fn text_chunk(keyword: &str, value: &str) -> Vec<u8> {
        let mut data = keyword.as_bytes().to_vec();
        data.push(0);
        data.extend_from_slice(value.as_bytes());
        data
    }

    fn ztxt_chunk(keyword: &str) -> Vec<u8> {
        let mut data = keyword.as_bytes().to_vec();
        data.push(0);
        data.push(0); // compression method
        data.extend_from_slice(b"not-real-zlib-data");
        data
    }

    fn itxt_chunk(keyword: &str, compressed: bool, text: &str) -> Vec<u8> {
        let mut data = keyword.as_bytes().to_vec();
        data.push(0);
        data.push(u8::from(compressed));
        data.push(0); // compression method
        data.push(0); // empty language tag
        data.push(0); // empty translated keyword
        data.extend_from_slice(text.as_bytes());
        data
    }

    #[test]
    fn png_collects_text_ztxt_and_itxt() {
        let bytes = png(&[
            ("IHDR", &[0; 13]),
            ("tEXt", &text_chunk("Comment", "hello")),
            ("zTXt", &ztxt_chunk("prompt")),
            ("iTXt", &itxt_chunk("workflow", false, "graph")),
            ("IEND", &[]),
        ]);
        let doc = ImageDoc::parse(&bytes).unwrap();
        assert_eq!(doc.format, ImageFormat::Png);
        let field = |key: &str| doc.fields.iter().find(|f| f.key == key).unwrap();

        let comment = field("Comment");
        assert_eq!(comment.value, "hello");
        assert!(!comment.compressed);

        let prompt = field("prompt");
        assert!(prompt.compressed);
        assert_eq!(prompt.value, "");

        let workflow = field("workflow");
        assert_eq!(workflow.value, "graph");
        assert!(!workflow.compressed);
    }

    #[test]
    fn png_itxt_compressed_flag_leaves_value_empty() {
        let bytes = png(&[
            ("iTXt", &itxt_chunk("workflow", true, "graph")),
            ("IEND", &[]),
        ]);
        let doc = ImageDoc::parse(&bytes).unwrap();
        assert_eq!(doc.fields.len(), 1);
        assert!(doc.fields[0].compressed);
        assert_eq!(doc.fields[0].value, "");
    }

    #[test]
    fn png_with_no_text_chunks_has_empty_fields() {
        let bytes = png(&[("IHDR", &[0; 13]), ("IDAT", &[1, 2, 3]), ("IEND", &[])]);
        let doc = ImageDoc::parse(&bytes).unwrap();
        assert!(doc.fields.is_empty());
    }

    #[test]
    fn png_truncated_chunk_header_stops_without_panic() {
        let mut bytes = png(&[("tEXt", &text_chunk("k", "v"))]);
        bytes.truncate(bytes.len() - 3); // cut into the CRC
        let doc = ImageDoc::parse(&bytes).unwrap();
        assert!(doc.fields.is_empty(), "the one chunk was truncated");
    }

    #[test]
    fn png_declared_length_past_end_stops_without_panic() {
        let mut bytes = png(&[("tEXt", &text_chunk("before", "ok"))]);
        // A second chunk header claiming far more data than actually follows.
        bytes.extend_from_slice(&500_000u32.to_be_bytes());
        bytes.extend_from_slice(b"tEXt");
        bytes.extend_from_slice(b"short");
        let doc = ImageDoc::parse(&bytes).unwrap();
        assert_eq!(doc.fields.len(), 1);
        assert_eq!(doc.fields[0].key, "before");
    }

    #[test]
    fn png_zero_length_chunk_does_not_loop_and_parsing_continues() {
        let bytes = png(&[
            ("tEXt", &text_chunk("first", "a")),
            ("unKn", &[]), // zero-length, unrecognized chunk type
            ("tEXt", &text_chunk("second", "b")),
            ("IEND", &[]),
        ]);
        let doc = ImageDoc::parse(&bytes).unwrap();
        let keys: Vec<&str> = doc.fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["first", "second"]);
    }

    #[test]
    fn extension_says_png_but_magic_says_jpeg() {
        let jpeg_bytes = jpeg(&[]);
        // `Lang::from_path` would resolve a ".png" file to `Lang::Image` from the extension
        // alone; `ImageDoc::parse` only ever looks at these bytes, so it must report the real
        // format regardless of what a caller named the file.
        let doc = ImageDoc::parse(&jpeg_bytes).unwrap();
        assert_eq!(doc.format, ImageFormat::Jpeg);
    }

    fn jpeg(segments: &[(u8, &[u8])]) -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8];
        for (marker, payload) in segments {
            out.push(0xFF);
            out.push(*marker);
            out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
            out.extend_from_slice(payload);
        }
        out.extend_from_slice(&[0xFF, 0xDA, 0, 0]); // SOS: entropy data begins
        out.push(0xAB); // one byte of "scan data" that must never be parsed as a segment
        out
    }

    #[test]
    fn jpeg_collects_exif_and_xmp_app1_segments_and_stops_at_sos() {
        let mut exif_payload = JPEG_EXIF_PREFIX.to_vec();
        exif_payload.extend_from_slice(b"Software: Midjourney v6");
        let mut xmp_payload = JPEG_XMP_PREFIX.to_vec();
        xmp_payload.extend_from_slice(b"<x:xmpmeta>ok</x:xmpmeta>");

        let bytes = jpeg(&[(0xE1, &exif_payload), (0xE1, &xmp_payload)]);
        let doc = ImageDoc::parse(&bytes).unwrap();
        assert_eq!(doc.format, ImageFormat::Jpeg);
        let field = |key: &str| doc.fields.iter().find(|f| f.key == key).unwrap();
        assert!(field("Exif").value.contains("Software: Midjourney v6"));
        assert_eq!(field("XMP").value, "<x:xmpmeta>ok</x:xmpmeta>");
    }

    #[test]
    fn jpeg_standalone_marker_mid_stream_does_not_break_parsing() {
        let mut exif_payload = JPEG_EXIF_PREFIX.to_vec();
        exif_payload.extend_from_slice(b"after-restart-marker");
        let mut bytes = vec![0xFF, 0xD8];
        bytes.extend_from_slice(&[0xFF, 0xD0]); // standalone RST0, no length
        bytes.push(0xFF);
        bytes.push(0xE1);
        bytes.extend_from_slice(&((exif_payload.len() + 2) as u16).to_be_bytes());
        bytes.extend_from_slice(&exif_payload);
        bytes.extend_from_slice(&[0xFF, 0xDA, 0, 0]);

        let doc = ImageDoc::parse(&bytes).unwrap();
        assert_eq!(doc.fields.len(), 1);
        assert!(doc.fields[0].value.contains("after-restart-marker"));
    }

    fn webp(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut payload = Vec::new();
        for (fourcc, data) in chunks {
            payload.extend_from_slice(*fourcc);
            payload.extend_from_slice(&(data.len() as u32).to_le_bytes());
            payload.extend_from_slice(data);
            if data.len() % 2 == 1 {
                payload.push(0);
            }
        }
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&((payload.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WEBP");
        out.extend_from_slice(&payload);
        out
    }

    #[test]
    fn webp_odd_size_payload_is_padded_and_next_chunk_still_parses() {
        let bytes = webp(&[
            (b"VP8 ", b"x"), // odd-length payload -> one pad byte follows
            (b"EXIF", b"Software=Firefly"),
        ]);
        let doc = ImageDoc::parse(&bytes).unwrap();
        assert_eq!(doc.format, ImageFormat::WebP);
        assert_eq!(doc.fields.len(), 1);
        assert_eq!(doc.fields[0].key, "Exif");
        assert_eq!(doc.fields[0].value, "Software=Firefly");
    }

    #[test]
    fn printable_drops_short_runs_and_keeps_long_ones() {
        let bytes = [b'a', b'b', 0, b'c', b'd', b'e', b'f', 0, b'g'];
        assert_eq!(printable(&bytes), "cdef");
    }

    #[test]
    fn printable_joins_multiple_long_runs_with_newline() {
        let bytes = [b'a', b'b', b'c', b'd', 0, b'e', b'f', b'g', b'h'];
        assert_eq!(printable(&bytes), "abcd\nefgh");
    }

    #[test]
    fn max_field_bytes_truncates_an_oversized_value() {
        let huge = "a".repeat(MAX_FIELD_BYTES + 1000);
        let bytes = png(&[("tEXt", &text_chunk("k", &huge)), ("IEND", &[])]);
        let doc = ImageDoc::parse(&bytes).unwrap();
        assert_eq!(doc.fields.len(), 1);
        assert_eq!(doc.fields[0].value.len(), MAX_FIELD_BYTES);
    }
}
