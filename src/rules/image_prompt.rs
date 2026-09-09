use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::image::ImageFormat;
use crate::lang;
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP045",
    name: "Generation prompt shipped in image",
    tier: Tier::A,
    langs: lang::IMAGE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// Metadata keys that carry a full generation prompt or workflow graph verbatim: A1111's
/// `parameters`, the bare `prompt` some Stable Diffusion front ends use, ComfyUI's `workflow`,
/// and the three InvokeAI variants. Compared with `==` against the whole key, never a substring
/// match: this repo's own `assets/findings.png` carries a zTXt chunk keyed `Raw profile type
/// icc` (an ICC color profile) and a real corpus file carries one keyed `author`, and both
/// happen to contain "i" and other short fragments a loose substring panel could still snag on
/// somewhere down the line -- exact equality is what actually rules that out, not caution.
///
/// Residual risk, kept at Tier A anyway: bare `prompt` is a soft judgment call -- a "writing
/// prompt of the day" card generator could plausibly key its own text this way. It stays because
/// the surrounding mitigations are real: a PNG tEXt/iTXt keyword is only ever set deliberately by
/// the app author (never inferred, unlike a filename or a comment), the match is case-sensitive
/// (`Prompt`/`PROMPT` don't trip it), and every other key in this panel (`workflow`,
/// `sd-metadata`, the `invokeai_*` pair) is unambiguous. Same treatment as the ICC/`author`
/// exclusion above: named here so the next reader sees it was weighed, not missed.
pub static PROMPT_KEYS: &[&str] = &[
    "parameters",
    "prompt",
    "workflow",
    "sd-metadata",
    "invokeai_metadata",
    "invokeai_workflow",
];

/// Names the metadata container in every image rule's message using `doc.format`, replacing the
/// generic "image metadata field" wording: PNG and WebP metadata lives in length-prefixed RIFF
/// chunks, JPEG's in APPn segments -- different enough vocabulary that the wrong one reads as
/// wrong to anyone who has opened a hex editor on either. Shared by all three image rules
/// (SLOP045-047) rather than duplicated per file, same reasoning as `PROMPT_KEYS` being imported
/// rather than re-listed.
pub(crate) fn container_label(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png chunk",
        ImageFormat::Jpeg => "jpeg segment",
        ImageFormat::WebP => "webp chunk",
    }
}

/// One diagnostic per file: this is a file-level fact ("this image carries a prompt chunk"), and
/// a C2PA-style image can genuinely repeat the same signal in more than one field. Stops at the
/// first match in `doc.fields` order (the order the container's own chunks/segments appear in),
/// so the finding is deterministic regardless of how many prompt-shaped fields the file actually
/// carries.
///
/// The message never quotes `field.value`: the keyword alone is the tell either way. When
/// `field.compressed` is set (zTXt, or an iTXt with its compression flag on) the value is empty
/// (see `MetaField::compressed`'s doc comment in image.rs), and the message says so explicitly
/// rather than silently reading the same as an uncompressed hit.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.image else { return };
    let Some(field) = doc
        .fields
        .iter()
        .find(|f| PROMPT_KEYS.contains(&f.key.as_str()))
    else {
        return;
    };
    let container = container_label(doc.format);
    let message = if field.compressed {
        format!(
            "{container} `{}` (offset {}) ships a compressed generation-prompt field: the `{}` \
             keyword is the tell, the zlib-compressed value can't be read",
            field.key, field.offset, field.key
        )
    } else {
        format!(
            "{container} `{}` (offset {}) ships its generation prompt",
            field.key, field.offset
        )
    };
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::ImageDoc;
    use crate::lang::Lang;

    const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

    // ImageDoc::parse never validates the trailing CRC (image.rs's own doc comment says so), so
    // these unit-test PNGs use a dummy one; only tests/image_fixtures.rs needs byte-valid CRCs.
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
    fn flags_a1111_parameters_field() {
        let bytes = png(&[
            (
                "tEXt",
                &text_chunk("parameters", "a photo of a cat, steps: 20"),
            ),
            ("IEND", &[]),
        ]);
        let diags = diagnostics_for(&bytes);
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (1, 1));
        assert!(diags[0].message.contains("parameters"));
    }

    #[test]
    fn flags_compressed_ztxt_workflow_on_keyword_alone() {
        let bytes = png(&[("zTXt", &ztxt_chunk("workflow")), ("IEND", &[])]);
        let diags = diagnostics_for(&bytes);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("workflow"));
        // The value is compressed and empty; the message must not claim to quote it.
        assert!(!diags[0].message.contains("not-real-zlib-data"));
    }

    #[test]
    fn fires_once_even_with_two_prompt_shaped_fields() {
        let bytes = png(&[
            ("tEXt", &text_chunk("parameters", "steps: 20")),
            ("tEXt", &text_chunk("prompt", "a cat")),
            ("IEND", &[]),
        ]);
        assert_eq!(diagnostics_for(&bytes).len(), 1);
    }

    #[test]
    fn clean_on_icc_profile_and_author_keys() {
        let bytes = png(&[
            ("zTXt", &ztxt_chunk("Raw profile type icc")),
            ("tEXt", &text_chunk("author", "Maria Silva")),
            ("IEND", &[]),
        ]);
        assert!(diagnostics_for(&bytes).is_empty());
    }

    #[test]
    fn clean_on_bare_png_with_no_text_chunks() {
        let bytes = png(&[("IHDR", &[0; 13]), ("IEND", &[])]);
        assert!(diagnostics_for(&bytes).is_empty());
    }
}
