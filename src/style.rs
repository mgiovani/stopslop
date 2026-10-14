use crate::context::{walk_tree, NodeIndex, TextNode};
use crate::lang::{ts_language, Lang};
use crate::prose::{compute_line_starts, ProseDoc};
use std::borrow::Cow;
use std::cell::Cell;
use tree_sitter::{Node, Parser, Point, Range};

/// One `property: value` pair, wherever it was written: a CSS declaration, a `style="..."`
/// attribute, or a `style={{ ... }}` object. Offsets are absolute in the source file.
#[derive(Debug, Clone)]
pub struct Decl<'a> {
    /// Lowercase, kebab-case (`backgroundClip` in a JSX object arrives as `background-clip`).
    pub property: Cow<'a, str>,
    pub value: &'a str,
    pub value_start: usize,
    pub start: usize,
    /// Declarations written together share a block id: the CSS rule body, the attribute, or the
    /// object literal.
    pub block: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ClassToken<'a> {
    pub text: &'a str,
    pub start: usize,
}

/// Declarations and utility-class lists of a Css, Html or Tsx file, collected once per file so
/// the `design` rules never re-parse a `<style>` body or re-split a `style=` attribute.
pub struct StyleDoc<'a> {
    pub decls: Vec<Decl<'a>>,
    /// One token list per `class=` / `className=` attribute, across every string it holds.
    pub class_lists: Vec<Vec<ClassToken<'a>>>,
    source: &'a str,
    line_starts: Vec<usize>,
    /// (byte, column) of the last `line_col` answer, so ascending queries on one line count
    /// characters from there instead of from the line start (a minified stylesheet is one line).
    col_memo: Cell<(usize, usize)>,
}

impl<'a> StyleDoc<'a> {
    // ponytail: SCSS `$var:` and `@include` produce ERROR nodes that can swallow a neighbouring
    // declaration; a full SCSS grammar would recover them.
    pub fn from_css(index: &NodeIndex<'_>, source: &'a str) -> Self {
        let mut decls = Vec::new();
        collect_css_decls(index, source, &mut decls);
        Self::finish(source, decls, Vec::new(), compute_line_starts(source))
    }

    /// `<style>` bodies are re-parsed with the CSS grammar over their included ranges, so every
    /// offset stays absolute; `class=` and `style=` come from the attribute values the prose
    /// scan already collected.
    pub fn from_html(doc: &ProseDoc<'a>, source: &'a str) -> Self {
        let line_starts = compute_line_starts(source);
        let mut decls = Vec::new();
        if !doc.style_ranges.is_empty() {
            let ranges: Vec<Range> = doc
                .style_ranges
                .iter()
                .map(|&(s, e)| Range {
                    start_byte: s,
                    end_byte: e,
                    start_point: point(&line_starts, s),
                    end_point: point(&line_starts, e),
                })
                .collect();
            let mut parser = Parser::new();
            parser
                .set_language(&ts_language(Lang::Css))
                .expect("bundled css grammar");
            // Ranges come from a pre-order walk, so they are sorted and disjoint.
            let ranges_ok = parser.set_included_ranges(&ranges).is_ok();
            debug_assert!(ranges_ok, "style ranges rejected");
            if let Some(tree) = ranges_ok.then(|| parser.parse(source, None)).flatten() {
                walk_tree(&mut tree.walk(), &mut |node| {
                    if node.kind() == "declaration" {
                        push_css_decl(&node, source, &mut decls);
                    }
                    true
                });
            }
        }
        let mut class_lists = Vec::new();
        for attr in &doc.attr_values {
            html_attribute(attr, &mut decls, &mut class_lists);
        }
        Self::finish(source, decls, class_lists, line_starts)
    }

    pub fn from_jsx(index: &NodeIndex<'_>, source: &'a str, strings: &[TextNode<'a>]) -> Self {
        let mut decls = Vec::new();
        let mut class_lists = Vec::new();
        for attr in index.nodes(&["jsx_attribute"]) {
            let Some(name) = attr.named_child(0).map(|n| &source[n.byte_range()]) else {
                continue;
            };
            match name {
                "className" | "class" => {
                    let tokens = strings_within(strings, &attr)
                        .filter(|s| !has_template_syntax(s.text))
                        .flat_map(|s| class_tokens(inner(s), s.start_byte + 1))
                        .collect();
                    class_lists.push(tokens);
                }
                "style" => jsx_style_object(&attr, source, &mut decls),
                _ => {}
            }
        }
        Self::finish(source, decls, class_lists, compute_line_starts(source))
    }

    fn finish(
        source: &'a str,
        mut decls: Vec<Decl<'a>>,
        class_lists: Vec<Vec<ClassToken<'a>>>,
        line_starts: Vec<usize>,
    ) -> Self {
        decls.sort_by_key(|d| d.block);
        StyleDoc {
            decls,
            class_lists,
            source,
            line_starts,
            col_memo: Cell::new((0, 1)),
        }
    }

    /// Declarations grouped by the block they were written in.
    pub fn blocks(&self) -> impl Iterator<Item = &[Decl<'a>]> {
        self.decls.chunk_by(|a, b| a.block == b.block)
    }

    pub fn line_col(&self, byte: usize) -> (usize, usize) {
        let byte = byte.min(self.source.len());
        let idx = self.line_starts.partition_point(|&s| s <= byte) - 1;
        let line_start = self.line_starts[idx];
        let (memo_byte, memo_col) = self.col_memo.get();
        let (from, col) = if (line_start..=byte).contains(&memo_byte) {
            (memo_byte, memo_col)
        } else {
            (line_start, 1)
        };
        let col = col + self.source[from..byte].chars().count();
        self.col_memo.set((byte, col));
        (idx + 1, col)
    }
}

/// The Tailwind utility a class token names: variants (`md:hover:`) and a leading or trailing
/// important marker (`!`) stripped. Colons inside an arbitrary value (`bg-[url(a:b)]`) are not
/// variant separators.
pub fn utility(token: &str) -> &str {
    let mut depth = 0usize;
    let mut cut = 0;
    for (i, c) in token.char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => cut = i + 1,
            _ => {}
        }
    }
    token[cut..].trim_start_matches('!').trim_end_matches('!')
}

/// Splits `a: b; c: d` into (property, value, value_start, start) with offsets shifted by
/// `base`. A `;` inside parentheses or quotes (a `url(data:...;base64,...)`) does not end a
/// declaration.
fn split_declarations(text: &str, base: usize) -> Vec<(&str, &str, usize, usize)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut quote = None;
    let mut seg_start = 0;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth = depth.saturating_sub(1),
            (None, ';') if depth == 0 => {
                out.extend(declaration(&text[seg_start..i], seg_start, base));
                seg_start = i + 1;
            }
            _ => {}
        }
    }
    out.extend(declaration(&text[seg_start..], seg_start, base));
    out
}

fn declaration(seg: &str, seg_start: usize, base: usize) -> Option<(&str, &str, usize, usize)> {
    let colon = seg.find(':')?;
    let property = seg[..colon].trim();
    let raw = &seg[colon + 1..];
    let value = raw.trim();
    if property.is_empty() || value.is_empty() {
        return None;
    }
    let lead = seg[..colon].len() - seg[..colon].trim_start().len();
    let value_start = seg_start + colon + 1 + (raw.len() - raw.trim_start().len());
    Some((property, value, base + value_start, base + seg_start + lead))
}

fn collect_css_decls<'a>(index: &NodeIndex<'_>, source: &'a str, out: &mut Vec<Decl<'a>>) {
    for node in index.nodes(&["declaration"]) {
        push_css_decl(&node, source, out);
    }
}

fn push_css_decl<'a>(node: &Node, source: &'a str, out: &mut Vec<Decl<'a>>) {
    let Some(name) = node.named_child(0).filter(|n| n.kind() == "property_name") else {
        return;
    };
    let text = &source[node.byte_range()];
    let after_name = name.end_byte() - node.start_byte();
    let Some(colon) = text[after_name..].find(':') else {
        return;
    };
    let raw = &text[after_name + colon + 1..];
    let value = raw.trim().trim_end_matches(';').trim_end();
    if value.is_empty() {
        return;
    }
    let value_start =
        node.start_byte() + after_name + colon + 1 + (raw.len() - raw.trim_start().len());
    out.push(Decl {
        property: lowercase(&source[name.byte_range()]),
        value,
        value_start,
        start: node.start_byte(),
        block: node.parent().map_or(0, |p| p.start_byte()),
    });
}

/// A `class="..."` or `style="..."` attribute, given as the `name="value"` span the prose scan
/// recorded. Values holding template syntax are skipped: they are not literal CSS.
fn html_attribute<'a>(
    attr: &TextNode<'a>,
    decls: &mut Vec<Decl<'a>>,
    class_lists: &mut Vec<Vec<ClassToken<'a>>>,
) {
    let Some((name, raw)) = attr.text.split_once('=') else {
        return;
    };
    let name = name.trim();
    let is_class = name.eq_ignore_ascii_case("class");
    if !is_class && !name.eq_ignore_ascii_case("style") {
        return;
    }
    let raw = raw.trim_start();
    let quoted = raw.starts_with(['"', '\'']);
    let value = if quoted {
        raw.get(1..raw.len().saturating_sub(1)).unwrap_or_default()
    } else {
        raw
    };
    let value_start = attr.start_byte + (attr.text.len() - raw.len()) + usize::from(quoted);
    if is_class {
        class_lists.push(class_tokens(value, value_start).collect());
    } else if !has_template_syntax(value) {
        for (property, value, value_start, start) in split_declarations(value, value_start) {
            decls.push(Decl {
                property: lowercase(property),
                value,
                value_start,
                start,
                block: attr.start_byte,
            });
        }
    }
}

fn jsx_style_object<'a>(attr: &Node, source: &'a str, out: &mut Vec<Decl<'a>>) {
    let mut cursor = attr.walk();
    let Some(object) = attr
        .named_children(&mut cursor)
        .find(|n| n.kind() == "jsx_expression")
        .and_then(|e| e.named_child(0))
        .filter(|n| n.kind() == "object")
    else {
        return;
    };
    let mut pairs = object.walk();
    for pair in object
        .named_children(&mut pairs)
        .filter(|n| n.kind() == "pair")
    {
        let (Some(key), Some(value)) = (
            pair.child_by_field_name("key"),
            pair.child_by_field_name("value"),
        ) else {
            continue;
        };
        let key_text = source[key.byte_range()].trim_matches(['"', '\'']);
        let start = match value.kind() {
            "string" | "template_string" => value.start_byte() + 1,
            "number" => value.start_byte(),
            _ => continue,
        };
        let end = match value.kind() {
            "number" => value.end_byte(),
            _ => value.end_byte().saturating_sub(1),
        };
        let Some(text) = source.get(start..end) else {
            continue;
        };
        if has_template_syntax(text) {
            continue;
        }
        out.push(Decl {
            property: Cow::Owned(kebab_case(key_text)),
            value: text,
            value_start: start,
            start: pair.start_byte(),
            block: object.start_byte(),
        });
    }
}

fn kebab_case(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    // React spells -ms-transform as `msTransform`, the only prefix with a lowercase first letter.
    if camel.starts_with("ms") && camel[2..].starts_with(|c: char| c.is_ascii_uppercase()) {
        out.push('-');
    }
    for c in camel.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn lowercase(s: &str) -> Cow<'_, str> {
    if s.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(s.to_ascii_lowercase())
    } else {
        Cow::Borrowed(s)
    }
}

fn has_template_syntax(s: &str) -> bool {
    s.contains("${") || s.contains("{{")
}

fn class_tokens(text: &str, base: usize) -> impl Iterator<Item = ClassToken<'_>> {
    let mut offset = 0;
    text.split_inclusive(char::is_whitespace)
        .filter_map(move |piece| {
            let start = offset;
            offset += piece.len();
            let token = piece.trim_end();
            (!token.is_empty() && !token.contains(['{', '}', '$'])).then_some(ClassToken {
                text: token,
                start: base + start,
            })
        })
}

fn strings_within<'s, 'a>(
    strings: &'s [TextNode<'a>],
    attr: &Node,
) -> impl Iterator<Item = &'s TextNode<'a>> {
    let from = strings.partition_point(|s| s.start_byte < attr.start_byte());
    let end = attr.end_byte();
    strings[from..]
        .iter()
        .take_while(move |s| s.end_byte <= end)
}

fn inner<'a>(s: &TextNode<'a>) -> &'a str {
    s.text
        .get(1..s.text.len().saturating_sub(1))
        .unwrap_or_default()
}

fn point(line_starts: &[usize], byte: usize) -> Point {
    let row = line_starts
        .partition_point(|&s| s <= byte)
        .saturating_sub(1);
    Point {
        row,
        column: byte - line_starts[row],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::extract;

    fn css(src: &str) -> StyleDoc<'_> {
        let mut p = Parser::new();
        p.set_language(&ts_language(Lang::Css)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (_, _, index) = extract(&tree, src, Lang::Css);
        StyleDoc::from_css(&index, src)
    }

    fn html(src: &str) -> StyleDoc<'_> {
        StyleDoc::from_html(&ProseDoc::parse_html(src), src)
    }

    fn tsx(src: &str) -> StyleDoc<'_> {
        let mut p = Parser::new();
        p.set_language(&ts_language(Lang::Tsx)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (_, strings, index) = extract(&tree, src, Lang::Tsx);
        StyleDoc::from_jsx(&index, src, &strings)
    }

    fn pairs<'a>(doc: &'a StyleDoc) -> Vec<(&'a str, &'a str)> {
        doc.decls.iter().map(|d| (&*d.property, d.value)).collect()
    }

    #[test]
    fn css_declarations_carry_property_value_and_block() {
        let doc = css("a { color: red; Background: url(x;y) }\nb { margin: 0 !important; }\n");
        assert_eq!(
            pairs(&doc),
            [
                ("color", "red"),
                ("background", "url(x;y)"),
                ("margin", "0 !important")
            ]
        );
        assert_eq!(doc.blocks().count(), 2);
    }

    #[test]
    fn scss_variables_do_not_hide_plain_declarations() {
        let doc = css("$brand: #7c3aed;\n.a { color: red; .b { margin: 0; } }\n");
        let props: Vec<_> = doc.decls.iter().map(|d| &*d.property).collect();
        assert!(
            props.contains(&"color") && props.contains(&"margin"),
            "{props:?}"
        );
    }

    #[test]
    fn html_style_element_lines_map_back_to_the_page() {
        let src = "<html>\n<head>\n<style>\nh1 {\n  color: red;\n}\n</style>\n</head></html>\n";
        let doc = html(src);
        assert_eq!(pairs(&doc), [("color", "red")]);
        let d = &doc.decls[0];
        assert_eq!(&src[d.value_start..d.value_start + 3], "red");
        assert_eq!(doc.line_col(d.start), (5, 3));
    }

    #[test]
    fn html_style_attribute_and_class_list_are_split() {
        let src =
            "<p style=\"color: red; background: url(a;b)\" class=\"md:hover:p-4 !m-2\">x</p>\n";
        let doc = html(src);
        assert_eq!(pairs(&doc), [("color", "red"), ("background", "url(a;b)")]);
        let tokens: Vec<_> = doc.class_lists[0].iter().map(|t| t.text).collect();
        assert_eq!(tokens, ["md:hover:p-4", "!m-2"]);
        let t = doc.class_lists[0][1];
        assert_eq!(&src[t.start..t.start + 4], "!m-2");
    }

    #[test]
    fn html_template_syntax_in_style_is_skipped() {
        let doc = html("<p style=\"color: {{ c }}\" class=\"a {{ b }} c\">x</p>\n");
        assert!(doc.decls.is_empty());
        let tokens: Vec<_> = doc.class_lists[0].iter().map(|t| t.text).collect();
        assert_eq!(tokens, ["a", "b", "c"]);
    }

    #[test]
    fn jsx_style_object_keys_become_kebab_case() {
        let src = "const a = <p style={{ WebkitBackgroundClip: 'text', backgroundImage: \"linear-gradient(red, blue)\", opacity: 1 }} />;\n";
        let doc = tsx(src);
        assert_eq!(
            pairs(&doc),
            [
                ("-webkit-background-clip", "text"),
                ("background-image", "linear-gradient(red, blue)"),
                ("opacity", "1")
            ]
        );
        assert_eq!(doc.blocks().count(), 1);
    }

    #[test]
    fn jsx_class_lists_read_every_string_and_skip_interpolation() {
        let src = "const a = <p className={cn(\"bg-clip-text\", x && 'text-transparent', `a ${b}`)} />;\nconst b = <i className=\"p-4\" />;\n";
        let doc = tsx(src);
        let first: Vec<_> = doc.class_lists[0].iter().map(|t| t.text).collect();
        assert_eq!(first, ["bg-clip-text", "text-transparent"]);
        assert_eq!(doc.class_lists[1][0].text, "p-4");
        let t = doc.class_lists[0][0];
        assert_eq!(&src[t.start..t.start + 12], "bg-clip-text");
    }

    #[test]
    fn utility_strips_variants_and_important_but_not_arbitrary_values() {
        assert_eq!(utility("md:hover:from-purple-500"), "from-purple-500");
        assert_eq!(utility("!bg-clip-text"), "bg-clip-text");
        assert_eq!(utility("bg-clip-text!"), "bg-clip-text");
        assert_eq!(utility("bg-[url(a:b)]"), "bg-[url(a:b)]");
    }

    #[test]
    fn split_declarations_reports_absolute_offsets() {
        let text = " color : red ;margin:0";
        let got = split_declarations(text, 100);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], ("color", "red", 109, 101));
        assert_eq!(got[1], ("margin", "0", 121, 114));
    }

    #[test]
    fn spaces_around_the_equals_sign_do_not_drop_the_attribute() {
        let doc = html("<p class = \"a b\" style = 'color: red'>x</p>\n");
        let tokens: Vec<_> = doc.class_lists[0].iter().map(|t| t.text).collect();
        assert_eq!(tokens, ["a", "b"]);
        assert_eq!(pairs(&doc), [("color", "red")]);
    }

    #[test]
    fn jsx_template_string_values_point_at_the_value() {
        let src = "const a = <p style={{ color: `#7c3aed` }} />;\n";
        let doc = tsx(src);
        let d = &doc.decls[0];
        assert_eq!(&src[d.value_start..d.value_start + 7], "#7c3aed");
    }

    #[test]
    fn empty_and_malformed_input_yields_no_declarations() {
        assert!(css("").decls.is_empty());
        assert!(css("a { color: ; }").decls.is_empty());
        assert!(html("").decls.is_empty());
        assert!(
            html("<style>a { color: red</style><p style=\"::;\" class=\"\">")
                .decls
                .len()
                <= 1
        );
        assert!(tsx("const a = <p style={{ color: x }} className=")
            .decls
            .is_empty());
    }

    #[test]
    fn line_col_counts_characters_after_non_ascii_text_and_across_crlf() {
        let src = "a { /* \u{e9}\u{e9} */ color: red;\r\n  margin: 0 }\r\n";
        let doc = css(src);
        let color = doc.decls.iter().find(|d| d.property == "color").unwrap();
        let margin = doc.decls.iter().find(|d| d.property == "margin").unwrap();
        assert_eq!(doc.line_col(color.start), (1, 14));
        assert_eq!(doc.line_col(margin.start), (2, 3));
        assert_eq!(doc.line_col(color.start), (1, 14));
    }

    #[test]
    fn ms_prefixed_jsx_keys_keep_their_dash() {
        assert_eq!(kebab_case("msTransform"), "-ms-transform");
        assert_eq!(kebab_case("margin"), "margin");
    }

    #[test]
    fn scss_line_comments_are_extracted_for_suppression() {
        let src = "// ai-slop-ignore SLOP053\n  // second\na { color: red }\n";
        let mut p = Parser::new();
        p.set_language(&ts_language(Lang::Css)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, _, _) = extract(&tree, src, Lang::Css);
        let found: Vec<_> = comments.iter().map(|c| (c.line, c.col, c.text)).collect();
        assert!(found.contains(&(1, 1, "// ai-slop-ignore SLOP053")));
        assert!(found.contains(&(2, 3, "// second")));
    }
}
