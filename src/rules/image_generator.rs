use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang;
use crate::registry::RuleDef;
use crate::rules::image_prompt;
use crate::rules::image_source_type;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP047",
    name: "Image metadata names a generator",
    tier: Tier::B,
    langs: lang::IMAGE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// Generator names distinctive enough on their own that seeing one anywhere in image metadata is
/// the tell: no ordinary photo, screenshot, or hand-edited image carries "Midjourney" or
/// "ComfyUI" in a metadata value for any other reason. Every entry is matched with a word-boundary
/// guard (`contains_word_bounded`, no ordinary alphanumeric flanking it), never a bare
/// `.contains()`: without it `DALLE` matched inside "medalled" and `Recraft` matched inside
/// "we recraft your brand image every quarter", both real false positives on ordinary English.
///
/// Deliberately excluded, each a real false positive: bare `Adobe`, `Photoshop`, `Canon`,
/// `Nikon`, `Sony` (every retouched or camera-original photo carries these in EXIF -- verified
/// against a real Canon EOS 10D JPEG whose EXIF reads "Adobe Photoshop CS2 Windows"; only the
/// full "Adobe Firefly" is distinctive enough to flag), and bare `Firefly`, `Imagen`, `Flux`,
/// `Grok`, `Aurora`, `Gemini` (ordinary words; `Imagen` is Portuguese for "image", and this
/// crate lints pt-BR prose).
///
/// Also removed, and not just guarded: bare `DALLE`. `DALL-E` and `DALL·E` above already cover
/// the real spellings, and the boundary guard alone would not save a hyphenless `dalle` form:
/// "medalled" is caught by the guard (both flanking letters are alphanumeric), but `dalle` is
/// also an ordinary standalone word in its own right (French for a paving slab or roof gutter),
/// which the guard cannot distinguish from a generator name since a whole-word match passes it
/// cleanly either way. And bare `Recraft`: a word boundary does not help here either, since "we
/// recraft your brand" uses it as a standalone ordinary verb -- the whole word matches, not a
/// substring inside one. It can come back once there is a distinctive multi-word or domain form
/// to key on instead (e.g. "Recraft AI", "recraft.ai").
static GENERATORS: &[&str] = &[
    "Midjourney",
    "DALL-E",
    "DALL·E",
    "Stable Diffusion",
    "AUTOMATIC1111",
    "ComfyUI",
    "NovelAI",
    "InvokeAI",
    "Adobe Firefly",
    "Ideogram AI",
    "Leonardo.Ai",
    "NightCafe",
    "Bing Image Creator",
    "Playground AI",
    "GPT-4o",
    "gpt-image",
    "FLUX.1",
];

/// Lowercased once at first use rather than per field per candidate: `check` runs on every
/// metadata field of every image in the tree, and rebuilding 17 short `String`s inside that loop
/// is the same waste AGENTS.md's "compile every regex in a module-scope `LazyLock`" rule exists
/// to prevent. Indices line up with `GENERATORS`, which keeps the display casing for the message.
static GENERATORS_LOWER: LazyLock<Vec<String>> =
    LazyLock::new(|| GENERATORS.iter().map(|g| g.to_ascii_lowercase()).collect());

/// A plain `.contains()` treats "medalled" as containing "dalle" and "we recraft" as containing
/// "recraft" -- both real false positives (see `GENERATORS`'s doc comment). This requires the
/// match not be flanked by an ASCII alphanumeric on either side, no regex needed: AGENTS.md warns
/// that a single Unicode `\b` in a regex sends the whole match to PikeVM on any non-ASCII input
/// (measured 2.3x slower on an 8 MB English file), so this stays a plain byte check.
/// `match_indices` (not a manual `find` + index-advance loop) is what keeps every checked index a
/// valid UTF-8 char boundary without extra bookkeeping, since `haystack` can contain a multi-byte
/// character (`DALL·E`'s `·`).
fn contains_word_bounded(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    haystack.match_indices(needle).any(|(start, matched)| {
        let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let end = start + matched.len();
        let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
        before_ok && after_ok
    })
}

/// Disjointness with SLOP045 is load-bearing, not incidental: a real ComfyUI PNG has the literal
/// string "ComfyUI" inside its own `prompt` and `workflow` JSON payloads, so without this skip
/// the same file would report both SLOP045 (the prompt chunk itself) and SLOP047 (the generator
/// name inside it) for one underlying fact. Importing `image_prompt::PROMPT_KEYS` rather than
/// re-listing those keys keeps the two panels from drifting apart (AGENTS.md allows sibling-rule
/// imports for exactly this).
///
/// Disjointness with SLOP046 (A2) is the same fix in the other direction: SLOP047 defers to
/// SLOP046 rather than the reverse, because the source-type declaration is the more specific,
/// higher-tier signal, and the generator name inside the same manifest adds no new location.
/// Confirmed on the real corpus: `c2pa_ai_assertion.png` and
/// `c2pa_ai_assertion_firefly_google.png` each reported both SLOP046 and SLOP047 on the identical
/// `caBX` field at the identical offset before this skip existed, because `Adobe Firefly` is in
/// `GENERATORS` precisely because Firefly signs C2PA manifests, and `digitalSourceType` lives in
/// exactly those manifests.
///
/// Blind spot, recorded rather than left silent (AGENTS.md: "record a rejected alternative"): a
/// zTXt chunk, or an iTXt with its compression flag set, always yields an empty
/// `MetaField::value` (image.rs adds no zlib dependency -- SLOP037/038 exist to say adding one
/// for what a few lines already avoid needing is a defect). A generator name that ships only
/// inside a *compressed* text chunk is invisible to this rule the same way it is to SLOP046.
/// Pinned by `compressed_generator_name_is_a_known_blind_spot` below.
///
/// ponytail: this reads a `printable()` extract (image.rs), so it can't tell which EXIF tag a
/// name came from -- a generator named in `ImageDescription` matches the same as one in
/// `Software`. Acceptable at Tier B; the upgrade path is parsing IFD0 and reading tag `0x0131`
/// (Software) by its actual offset. That precision would currently be a regression: a real
/// corpus file puts "Ideogram AI" in tag `0x010F` (Make), not `0x0131`, so the imprecise version
/// is the one that actually catches it.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.image else { return };
    for field in &doc.fields {
        if image_prompt::PROMPT_KEYS.contains(&field.key.as_str()) {
            continue;
        }
        let lower = field.value.to_ascii_lowercase();
        if image_source_type::declares_source_type(&lower) {
            continue;
        }
        let Some(hit) = GENERATORS_LOWER
            .iter()
            .position(|g| contains_word_bounded(&lower, g))
        else {
            continue;
        };
        let generator = GENERATORS[hit];
        let container = image_prompt::container_label(doc.format);
        let message = format!(
            "{container} `{}` (offset {}) names the generator `{}`",
            field.key, field.offset, generator
        );
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            1,
            1,
            message,
            format!(
                "strip the `{}` metadata field before shipping the image",
                field.key
            ),
        ));
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::ImageDoc;
    use crate::lang::Lang;

    const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

    fn png(chunks: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = PNG_SIG.to_vec();
        for (chunk_type, data) in chunks {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(chunk_type.as_bytes());
            out.extend_from_slice(data);
            out.extend_from_slice(&[0, 0, 0, 0]);
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

    fn diagnostics_for(bytes: &[u8]) -> Vec<Diagnostic> {
        let doc = ImageDoc::parse(bytes).unwrap();
        let ctx = LintContext {
            display_path: "test.png".to_string(),
            source: "",
            index: None,
            lang: Lang::Image,
            comments: &[],
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            image: Some(&doc),
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_generator_name_in_software_tag() {
        let bytes = png(&[
            ("tEXt", &text_chunk("Software", "Made with ComfyUI 0.3")),
            ("IEND", &[]),
        ]);
        let diags = diagnostics_for(&bytes);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("ComfyUI"));
    }

    #[test]
    fn matches_case_insensitively() {
        let bytes = png(&[
            ("tEXt", &text_chunk("Software", "rendered with midjourney")),
            ("IEND", &[]),
        ]);
        assert_eq!(diagnostics_for(&bytes).len(), 1);
    }

    /// Disjointness guard: a ComfyUI PNG's own `prompt`/`workflow` field is skipped even though
    /// its JSON literally contains "ComfyUI" -- that fact is SLOP045's to report, not this
    /// rule's too.
    #[test]
    fn skips_slop045_owned_keys_even_when_they_name_a_generator() {
        let bytes = png(&[
            (
                "tEXt",
                &text_chunk("prompt", r#"{"generator": "ComfyUI", "nodes": []}"#),
            ),
            ("IEND", &[]),
        ]);
        assert!(diagnostics_for(&bytes).is_empty());
    }

    #[test]
    fn clean_on_bare_adobe_photoshop_camera_exif() {
        let bytes = png(&[
            (
                "tEXt",
                &text_chunk("Software", "Adobe Photoshop CS2 Windows"),
            ),
            ("tEXt", &text_chunk("Make", "Canon")),
            ("IEND", &[]),
        ]);
        assert!(diagnostics_for(&bytes).is_empty());
    }

    #[test]
    fn clean_on_bare_ordinary_words_that_are_not_generator_names() {
        let bytes = png(&[
            (
                "tEXt",
                &text_chunk(
                    "Description",
                    "a imagen do por do sol na Flux Capacitor exhibit",
                ),
            ),
            ("IEND", &[]),
        ]);
        assert!(diagnostics_for(&bytes).is_empty());
    }

    /// Real false positive (B1) that a bare `.contains()` produced: "medalled" contains "dalle".
    /// `DALLE` is dropped from `GENERATORS` entirely rather than merely boundary-guarded, since
    /// `DALL-E`/`DALL·E` already cover the real spellings.
    #[test]
    fn clean_on_medalled_athlete_word_containing_dalle() {
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
        assert!(diagnostics_for(&bytes).is_empty());
    }

    /// Real false positive (B2) that a bare `.contains()` produced: "recraft" used as an ordinary
    /// verb. `Recraft` is dropped from `GENERATORS` entirely, since a word-boundary guard would
    /// not have saved it -- the whole word matches here, not a substring inside one.
    #[test]
    fn clean_on_recraft_used_as_an_ordinary_verb() {
        let bytes = png(&[
            (
                "tEXt",
                &text_chunk("Description", "we recraft your brand image every quarter"),
            ),
            ("IEND", &[]),
        ]);
        assert!(diagnostics_for(&bytes).is_empty());
    }

    /// Disjointness guard (A2): a C2PA/EXIF field can declare `digitalSourceType` and name its
    /// signing generator in the same value -- confirmed on the real corpus at an identical offset
    /// -- and SLOP046 (the more specific, higher-tier signal) owns that fact, not this rule.
    #[test]
    fn skips_field_that_declares_source_type_even_when_it_also_names_a_generator() {
        let bytes = png(&[
            (
                "caBX",
                b"digitalSourceType trainedAlgorithmicMedia claim_generator Adobe Firefly",
            ),
            ("IEND", &[]),
        ]);
        assert!(diagnostics_for(&bytes).is_empty());
    }

    /// Known limitation, not a regression (see the blind-spot doc comment on `check`): zTXt
    /// values are always empty, so a generator name that ships only inside a compressed text
    /// chunk is invisible to this rule. Pinned here so a future reader sees it as known.
    #[test]
    fn compressed_generator_name_is_a_known_blind_spot() {
        let bytes = png(&[("zTXt", &ztxt_chunk("Software")), ("IEND", &[])]);
        assert!(diagnostics_for(&bytes).is_empty());
    }
}
