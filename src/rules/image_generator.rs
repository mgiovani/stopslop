use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang;
use crate::registry::RuleDef;
use crate::rules::image_prompt;
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
/// "ComfyUI" in a metadata value for any other reason.
///
/// Deliberately excluded, each a real false positive: bare `Adobe`, `Photoshop`, `Canon`,
/// `Nikon`, `Sony` (every retouched or camera-original photo carries these in EXIF -- verified
/// against a real Canon EOS 10D JPEG whose EXIF reads "Adobe Photoshop CS2 Windows"; only the
/// full "Adobe Firefly" is distinctive enough to flag), and bare `Firefly`, `Imagen`, `Flux`,
/// `Grok`, `Aurora`, `Gemini` (ordinary words; `Imagen` is Portuguese for "image", and this
/// crate lints pt-BR prose).
static GENERATORS: &[&str] = &[
    "Midjourney",
    "DALL-E",
    "DALL·E",
    "DALLE",
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
    "Recraft",
    "GPT-4o",
    "gpt-image",
    "FLUX.1",
];

/// Lowercased once at first use rather than per field per candidate: `check` runs on every
/// metadata field of every image in the tree, and rebuilding 19 short `String`s inside that loop
/// is the same waste AGENTS.md's "compile every regex in a module-scope `LazyLock`" rule exists
/// to prevent. Indices line up with `GENERATORS`, which keeps the display casing for the message.
static GENERATORS_LOWER: LazyLock<Vec<String>> =
    LazyLock::new(|| GENERATORS.iter().map(|g| g.to_ascii_lowercase()).collect());

/// Disjointness with SLOP045 is load-bearing, not incidental: a real ComfyUI PNG has the literal
/// string "ComfyUI" inside its own `prompt` and `workflow` JSON payloads, so without this skip
/// the same file would report both SLOP045 (the prompt chunk itself) and SLOP047 (the generator
/// name inside it) for one underlying fact. Importing `image_prompt::PROMPT_KEYS` rather than
/// re-listing those keys keeps the two panels from drifting apart (AGENTS.md allows sibling-rule
/// imports for exactly this).
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
        let Some(hit) = GENERATORS_LOWER
            .iter()
            .position(|g| lower.contains(g.as_str()))
        else {
            continue;
        };
        let generator = GENERATORS[hit];
        let message = format!(
            "image metadata field `{}` (offset {}) names the generator `{}`",
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
}
