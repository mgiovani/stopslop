use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, ALL_NATLANGS};
use crate::prose::{compute_line_col, compute_line_starts, decode_entity};
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP057",
    name: "Decorative separator glyph",
    tier: Tier::B,
    langs: &[Lang::Tsx, Lang::Html],
    natlangs: ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// One separator in a breadcrumb or footer is ordinary; the tell is the habit of repeating it.
const MIN_MATCHES: usize = 2;

const SEPARATORS: [char; 3] = ['\u{B7}', '\u{2022}', '\u{2192}'];

/// Longest entity this rule decodes (`&#x2192;`), so a stray `&` never scans far for its `;`.
const MAX_ENTITY_BYTES: usize = 10;

static LITERAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[·•→]").unwrap());

/// A JavaScript string literal decodes escapes; JSX text and attribute strings decode entities.
static ESCAPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\u([0-9a-fA-F]{4})|\\u\{([0-9a-fA-F]{1,6})\}|\\x([0-9a-fA-F]{2})").unwrap()
});

static ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&#?[0-9A-Za-z]{1,8};").unwrap());

/// Reports each spaced separator glyph once the file holds `MIN_MATCHES` of them, anchored at the
/// glyph. A glyph that opens its line separates nothing, so it stays quiet.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let glyphs = match ctx.lang {
        Lang::Html => html_glyphs(ctx),
        _ => tsx_glyphs(ctx),
    };
    if glyphs.len() < MIN_MATCHES {
        return;
    }
    let line_starts = compute_line_starts(ctx.source);
    for (byte, glyph) in glyphs {
        let (line, col) = compute_line_col(&line_starts, ctx.source, byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            format!(
                "decorative `{glyph}` separator between tokens; use layout (gap, separate elements) or a comma"
            ),
        ));
    }
}

type Candidate = (usize, usize, Option<char>);

fn tsx_glyphs(ctx: &LintContext) -> Vec<(usize, char)> {
    let mut candidates: Vec<Candidate> = Vec::new();
    for s in ctx.strings {
        candidates.extend(literals(s.text, s.start_byte));
        candidates.extend(decoded(&ESCAPE, s.text, s.start_byte, decode_escape));
    }
    for n in ctx.nodes(&["jsx_text"]) {
        candidates.extend(literals(ctx.node_text(&n), n.start_byte()));
    }
    for n in ctx.nodes(&["html_character_reference"]) {
        candidates.push((
            n.start_byte(),
            n.end_byte(),
            decode_entity(ctx.node_text(&n)),
        ));
    }
    accepted(ctx.source, candidates)
}

fn html_glyphs(ctx: &LintContext) -> Vec<(usize, char)> {
    let Some(doc) = ctx.prose else {
        return Vec::new();
    };
    // `masked` keeps comment text for the rules that read it, so literal glyphs need the filter.
    let mut candidates: Vec<Candidate> = literals(&doc.masked, 0)
        .filter(|&(from, ..)| {
            !doc.comments
                .iter()
                .any(|k| (k.start_byte..k.end_byte).contains(&from))
        })
        .collect();
    for attr in &doc.attr_values {
        candidates.extend(literals(attr.text, attr.start_byte));
        candidates.extend(decoded(&ENTITY, attr.text, attr.start_byte, decode_entity));
    }
    for &(byte, glyph) in &doc.entities {
        let len = ctx.source.as_bytes()[byte..]
            .iter()
            .take(MAX_ENTITY_BYTES)
            .position(|&b| b == b';')
            .map_or(1, |i| i + 1);
        candidates.push((byte, byte + len, Some(glyph)));
    }
    accepted(ctx.source, candidates)
}

/// Separator glyphs among the candidates that sit between two tokens, deduped by byte because
/// string literals nest and an attribute value overlaps the masked stream.
fn accepted(source: &str, candidates: Vec<Candidate>) -> Vec<(usize, char)> {
    let found: BTreeSet<(usize, char)> = candidates
        .into_iter()
        .filter_map(|(from, to, glyph)| {
            let glyph = glyph.filter(|g| SEPARATORS.contains(g))?;
            flanked(source, from, to).then_some((from, glyph))
        })
        .collect();
    found.into_iter().collect()
}

fn literals(text: &str, offset: usize) -> impl Iterator<Item = Candidate> + '_ {
    LITERAL.find_iter(text).map(move |m| {
        (
            offset + m.start(),
            offset + m.end(),
            m.as_str().chars().next(),
        )
    })
}

fn decoded<'a>(
    re: &'a Regex,
    text: &'a str,
    offset: usize,
    decode: fn(&str) -> Option<char>,
) -> impl Iterator<Item = Candidate> + 'a {
    re.find_iter(text)
        .map(move |m| (offset + m.start(), offset + m.end(), decode(m.as_str())))
}

fn decode_escape(escape: &str) -> Option<char> {
    let hex = escape
        .trim_start_matches(['\\', 'u', 'x'])
        .trim_matches(['{', '}']);
    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
}

/// A space or NBSP on each side, then a non-space char: `a · b`, never `a·b` or a line-leading `→ b`.
fn flanked(source: &str, start: usize, end: usize) -> bool {
    let gap = |c: char| c == ' ' || c == '\u{A0}';
    let before = source[..start].strip_suffix(gap);
    let after = source[end..].strip_prefix(gap);
    matches!((before, after), (Some(b), Some(a))
        if b.chars().next_back().is_some_and(|c| !c.is_whitespace())
            && a.chars().next().is_some_and(|c| !c.is_whitespace()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::extract;
    use crate::lang::ts_language;
    use crate::prose::ProseDoc;

    fn diagnostics_for(lang: Lang, src: &str) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        if lang == Lang::Html {
            let doc = ProseDoc::parse_html(src);
            let ctx = context(lang, src, &[], None, Some(&doc));
            check(&RULE, &ctx, &mut out);
            return out;
        }
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ts_language(lang)).unwrap();
        let tree = parser.parse(src, None).unwrap();
        let (comments, strings, index) = extract(&tree, src, lang);
        let mut ctx = context(lang, src, &strings, Some(&index), None);
        ctx.comments = &comments;
        check(&RULE, &ctx, &mut out);
        out
    }

    fn context<'a>(
        lang: Lang,
        src: &'a str,
        strings: &'a [crate::context::TextNode<'a>],
        index: Option<&'a crate::context::NodeIndex<'a>>,
        prose: Option<&'a ProseDoc<'a>>,
    ) -> LintContext<'a> {
        LintContext {
            display_path: "t".into(),
            source: src,
            index,
            lang,
            comments: &[],
            strings,
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose,
            image: None,
            style: None,
            natlangs: ALL_NATLANGS,
        }
    }

    fn tsx(src: &str) -> Vec<Diagnostic> {
        diagnostics_for(Lang::Tsx, src)
    }

    fn html(src: &str) -> Vec<Diagnostic> {
        diagnostics_for(Lang::Html, src)
    }

    #[test]
    fn flags_jsx_text_and_counts_a_chain() {
        let d = tsx("const a = <p>Home · Docs · Blog</p>;\n");
        assert_eq!(d.iter().map(|d| d.col).collect::<Vec<_>>(), [19, 26]);
    }

    #[test]
    fn flags_string_literals_and_escapes() {
        let d = tsx("const a = \"a • b\";\nconst b = `c \\u00b7 d`;\n");
        assert_eq!(d.len(), 2);
        assert_eq!(tsx("const a = \"a → b\", b = 'c → d';\n").len(), 2);
    }

    #[test]
    fn a_single_occurrence_is_quiet() {
        assert!(tsx("const a = <p>Home · Docs</p>;\n").is_empty());
    }

    #[test]
    fn comments_and_code_do_not_count() {
        let src = "// a · b · c\n/* d → e → f */\nconst x = a > b ? 1 : 2;\n";
        assert!(tsx(src).is_empty());
    }

    #[test]
    fn tight_and_one_sided_glyphs_are_quiet() {
        assert!(tsx("const a = [\"DALL·E\", \"a ·b\", \"c· d\", \"e→f\"];\n").is_empty());
    }

    #[test]
    fn html_text_attribute_and_nbsp_forms_are_flagged() {
        let src = "<p>a \u{2022} b</p>\n<p title=\"c \u{2192} d\">e\u{A0}\u{B7}\u{A0}f</p>\n";
        assert_eq!(html(src).len(), 3);
    }

    #[test]
    fn html_entities_are_flagged() {
        let src = "<nav><a>A</a> &middot; <a>B</a> &#183; <a>C</a> &#xb7; <a>D</a> &bull; <a>E</a> &#8226; <a>F</a> &rarr; <a>G</a> &#8594; <a>H</a></nav>\n";
        assert_eq!(html(src).len(), 7);
        let d = html("<p>a &middot; b &middot; c</p>\n");
        assert_eq!((d[0].line, d[0].col), (1, 6));
    }

    #[test]
    fn html_script_style_and_comments_are_quiet() {
        let src = "<script>var a = \"b \u{B7} c \u{B7} d\";</script>\n<style>a::after { content: \"x \u{2192} y \u{2192} z\"; }</style>\n<!-- e \u{2022} f \u{2022} g -->\n";
        assert!(html(src).is_empty());
    }

    #[test]
    fn a_line_leading_marker_is_not_ours() {
        assert!(html("<p>\n\u{2192} first\n\u{2192} second\n\u{2192} third\n</p>\n").is_empty());
        assert!(html("<p>x</p>\n  \u{2022} first\n  \u{2022} second\n").is_empty());
    }

    #[test]
    fn jsx_entities_are_decoded_in_text_and_attributes() {
        let src =
            "const a = <p title=\"x &middot; y\">x &middot; y &#XB7; z &#xB7; w &bull; v</p>;\n";
        assert_eq!(tsx(src).len(), 5);
    }

    #[test]
    fn a_plain_string_does_not_decode_entities() {
        assert!(tsx("const b = \"p &middot; q &middot; r &#183; s\";\n").is_empty());
    }

    #[test]
    fn jsx_text_does_not_decode_escapes() {
        assert!(tsx("const a = <p>a \\u00b7 b \\u00b7 c</p>;\n").is_empty());
    }

    #[test]
    fn escapes_match_in_any_hex_case() {
        let src = "const a = [\"a \\u00B7 b\", \"c \\xB7 d\", \"e \\u{B7} f\", `g \\u{2022} h`, \"i \\u2192 j\"];\n";
        assert_eq!(tsx(src).len(), 5);
        assert!(tsx("const a = [\"a \\u00b8 b\", \"c \\u0041 d\"];\n").is_empty());
    }

    #[test]
    fn a_jsx_comment_is_quiet() {
        assert!(tsx("const a = <p>{/* a \u{B7} b \u{B7} c */}</p>;\n").is_empty());
    }

    #[test]
    fn html_attribute_entities_are_flagged() {
        let src = "<a title=\"x &middot; y &#xB7; z\">k</a>\n";
        assert_eq!(html(src).len(), 2);
    }

    #[test]
    fn html_comment_entities_are_quiet() {
        assert!(html("<!-- a &middot; b &bull; c -->\n<p x=\"1\">y</p>\n").is_empty());
    }

    #[test]
    fn html_preformatted_code_is_quiet() {
        let src = "<pre>a \u{B7} b</pre><code>c \u{B7} d</code><textarea>e \u{B7} f</textarea>\n";
        assert!(html(src).is_empty());
    }

    #[test]
    fn a_trailing_glyph_with_nothing_after_it_is_quiet() {
        assert!(tsx("const a = <p>a \u{B7} b \u{B7}</p>; // \u{B7}").is_empty());
        assert_eq!(html("<p>a \u{B7} b \u{B7} c</p> &middot;").len(), 2);
    }
}
