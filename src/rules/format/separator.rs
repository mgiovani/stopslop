use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, ALL_NATLANGS};
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

/// JSX text decodes entities and string literals decode escapes, so both spellings are the glyph.
static TSX_GLYPH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"[·•→]|&(?:middot|bull|rarr|#183|#xb7|#8226|#x2022|#8594|#x2192);|\\u(?:00b7|2022|2192|\{(?:b7|2022|2192)\})|\\xb7",
    )
    .unwrap()
});

/// Reports each spaced separator glyph once the file holds `MIN_MATCHES` of them, anchored at the
/// glyph. A glyph that opens its line has no token before it and is SLOP021's marker, not ours.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let glyphs = match ctx.lang {
        Lang::Html => html_glyphs(ctx),
        _ => tsx_glyphs(ctx),
    };
    if glyphs.len() < MIN_MATCHES {
        return;
    }
    for (byte, glyph) in glyphs {
        let (line, col) = line_col(ctx.source, byte);
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

fn tsx_glyphs(ctx: &LintContext) -> Vec<(usize, char)> {
    let mut found = BTreeSet::new();
    let strings = ctx.strings.iter().map(|s| (s.start_byte, s.end_byte));
    let jsx_text = ctx
        .nodes(&["jsx_text"])
        .into_iter()
        .map(|n| (n.start_byte(), n.end_byte()));
    // `strings` nests (a template substitution holds its own strings), so the set dedupes by byte.
    for (start, end) in strings.chain(jsx_text) {
        for m in TSX_GLYPH.find_iter(&ctx.source[start..end]) {
            let (from, to) = (start + m.start(), start + m.end());
            if flanked(ctx.source, from, to) {
                found.insert((from, glyph_of(m.as_str())));
            }
        }
    }
    found.into_iter().collect()
}

fn html_glyphs(ctx: &LintContext) -> Vec<(usize, char)> {
    let Some(doc) = ctx.prose else {
        return Vec::new();
    };
    let literal = |text: &str, offset: usize| -> Vec<(usize, char)> {
        text.char_indices()
            .filter(|&(_, c)| SEPARATORS.contains(&c))
            .map(|(i, c)| (offset + i, c))
            .collect()
    };
    let mut candidates = literal(&doc.masked, 0);
    for attr in &doc.attr_values {
        candidates.extend(literal(attr.text, attr.start_byte));
    }
    candidates.extend(
        doc.entities
            .iter()
            .copied()
            .filter(|&(_, c)| SEPARATORS.contains(&c)),
    );
    candidates.retain(|&(byte, _)| {
        !doc.comments
            .iter()
            .any(|k| (k.start_byte..k.end_byte).contains(&byte))
    });
    candidates.retain(|&(byte, c)| flanked(ctx.source, byte, glyph_end(ctx.source, byte, c)));
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

/// End byte of the glyph at `byte`: the entity runs to its `;`, a literal glyph is one char.
fn glyph_end(source: &str, byte: usize, glyph: char) -> usize {
    if source[byte..].starts_with('&') {
        source[byte..].find(';').map_or(byte + 1, |i| byte + i + 1)
    } else {
        byte + glyph.len_utf8()
    }
}

fn glyph_of(matched: &str) -> char {
    match matched {
        "\u{B7}" | "&middot;" | "&#183;" | "&#xb7;" | "\\u00b7" | "\\u{b7}" | "\\xb7" => '\u{B7}',
        "\u{2022}" | "&bull;" | "&#8226;" | "&#x2022;" | "\\u2022" | "\\u{2022}" => '\u{2022}',
        _ => '\u{2192}',
    }
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

fn line_col(source: &str, byte: usize) -> (usize, usize) {
    let head = &source[..byte];
    let line = head.matches('\n').count() + 1;
    let col = head
        .rfind('\n')
        .map_or(head, |i| &head[i + 1..])
        .chars()
        .count()
        + 1;
    (line, col)
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
}
