use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang;
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP046",
    name: "Declared AI source type",
    tier: Tier::A,
    langs: lang::IMAGE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// IPTC's digital-source-type vocabulary carries two AI-related terms: `trainedAlgorithmicMedia`
/// (fully generated) and `compositeWithTrainedAlgorithmicMedia` (an AI-assisted edit of a real
/// photograph). One case-insensitive needle, the shorter term, catches both, because the longer
/// one contains it as a substring differing only in case -- there is no way to match one without
/// also matching the other, so the check below distinguishes them after the fact rather than
/// running two separate scans.
///
/// This is deliberately one rule, not two, even though the two terms mean opposite things (fully
/// synthetic vs. AI-touched original). The value can surface in an XMP packet, a legacy IPTC
/// block, or inside a C2PA manifest's CBOR assertion -- `printable()` (image.rs) extracts it as a
/// plain substring regardless of which container it came from -- and a single real file (a C2PA
/// manifest plus an XMP sidecar packet saying the same thing) carries it in more than one of
/// those containers at once. Splitting by container would double-report that one fact and break
/// the panel-disjointness invariant (AGENTS.md); keying on the value itself does not.
///
/// Deliberately NOT flagged: C2PA manifest *presence* on its own. A Leica M11-P or Sony Alpha
/// body signs every frame it takes with a `caBX` chunk (PNG) or an APP11 segment (JPEG) carrying
/// real camera provenance, so keying on that chunk/segment existing would flag every photo shot
/// on either camera. Only this vocabulary value distinguishes an AI-authored manifest from a
/// camera-authored one, which is why the rule reads field *values* and never the presence of a
/// C2PA container by itself.
const NEEDLE: &str = "trainedalgorithmicmedia";
const COMPOSITE_NEEDLE: &str = "compositewithtrainedalgorithmicmedia";

/// One diagnostic per file, first match in `doc.fields` order -- same rationale as SLOP045: this
/// is a file-level fact, and a real file can repeat it across an XMP packet and a C2PA manifest
/// at once.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.image else { return };
    // Lowercased once and carried out of the search: a `MetaField` value runs to
    // `MAX_FIELD_BYTES`, so re-lowercasing the match to pick the message would copy it twice.
    let Some((field, lower)) = doc.fields.iter().find_map(|f| {
        let lower = f.value.to_ascii_lowercase();
        lower.contains(NEEDLE).then_some((f, lower))
    }) else {
        return;
    };
    let message = if lower.contains(COMPOSITE_NEEDLE) {
        format!(
            "image metadata field `{}` (offset {}) declares digitalSourceType \
             compositeWithTrainedAlgorithmicMedia: an AI-assisted edit of a real photograph",
            field.key, field.offset
        )
    } else {
        format!(
            "image metadata field `{}` (offset {}) declares digitalSourceType \
             trainedAlgorithmicMedia: the image is fully AI-generated",
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
    fn flags_plain_trained_algorithmic_media_as_fully_generated() {
        let xmp = "iptcExt:DigitalSourceType=\"http://cv.iptc.org/newscodes/\
                   digitalsourcetype/trainedAlgorithmicMedia\"";
        let bytes = png(&[
            ("tEXt", &text_chunk("XML:com.adobe.xmp", xmp)),
            ("IEND", &[]),
        ]);
        let diags = diagnostics_for(&bytes);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("fully AI-generated"));
        assert!(!diags[0].message.contains("edit"));
    }

    #[test]
    fn flags_composite_variant_as_ai_assisted_edit() {
        let xmp = "iptcExt:DigitalSourceType=\"http://cv.iptc.org/newscodes/\
                   digitalsourcetype/compositeWithTrainedAlgorithmicMedia\"";
        let bytes = png(&[
            ("tEXt", &text_chunk("XML:com.adobe.xmp", xmp)),
            ("IEND", &[]),
        ]);
        let diags = diagnostics_for(&bytes);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("AI-assisted edit"));
    }

    #[test]
    fn matches_case_insensitively() {
        let bytes = png(&[
            (
                "tEXt",
                &text_chunk("XML:com.adobe.xmp", "TRAINEDALGORITHMICMEDIA"),
            ),
            ("IEND", &[]),
        ]);
        assert_eq!(diagnostics_for(&bytes).len(), 1);
    }

    /// The deliberate false-positive guard: a C2PA-shaped manifest that names real camera
    /// provenance and nothing from the AI vocabulary must stay silent, or the rule would flag
    /// every Leica/Sony frame.
    #[test]
    fn clean_on_c2pa_camera_provenance_with_no_ai_vocabulary() {
        let bytes = png(&[
            (
                "caBX",
                b"c2pa.actions claim_generator: Leica M11-P firmware 1.2",
            ),
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
