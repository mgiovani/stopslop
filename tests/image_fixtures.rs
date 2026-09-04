//! End-to-end fixtures for the image rules (SLOP045-047), built byte-for-byte in Rust rather
//! than committed as binary files -- no binary asset lives in this repo just to feed a test. A
//! byte-oriented `LintContext` can't carry an inline `expect:` marker, so this harness plays the
//! role `tests/integration.rs`'s marker harness plays for text fixtures (see that file's
//! `UNEXERCISED_LANGS` doc comment for why `Lang::Image` is excepted from it).

use std::collections::HashSet;
use stopslop::{lint_image, resolve_enabled, Settings, ALL_NATLANGS};

/// Selects the whole `"SLOP"` group, same as `tests/integration.rs`'s text-fixture harness, so an
/// unrelated rule over-firing on one of these fixtures is caught rather than silently masked by
/// selecting only SLOP045-047.
fn settings() -> Settings {
    Settings {
        enabled: resolve_enabled(&["SLOP".to_string()], &[], &[], &[], &[], false),
        deps: None,
        custom_rules: Vec::new(),
        natlangs: ALL_NATLANGS.to_vec(),
    }
}

fn codes(bytes: &[u8]) -> HashSet<&'static str> {
    lint_image("fixture".to_string(), bytes, &settings())
        .expect("fixture bytes must parse as a known container format")
        .into_iter()
        .map(|d| d.code)
        .collect()
}

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Table-free CRC-32 (poly 0xEDB88320), bit by bit -- the same algorithm `src/image.rs`'s own
/// unit tests use, so these PNGs carry a real per-chunk CRC and are byte-valid PNG files, without
/// vendoring a lookup table stopslop's own parser never reads (it walks past the CRC, never
/// checks it).
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

/// The parser never validates a chunk's CRC (it walks past it, see `crc32`'s own doc comment), so
/// nothing in the fixture-building path here would catch a wrong constant in `crc32` above --
/// pinned against a known-good value instead. `IEND`'s CRC is a fixed, publicly documented
/// constant precisely because every PNG encoder emits the same empty `IEND` chunk.
#[test]
fn crc32_self_check_against_known_iend_constant() {
    assert_eq!(crc32(b"IEND"), 0xAE42_6082);
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

fn ztxt_chunk(keyword: &str, payload: &[u8]) -> Vec<u8> {
    let mut data = keyword.as_bytes().to_vec();
    data.push(0); // null separator
    data.push(0); // compression method
    data.extend_from_slice(payload);
    data
}

fn itxt_chunk(keyword: &str, text: &str) -> Vec<u8> {
    let mut data = keyword.as_bytes().to_vec();
    data.push(0);
    data.push(0); // compression flag: uncompressed
    data.push(0); // compression method
    data.push(0); // empty language tag
    data.push(0); // empty translated keyword
    data.extend_from_slice(text.as_bytes());
    data
}

fn jpeg(segments: &[(u8, &[u8])]) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    for (marker, payload) in segments {
        out.push(0xFF);
        out.push(*marker);
        out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(payload);
    }
    out.extend_from_slice(&[0xFF, 0xDA, 0, 0]); // SOS: entropy-coded data begins
    out
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

// --- positive fixtures ---

#[test]
fn a1111_parameters_text_flags_slop045_only() {
    let bytes = png(&[
        (
            "tEXt",
            &text_chunk(
                "parameters",
                "a photo of a cat, steps: 20, sampler: Euler a, cfg scale: 7",
            ),
        ),
        ("IEND", &[]),
    ]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP045"]));
}

/// Compressed field, empty value: proves the keyword-only path fires without any usable text.
#[test]
fn comfyui_workflow_ztxt_flags_slop045_via_keyword_alone() {
    let bytes = png(&[
        ("zTXt", &ztxt_chunk("workflow", b"not-real-zlib-bytes")),
        ("IEND", &[]),
    ]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP045"]));
}

#[test]
fn xmp_itxt_trained_algorithmic_media_flags_slop046() {
    let xmp = "<x:xmpmeta><rdf:RDF><rdf:Description \
               iptcExt:DigitalSourceType=\"http://cv.iptc.org/newscodes/digitalsourcetype/\
               trainedAlgorithmicMedia\"/></rdf:RDF></x:xmpmeta>";
    let bytes = png(&[
        ("iTXt", &itxt_chunk("XML:com.adobe.xmp", xmp)),
        ("IEND", &[]),
    ]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP046"]));
}

#[test]
fn xmp_itxt_composite_with_trained_algorithmic_media_flags_slop046() {
    let xmp = "<x:xmpmeta><rdf:RDF><rdf:Description \
               iptcExt:DigitalSourceType=\"http://cv.iptc.org/newscodes/digitalsourcetype/\
               compositeWithTrainedAlgorithmicMedia\"/></rdf:RDF></x:xmpmeta>";
    let bytes = png(&[
        ("iTXt", &itxt_chunk("XML:com.adobe.xmp", xmp)),
        ("IEND", &[]),
    ]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP046"]));
}

/// `trainedAlgorithmicMedia` sitting between binary framing bytes, inside a `caBX` chunk --
/// exactly the shape `printable()` (image.rs) is built to pull out of a real C2PA CBOR manifest.
#[test]
fn cabx_chunk_carries_trained_algorithmic_media_between_binary_framing() {
    let mut data = vec![0xA1, 0x00, 0xDE, 0xAD, 0x02];
    data.extend_from_slice(b"trainedAlgorithmicMedia");
    data.extend_from_slice(&[0x00, 0xBE, 0xEF, 0x01]);
    let bytes = png(&[("caBX", &data), ("IEND", &[])]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP046"]));
}

#[test]
fn jpeg_app1_exif_names_midjourney_flags_slop047() {
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(b"Software: Midjourney v6.1");
    let bytes = jpeg(&[(0xE1, &payload)]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP047"]));
}

#[test]
fn webp_xmp_chunk_naming_a_generator_flags_slop047() {
    let bytes = webp(&[(b"XMP ", b"Generated with Stable Diffusion 1.5")]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP047"]));
}

// --- clean fixtures: each one is a false positive that would otherwise ship ---

#[test]
fn bare_png_with_no_text_chunks_is_clean() {
    let bytes = png(&[("IHDR", &[0; 13]), ("IDAT", &[1, 2, 3]), ("IEND", &[])]);
    assert!(codes(&bytes).is_empty());
}

/// What this repo's own `assets/findings.png` and a real ImageMagick-produced file carry: a zTXt
/// ICC color profile and a zTXt author field, neither an AI tell.
#[test]
fn imagemagick_icc_and_author_ztxt_is_clean() {
    let bytes = png(&[
        (
            "zTXt",
            &ztxt_chunk("Raw profile type icc", b"binary-icc-profile-payload"),
        ),
        ("zTXt", &ztxt_chunk("author", b"not-real-zlib-bytes")),
        ("IEND", &[]),
    ]);
    assert!(codes(&bytes).is_empty());
}

/// A real retouched camera photo: Canon EXIF plus an Adobe Photoshop `Software` tag. Must stay
/// silent, or SLOP047 would flag every camera-original or Photoshop-touched JPEG in existence.
#[test]
fn camera_retouched_jpeg_exif_is_clean() {
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(b"Canon EOS 10D\0Adobe Photoshop CS2 Windows");
    let bytes = jpeg(&[(0xE1, &payload)]);
    assert!(codes(&bytes).is_empty());
}

/// The camera-provenance case for SLOP046: a C2PA-shaped manifest naming a real camera and its
/// own actions, with no AI vocabulary term anywhere in it. No public sample of a real
/// camera-signed C2PA manifest exists to download, so this synthetic fixture is the only guard
/// SLOP046 has against flagging every Leica/Sony photo that ships a manifest.
#[test]
fn c2pa_camera_provenance_with_no_ai_vocabulary_is_clean() {
    let mut data = vec![0x00, 0x01, 0x02, 0x03];
    data.extend_from_slice(b"c2pa.assertions/c2pa.actions");
    data.extend_from_slice(&[0x00, 0x04, 0x05]);
    data.extend_from_slice(b"claim_generator: Leica M11-P firmware 1.2");
    data.extend_from_slice(&[0x00, 0x06]);
    let bytes = png(&[("caBX", &data), ("IEND", &[])]);
    assert!(codes(&bytes).is_empty());
}

/// The disjointness guard: a ComfyUI PNG's own `prompt` field literally contains "ComfyUI" in
/// its JSON, but that fact belongs to SLOP045 alone -- SLOP047 must skip any field whose key is
/// in SLOP045's panel, or the same file double-reports one signal.
#[test]
fn comfyui_named_inside_its_own_prompt_field_is_slop045_only() {
    let json = r#"{"generator": "ComfyUI", "nodes": []}"#;
    let bytes = png(&[("tEXt", &text_chunk("prompt", json)), ("IEND", &[])]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP045"]));
}

#[test]
fn clean_jpeg_with_no_metadata_segments_is_clean() {
    let bytes = jpeg(&[]);
    assert!(codes(&bytes).is_empty());
}

#[test]
fn clean_webp_with_no_metadata_chunks_is_clean() {
    let bytes = webp(&[]);
    assert!(codes(&bytes).is_empty());
}

/// A2: a real C2PA/EXIF field can declare `digitalSourceType` and name the generator that signed
/// the manifest in the same value, at the same offset -- confirmed on the real corpus
/// (`c2pa_ai_assertion.png`, `c2pa_ai_assertion_firefly_google.png`). SLOP047 must defer to
/// SLOP046 here, or the same field double-reports one fact.
#[test]
fn c2pa_field_declaring_source_type_and_naming_its_signer_is_slop046_only() {
    let mut data = vec![0x00, 0x01, 0x02];
    data.extend_from_slice(b"digitalSourceType trainedAlgorithmicMedia");
    data.extend_from_slice(&[0x00, 0x03]);
    data.extend_from_slice(b"claim_generator Adobe Firefly");
    data.extend_from_slice(&[0x00, 0x04]);
    let bytes = png(&[("caBX", &data), ("IEND", &[])]);
    assert_eq!(codes(&bytes), HashSet::from(["SLOP046"]));
}

/// B1: a real false positive a bare `.contains()` produced -- "medalled" contains "dalle". Must
/// stay silent, or SLOP047 would flag ordinary photo captions mentioning athletes.
#[test]
fn medalled_athlete_description_is_clean() {
    let bytes = png(&[
        (
            "tEXt",
            &text_chunk(
                "Description",
                "the gold-medalled athlete waved to the crowd",
            ),
        ),
        ("IEND", &[]),
    ]);
    assert!(codes(&bytes).is_empty());
}

/// B2: a real false positive a bare `.contains()` produced -- "recraft" used as an ordinary verb.
/// A word-boundary guard alone would not fix this one, since the whole word matches; `Recraft` is
/// dropped from the panel entirely.
#[test]
fn recraft_used_as_an_ordinary_verb_is_clean() {
    let bytes = png(&[
        (
            "tEXt",
            &text_chunk("Description", "we recraft your brand image every quarter"),
        ),
        ("IEND", &[]),
    ]);
    assert!(codes(&bytes).is_empty());
}

/// D: known blind spot, not a regression -- a zTXt value is always empty (image.rs adds no zlib
/// dependency), so a `digitalSourceType` declaration inside a compressed chunk is invisible to
/// SLOP046. Pinned so this negative reads as known rather than untested.
#[test]
fn compressed_source_type_field_is_a_known_blind_spot() {
    let bytes = png(&[
        (
            "zTXt",
            &ztxt_chunk("XML:com.adobe.xmp", b"not-real-zlib-bytes"),
        ),
        ("IEND", &[]),
    ]);
    assert!(codes(&bytes).is_empty());
}

/// D: same known blind spot for SLOP047 -- a generator name shipped only inside a compressed
/// text chunk is invisible, since the value the rule matches on is always empty.
#[test]
fn compressed_generator_name_field_is_a_known_blind_spot() {
    let bytes = png(&[
        ("zTXt", &ztxt_chunk("Software", b"not-real-zlib-bytes")),
        ("IEND", &[]),
    ]);
    assert!(codes(&bytes).is_empty());
}
