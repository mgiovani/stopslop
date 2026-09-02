use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use crate::suppress::comment_body;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;
use tree_sitter::Node;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP042",
    name: "Comment that restates the code",
    tier: Tier::B,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    default_on: true,
    path_gated: true,
    check,
};

const MSG: &str = "comment restates the code it annotates";
const FIX: &str = "delete it or say why instead; if the name it restates is unclear, rename that";

// Word panels live here per project convention (bulky per-rule lists stay with their one
// consumer rather than in a shared file -- see prose_words.rs's own doc comment).
const STOPWORDS_RAW: &str = "the a an this that these those it its to of for in on at by with \
from as and or is are be was were been we our you your here now then into onto up down out \
over all each every any some also just will can do does done has have had which what \
via per current given one two s t";

static STOPWORDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| STOPWORDS_RAW.split_whitespace().collect());

// Words that only name the construct the code already shows -- a comment made entirely of these
// restates nothing on its own (see the coherence check in `evaluate_block`). Only ever excuses a
// single-line anchor: a comment over a multi-line block describes content we can't see, so there
// every content word must be literally in the code, lexicon or not.
const CODE_VERBS_RAW: &str = "set get init initialize initialise setup loop iterate return parse \
check call invoke create make build define declare assign update increment decrement add append \
push pop remove delete insert compute calculate convert cast print log read write open close load \
save store fetch send receive handle process run execute start stop import export include require \
extract wrap unwrap clone copy move allocate free release reset clear flush validate verify test \
try catch throw raise await yield spawn lock unlock acquire sleep wait retry skip use new variable \
var function func fn method class struct enum type field property prop param parameter arg \
argument array list vector vec map dict dictionary hash string str int integer number bool boolean \
flag object instance pointer reference ref value result error err exception index key element item \
entry iterator callback closure lambda constant const default empty null nil none true false \
length len size count sum total max min name file path data input output";

static CODE_VERBS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| CODE_VERBS_RAW.split_whitespace().collect());

// Condition/time-framing words (if/when/while/...) and citation/emphasis markers: a comment
// carrying one of these is explaining *when* or *why*, not just re-describing the statement.
const WHY_MARKERS_RAW: &str = "because since so thus hence therefore otherwise if when while once \
whenever during where avoid avoids workaround hack todo fixme xxx note nb safety see cf eg ie \
http https www issue bug cve rfc not no never only unless before after first last but except \
until must should always still yet instead rather without dont doesnt cant wont isnt important \
critical careful subtle beware warning caution danger gotcha tricky intentional intentionally \
deliberate deliberately purpose temporary legacy deprecated compat compatibility";

static WHY_MARKERS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| WHY_MARKERS_RAW.split_whitespace().collect());

const PRAGMA_PREFIXES: &[&str] = &[
    "type:",
    "noqa",
    "pylint",
    "flake8",
    "mypy",
    "pyright",
    "ruff",
    "isort",
    "fmt:",
    "rustfmt",
    "clippy",
    "eslint",
    "prettier",
    "@ts-",
    "ts-",
    "biome",
    "nolint",
    "go:",
    "nosec",
    "pragma",
    "ai-slop-ignore",
    "#!",
    "-*-",
    "coverage",
    "istanbul",
    "safety:",
    "lint:",
];

const REGEX_MARKERS: &[&str] = &[
    "Regex::new",
    "re.compile",
    "regexp.",
    "new RegExp",
    " = /",
    "(/",
    "r\"",
    "r#\"",
    "r'",
];

// Clean Code's "Clarification" comment category translates a cryptic literal or call into
// readable form, and by construction reuses its words (`// { int sys_dup2(int from, int to); }`,
// `// Y3 := Y3 + t0`, `// name "..." type`). A comment body carrying source-level punctuation --
// quotes included, e.g. `/* compiler flag, add "-c" */` -- is that kind of translation, not a
// restatement, so it's excluded outright.
const CLARIFICATION_SYMBOLS: &[char] = &[
    '=', '<', '>', '+', '*', '{', '}', '[', ']', '|', '&', '^', '%', '/', '\\', '~', '$', '@', '(',
    ')', '"', '\'', '`',
];

// A banner/section-divider line (`// --- Process traffic ---`, `### Encoding table`) isn't
// describing the statement below it, it's a heading; skip on sight.
const BANNER_PREFIXES: &[char] = &['-', '=', '*', '#', '~', '_', '+', '|'];

static URL_OR_ISSUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://|www\.|#\d+").unwrap());

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    ctx.walk(|node| {
        if !is_comment_kind(ctx.lang, node.kind()) {
            return;
        }
        if is_doc_comment_text(ctx.lang, ctx.node_text(&node)) {
            return;
        }
        // Godoc mandates a top-level comment on every exported identifier, so it's
        // redundant-but-undeletable rather than slop.
        if ctx.lang == Lang::Go
            && node
                .parent()
                .map(|p| p.kind() == "source_file")
                .unwrap_or(false)
        {
            return;
        }
        if is_continuation(ctx, node) {
            return; // consumed by the block head when that node was visited
        }
        if let Some((line, col)) = evaluate_block(ctx, node) {
            out.push(Diagnostic::at_fix(rule, ctx, line, col, MSG, FIX));
        }
    });
}

fn evaluate_block(ctx: &LintContext, head: Node) -> Option<(usize, usize)> {
    let (tail, body) = build_block(ctx, head);
    let (anchor, anchor_is_multiline) = find_anchor(ctx, head, tail)?;
    let raw_head = ctx.node_text(&head).trim_start().to_lowercase();
    if should_skip(&body, &anchor, &raw_head) || is_attribute_assignment(&anchor) {
        return None;
    }

    let code_tokens: HashSet<String> = tokenize(&anchor).into_iter().collect();
    let content_words: Vec<String> = tokenize(&body)
        .into_iter()
        .filter(|w| !STOPWORDS.contains(w.as_str()))
        .collect();
    // A one-word "comment" is a section label over a block whose extent we can't see, not a
    // restatement of the single statement we anchored to.
    if content_words.len() < 2 {
        return None;
    }

    // A multi-line anchor (an if/for/match/fn/class block) hides its body from us, so only its
    // header line already saying everything the comment says counts as redundant -- no lexicon
    // excuse for words the header doesn't actually contain.
    let all_explained = content_words.iter().all(|w| {
        code_tokens.contains(w.as_str())
            || (!anchor_is_multiline && CODE_VERBS.contains(w.as_str()))
    });
    let matched = content_words
        .iter()
        .filter(|w| code_tokens.contains(w.as_str()))
        .count();
    // Steidl/Hummel/Jurgens' "trivial comment" threshold: c_coeff > 0.5, i.e. at least half the
    // content words are literally reused from the code, not just excused by the CODE_VERBS
    // lexicon (a lone shared noun like "type" or "file" doesn't make a block-summary redundant).
    let coherent = matched * 2 >= content_words.len();

    (all_explained && coherent).then(|| ctx.pos(&head))
}

/// Merges `head` with any consecutive leading comment lines that continue it, returning the
/// last node in that chain plus the merged, normalized body text.
fn build_block<'a>(ctx: &LintContext<'a>, head: Node<'a>) -> (Node<'a>, String) {
    let mut tail = head;
    let mut parts = vec![normalize_body(ctx.node_text(&tail))];
    while is_leading(ctx, tail) {
        let Some(next) = tail.next_named_sibling() else {
            break;
        };
        if !is_comment_kind(ctx.lang, next.kind())
            || next.start_position().row != tail.end_position().row + 1
        {
            break;
        }
        let next_text = ctx.node_text(&next);
        if is_doc_comment_text(ctx.lang, next_text) {
            break;
        }
        parts.push(normalize_body(next_text));
        tail = next;
    }
    (tail, parts.join(" "))
}

/// A comment is a continuation (already absorbed by an earlier block head) when it directly
/// follows another comment that itself opened its own line.
fn is_continuation(ctx: &LintContext, node: Node) -> bool {
    let Some(prev) = node.prev_named_sibling() else {
        return false;
    };
    is_comment_kind(ctx.lang, prev.kind())
        && prev.end_position().row + 1 == node.start_position().row
        && is_leading(ctx, prev)
}

/// Anchors the block to exactly one statement: trailing (code before it, same row) or leading
/// (the statement starting the row right after the block). `None` when neither applies. The
/// bool says whether the anchor spans more than one source row.
fn find_anchor(ctx: &LintContext, head: Node, tail: Node) -> Option<(String, bool)> {
    let is_trailing_position = head.prev_named_sibling().is_some_and(|prev| {
        !is_comment_kind(ctx.lang, prev.kind())
            && prev.end_position().row == head.start_position().row
    });
    if is_trailing_position {
        // Trailing comments outside a function body are definition docs by convention in all
        // four languages (a struct field, a top-level const, a Python class attribute), the
        // same undeletable shape as godoc -- so only trailing comments actually inside a
        // function/method body count.
        let in_body = head.parent().is_some_and(|p| {
            matches!(p.kind(), "block" | "statement_block")
                && !(ctx.lang == Lang::Python
                    && p.parent().is_some_and(|gp| gp.kind() == "class_definition"))
        });
        if !in_body {
            return None;
        }
        let line_start = line_start_byte(ctx.source, head.start_byte());
        return Some((ctx.source[line_start..head.start_byte()].to_string(), false));
    }
    if !is_leading(ctx, head) {
        // Shares its row with code but isn't glued to a real trailing statement (e.g.
        // `} else { /* ... */`) -- it annotates that row, never falls through to the next one.
        return None;
    }

    let raw_next = tail.next_named_sibling()?;
    if is_comment_kind(ctx.lang, raw_next.kind())
        || raw_next.start_position().row != tail.end_position().row + 1
    {
        return None; // another comment, or a blank line -> section header, not an anchor
    }
    let next = first_real_statement(raw_next);
    if !is_statement_like(next.kind()) {
        return None; // struct field, match arm, attribute, ... -- not a statement to restate
    }
    if is_definition_doc_anchor(ctx.lang, next) {
        return None; // a const/module-level/class-attribute declaration is a doc by convention
    }
    let first_line = ctx.node_text(&next).split('\n').next()?;
    // A decorator/attribute line is about the item under it, not the decorator itself (Python
    // folds `@deco` into the def's own node, so this is a text check, not a kind check).
    if first_line.starts_with('@') || first_line.starts_with("#[") || first_line.starts_with("#!") {
        return None;
    }
    let multiline = next.end_position().row > next.start_position().row;
    Some((first_line.to_string(), multiline))
}

/// `self.size = 0  # file size` in an `__init__` (or `this.x = ...` in a constructor) is the
/// attribute's documentation by convention (Sphinx even reads `#:` there), not a restatement.
fn is_attribute_assignment(code: &str) -> bool {
    let code = code.trim_start();
    (code.starts_with("self.") || code.starts_with("this."))
        && code
            .find('=')
            .is_some_and(|i| !matches!(code.as_bytes().get(i + 1), Some(b'=')))
}

/// Struct fields, table/dict entries, match arms, enum variants, and attributes aren't the kind
/// of statement a comment restates -- only real statements/declarations/definitions/items count.
fn is_statement_like(kind: &str) -> bool {
    !matches!(
        kind,
        "attribute_item" | "inner_attribute_item" | "field_declaration" | "public_field_definition"
    ) && (kind.ends_with("_statement")
        || kind.ends_with("_declaration")
        || kind.ends_with("_definition")
        || kind.ends_with("_item"))
}

/// Declarations that carry documentation by convention rather than restating code: Rust
/// consts/statics, Python module- and class-level assignments, TS/TSX top-level declarations.
fn is_definition_doc_anchor(lang: Lang, node: Node) -> bool {
    match lang {
        Lang::Rust => matches!(node.kind(), "const_item" | "static_item"),
        Lang::Python => {
            node.kind() == "expression_statement"
                && node.parent().is_some_and(|p| {
                    p.kind() == "module"
                        || (p.kind() == "block"
                            && p.parent().is_some_and(|gp| gp.kind() == "class_definition"))
                })
        }
        Lang::Ts | Lang::Tsx => {
            matches!(
                node.kind(),
                "lexical_declaration" | "variable_declaration" | "export_statement"
            ) && node.parent().is_some_and(|p| p.kind() == "program")
        }
        Lang::Go | Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => false,
    }
}

/// A comment that's the very first thing in a block can end up as the sibling of a wrapper node
/// (Python's `block`, Go's `statement_list`) instead of the real statement inside it, because
/// tree-sitter can't make it the first token of a body that starts with an actual statement. The
/// wrapper's first named child always starts at the exact same position, so drilling into it
/// doesn't change which source line "the anchor" is.
fn first_real_statement(node: Node) -> Node {
    let mut n = node;
    while !is_statement_like(n.kind()) {
        let mut c = n.walk();
        let Some(first_child) = n.named_children(&mut c).next() else {
            break;
        };
        if first_child.start_position() != n.start_position() {
            break;
        }
        n = first_child;
    }
    n
}

fn should_skip(body: &str, anchor: &str, raw_head: &str) -> bool {
    let body = body.trim();
    if body.ends_with('?') {
        return true;
    }
    // A single typed identifier ("MAX_DATA", "Expr::Async") is a label, not a sentence -- even
    // though camel/snake splitting later turns it into 2+ content tokens.
    if body.split_whitespace().count() < 2 {
        return true;
    }
    let words = words_of(body);
    if words.len() > 12 || !words.iter().any(|w| w.chars().any(|c| c.is_alphabetic())) {
        return true;
    }
    if body.starts_with(BANNER_PREFIXES) {
        return true;
    }
    if body.contains(CLARIFICATION_SYMBOLS) {
        return true;
    }
    if PRAGMA_PREFIXES
        .iter()
        .any(|p| body.starts_with(p) || raw_head.starts_with(p))
    {
        return true;
    }
    if words.iter().any(|w| WHY_MARKERS.contains(w)) {
        return true;
    }
    if body.contains("n't") || URL_OR_ISSUE_RE.is_match(body) {
        return true;
    }
    is_symbol_dense(anchor) || REGEX_MARKERS.iter().any(|m| anchor.contains(m))
}

fn is_comment_kind(lang: Lang, kind: &str) -> bool {
    match lang {
        Lang::Rust => kind == "line_comment" || kind == "block_comment",
        Lang::Ts | Lang::Tsx | Lang::Python | Lang::Go => kind == "comment",
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => false,
    }
}

/// Same test as `context::is_doc_comment`, matched directly on node text.
fn is_doc_comment_text(lang: Lang, text: &str) -> bool {
    match lang {
        Lang::Ts | Lang::Tsx => text.starts_with("/**"),
        Lang::Rust => {
            text.starts_with("///")
                || text.starts_with("//!")
                || text.starts_with("/**")
                || text.starts_with("/*!")
        }
        Lang::Python | Lang::Go | Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => false,
    }
}

/// True when nothing but whitespace precedes `node` on its own start row (i.e. it isn't a
/// trailing comment glued to code).
fn is_leading(ctx: &LintContext, node: Node) -> bool {
    let line_start = line_start_byte(ctx.source, node.start_byte());
    ctx.source[line_start..node.start_byte()].trim().is_empty()
}

fn line_start_byte(source: &str, byte: usize) -> usize {
    source[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn is_symbol_dense(line: &str) -> bool {
    let non_ws: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    if non_ws.is_empty() {
        return false;
    }
    let symbols = non_ws
        .iter()
        .filter(|c| !(c.is_ascii_alphanumeric() || **c == '_'))
        .count();
    symbols * 2 > non_ws.len()
}

/// Strips the comment delimiter (via `suppress::comment_body`), and for block comments also
/// the closing `*/` and any per-line leading `*`, then lowercases.
fn normalize_body(raw: &str) -> String {
    let body = comment_body(raw);
    if !raw.trim_start().starts_with("/*") {
        return body.trim().to_lowercase();
    }
    let body = body.strip_suffix("*/").unwrap_or(body);
    body.lines()
        .map(|l| {
            let l = l.trim();
            l.strip_prefix('*').map_or(l, str::trim)
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Plain word split (no camel-case or plural normalization) for the exclusion checks in
/// `should_skip`, which need to see literal typed words like "avoids" or "does".
fn words_of(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Split on non-alphanumerics, split camelCase, lowercase, strip plurals. Shared by both sides
/// of the comparison (comment body and anchor code line) so identifiers compare equal regardless
/// of naming convention.
fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .flat_map(split_camel)
        .map(|t| depluralize(&t.to_lowercase()))
        .collect()
}

fn split_camel(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut tokens = Vec::new();
    let mut cur = String::new();
    for i in 0..chars.len() {
        let c = chars[i];
        if !cur.is_empty() {
            let prev = chars[i - 1];
            let boundary = (prev.is_lowercase() || prev.is_ascii_digit()) && c.is_uppercase()
                || prev.is_uppercase()
                    && c.is_uppercase()
                    && i + 1 < chars.len()
                    && chars[i + 1].is_lowercase();
            if boundary {
                tokens.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

fn depluralize(w: &str) -> String {
    if let Some(stem) = w.strip_suffix("ies") {
        return format!("{stem}y");
    }
    if w.len() > 3
        && w.ends_with('s')
        && !w.ends_with("ss")
        && !w.ends_with("us")
        && !w.ends_with("is")
    {
        return w[..w.len() - 1].to_string();
    }
    w.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use crate::lang::ts_language;
    use tree_sitter::Parser;

    fn run(lang: Lang, src: &str) -> Vec<Diagnostic> {
        let mut parser = Parser::new();
        parser.set_language(&ts_language(lang)).unwrap();
        let tree = parser.parse(src, None).unwrap();
        let (comments, strings) = context::extract(&tree, src, lang);
        let ctx = LintContext {
            display_path: "test".into(),
            source: src,
            tree: Some(&tree),
            lang,
            comments: &comments,
            strings: &strings,
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn rust_canonical_hit() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // increment the counter\n    counter += 1;\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn python_canonical_hit() {
        assert_eq!(
            run(
                Lang::Python,
                "def f():\n    # increment the counter\n    counter += 1"
            )
            .len(),
            1
        );
    }

    #[test]
    fn ts_canonical_hit() {
        assert_eq!(
            run(
                Lang::Ts,
                "function f() {\n  // increment the counter\n  counter += 1;\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn go_canonical_hit() {
        assert_eq!(
            run(
                Lang::Go,
                "package main\nfunc f() {\n\t// increment the counter\n\tcounter += 1\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn trailing_comment_hit() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    counter += 1; // increment the counter\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn consecutive_line_block_merges_into_one_finding() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // increment\n    // the counter\n    counter += 1;\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn load_the_state_still_flags() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f(x: i32) {\n    // Load the state\n    let mut state = State::load(x, y);\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn rust_doc_comment_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    /// increment the counter\n    counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn ts_doc_comment_skipped() {
        assert!(run(
            Lang::Ts,
            "function f() {\n  /** increment the counter */\n  counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn go_top_level_comment_skipped() {
        assert!(run(
            Lang::Go,
            "package main\n\n// increment the counter\nfunc f() {\n\tcounter += 1\n}"
        )
        .is_empty());
    }

    #[test]
    fn blank_line_between_comment_and_code_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // increment the counter\n\n    counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn interrogative_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // increment the counter?\n    counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn pragma_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // noqa increment counter\n    counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn why_marker_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // retry because upstream returns 503\n    retry();\n}"
        )
        .is_empty());
    }

    #[test]
    fn condition_framing_word_skipped() {
        assert!(run(
            Lang::Go,
            "package main\nfunc f(targs []int) {\n\tif len(targs) > 0 {\n\t\t// Add the type arguments if this is an instance.\n\t\tuse(targs)\n\t}\n}"
        )
        .is_empty());
    }

    #[test]
    fn single_word_comment_skipped() {
        assert!(run(
            Lang::Go,
            "package main\nfunc f(chroot *string) {\n\t// Chroot\n\tif chroot != nil {\n\t}\n}"
        )
        .is_empty());
    }

    #[test]
    fn clarification_with_new_vocabulary_kept_unflagged() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // 86_400 == one day in seconds\n    sleep(86_400);\n}"
        )
        .is_empty());
    }

    #[test]
    fn clarification_with_symbols_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // counter = counter + 1\n    counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn over_twelve_words_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // this here totally completely absolutely definitely certainly plainly simply just merely increment the counter\n    counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn regex_anchor_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // matches the pattern\n    let re = Regex::new(r\"^\\d+$\").unwrap();\n}"
        )
        .is_empty());
    }

    #[test]
    fn camel_snake_and_plural_normalization_flags() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // parses the configs\n    parse_config(path);\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn lexicon_only_comment_not_flagged() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // initialize\n    let x = Foo::new();\n}"
        )
        .is_empty());
    }

    #[test]
    fn coherence_ratio_below_half_not_flagged() {
        assert!(run(
            Lang::Rust,
            "fn f(list: &mut Vec<i32>, x: i32) {\n    // build the argument list here\n    list.push(x);\n}"
        )
        .is_empty());
    }

    #[test]
    fn leading_anchor_must_be_statement_like() {
        assert!(run(
            Lang::Rust,
            "fn outer() {\n    // allow dead code here always\n    #[allow(dead_code)]\n    fn inner() {}\n}"
        )
        .is_empty());
    }

    #[test]
    fn trailing_comment_outside_function_body_skipped() {
        assert!(run(
            Lang::Go,
            "package main\n\ntype T struct {\n\tState int // the state field\n}"
        )
        .is_empty());
    }

    #[test]
    fn rule_is_path_gated() {
        assert!(RULE.path_gated);
    }

    /// Coordinator round 3, #1: a multi-line anchor (a whole `if` block) hides its body, so the
    /// CODE_VERBS lexicon can't excuse "print" -- only the header's literal words count.
    #[test]
    fn multiline_anchor_gets_no_lexicon_excuse() {
        assert!(run(
            Lang::Go,
            "package main\nfunc f(x *int) {\n\t// print x\n\tif x == nil {\n\t\treturn\n\t}\n}"
        )
        .is_empty());
    }

    /// Coordinator round 3, #2: one typed identifier, however it splits on case/underscore, is
    /// a label, not a sentence.
    #[test]
    fn single_typed_word_skipped() {
        assert!(run(Lang::Rust, "fn f() {\n    // MAX_DATA\n    max_data();\n}").is_empty());
    }

    /// Coordinator round 3, #3: a quoted literal is a Clean Code clarification, not a
    /// restatement, even though the quoted word matches the code.
    #[test]
    fn quoted_clarification_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // \"path\" \"name\" \"value\"\n    let path_name_value = 1;\n}"
        )
        .is_empty());
    }

    /// Coordinator round 3, #4: a banner/section divider is a heading, not a description.
    #[test]
    fn banner_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // --- increment counter ---\n    counter += 1;\n}"
        )
        .is_empty());
    }

    /// Coordinator round 3, #5 (leading): a `const` is documentation by convention.
    #[test]
    fn const_item_anchor_skipped() {
        assert!(run(
            Lang::Rust,
            "// Clock types\npub const CLOCK_REALTIME: i32 = 0;"
        )
        .is_empty());
    }

    /// Coordinator round 3, #5 (trailing): a Python class attribute's trailing comment is a
    /// doc by convention, the same undeletable shape as a struct field's.
    #[test]
    fn python_class_attribute_trailing_doc_skipped() {
        assert!(run(
            Lang::Python,
            "class TarFile:\n    extraction_filter = None    # the default filter for extraction"
        )
        .is_empty());
    }

    /// Coordinator round 3, #6: a comment glued to a keyword line (`} else { /* ... */`) is
    /// neither a clean trailing comment nor a leading one -- it must not fall through and
    /// anchor to the next statement.
    #[test]
    fn comment_glued_to_keyword_line_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f(x: i32) {\n    if x > 0 {\n        y();\n    } else { /* compiler flag, add the c option */\n        z();\n    }\n}"
        )
        .is_empty());
    }

    #[test]
    fn tokenize_splits_camel_case() {
        assert_eq!(tokenize("parseConfig"), vec!["parse", "config"]);
        assert_eq!(tokenize("HTTPServer"), vec!["http", "server"]);
    }

    #[test]
    fn tokenize_splits_snake_case_and_strips_plurals() {
        assert_eq!(tokenize("parse_configs"), vec!["parse", "config"]);
        assert_eq!(tokenize("i"), vec!["i"]);
        assert_eq!(tokenize("class"), vec!["class"]); // "ss" ending is not a plural
    }

    #[test]
    fn struct_field_doc_not_flagged() {
        assert!(run(
            Lang::Go,
            "package main\ntype Func struct {\n\t// Parent of a closure\n\tClosureParent *Func\n}"
        )
        .is_empty());
        assert!(run(
            Lang::Rust,
            "struct Hello {\n    // A random value for the inner hello.\n    inner_hello_random: Random,\n}"
        )
        .is_empty());
    }

    #[test]
    fn attribute_assignment_doc_not_flagged() {
        assert!(run(
            Lang::Python,
            "class T:\n    def __init__(self):\n        self.size = 0  # file size\n        # The list of completions\n        self.completions = None\n"
        )
        .is_empty());
        assert!(run(
            Lang::Ts,
            "class T {\n  constructor() {\n    // the user id\n    this.userId = 0;\n  }\n}"
        )
        .is_empty());
        assert_eq!(
            run(
                Lang::Python,
                "def f(self):\n    # check the sizes\n    self.size == other.size\n"
            )
            .len(),
            1
        );
    }

    #[test]
    fn is_symbol_dense_detects_dense_lines() {
        assert!(is_symbol_dense("x = a+b*c-d/e%f^g&h|i&&j==k;"));
        assert!(!is_symbol_dense("counter += 1;"));
    }
}
