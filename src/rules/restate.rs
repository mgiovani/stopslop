use crate::context::{self, LintContext};
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, CODE_LANGS};
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
    langs: CODE_LANGS,
    default_on: true,
    path_gated: true,
    check,
};

const MSG: &str = "comment restates the code it annotates";
const FIX: &str = "delete it or say why instead; if the name it restates is unclear, rename that";

// Bulky per-rule word lists stay with their one consumer rather than a shared file, per project
// convention (see prose_words.rs's doc comment). The three panels are stemmed at build time via
// `stem`, the same normalization `tokenize` applies to both sides.
const STOPWORDS_RAW: &str = "the a an this that these those it its to of for in on at by with \
from as and or is are be was were been we our you your here now then into onto up down out \
over all each every any some also just will can do does done has have had which what \
via per current given one two s t";

static STOPWORDS: LazyLock<HashSet<String>> = LazyLock::new(|| stemmed(STOPWORDS_RAW));

// Per issue #6's predicate, verbs naming only the operation the code shows are dropped before
// the subset test: `// parse and validate` over `parse(x)` restates despite having no vocabulary
// of its own, and never count against the coherence ratio.
const CODE_VERBS_RAW: &str = "set get init initialize initialise setup loop iterate return parse \
check call invoke create make build define declare assign update increment decrement add append \
push pop remove delete insert compute calculate convert cast print log read write open close load \
save store fetch send receive handle process run execute start stop import export include require \
extract wrap unwrap clone copy move allocate free release reset clear flush validate verify test \
try catch throw raise await yield spawn lock unlock acquire sleep wait retry skip use";

static CODE_VERBS: LazyLock<HashSet<String>> = LazyLock::new(|| stemmed(CODE_VERBS_RAW));

// The CODE_VERBS subset naming the compound construct itself. A block comment can describe body
// content we can't see, so `// print x` over `if x == nil {` isn't excused the way it would be
// over a bare `print(x)`.
const CONSTRUCT_VERBS_RAW: &str = "loop iterate check handle try define declare";

static CONSTRUCT_VERBS: LazyLock<HashSet<String>> = LazyLock::new(|| stemmed(CONSTRUCT_VERBS_RAW));

// Nouns that name a construct or type the statement visibly is (`// create a new list` over
// `Vec::new()`). Explained, but unlike the verbs they stay in the coherence denominator: a
// comment can't become redundant on construct nouns alone.
const CODE_NOUNS_RAW: &str = "new variable var function func fn method class struct enum type \
field property prop param parameter arg argument array list vector vec map dict dictionary hash \
string str int integer number bool boolean flag object instance pointer reference ref value result \
error err exception index key element item entry iterator callback closure lambda constant const \
default empty null nil none true false length len size count sum total max min name file path data \
input output";

static CODE_NOUNS: LazyLock<HashSet<String>> = LazyLock::new(|| stemmed(CODE_NOUNS_RAW));

// Issue #6's escape hatches (why/constraint/emphasis/citation markers). Condition words
// (if/when/while) are deliberately excluded: `// retry when locked` over `if locked { retry() }`
// still restates, and including them was the single largest recall loss measured.
const WHY_MARKERS_RAW: &str = "because since so thus hence therefore otherwise avoid avoids \
workaround hack todo fixme xxx safety see cf http https www issue bug cve rfc not no never only \
unless before after first last but except until must should always still yet instead rather \
without dont doesnt cant wont isnt important critical careful subtle beware warning caution \
danger gotcha tricky intentional intentionally deliberate deliberately purpose temporary legacy \
deprecated compat compatibility";

static WHY_MARKERS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| WHY_MARKERS_RAW.split_whitespace().collect());

fn stemmed(raw: &str) -> HashSet<String> {
    raw.split_whitespace().map(stem).collect()
}

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

// Clean Code's "Clarification" comments translate cryptic code and reuse its words by design --
// source punctuation, quotes included, marks that and is excluded outright. Parens, slashes, and
// intra-word apostrophes are ordinary prose and don't count; see `has_quote_apostrophe`.
const CLARIFICATION_SYMBOLS: &[char] = &[
    '=', '<', '>', '+', '*', '{', '}', '[', ']', '|', '&', '^', '%', '\\', '~', '$', '@', '"', '`',
];

// A banner/section-divider line (`// --- Process traffic ---`, `### Encoding table`) isn't
// describing the statement below it, it's a heading; skip on sight.
const BANNER_PREFIXES: &[char] = &['-', '=', '*', '#', '~', '_', '+', '|'];

static URL_OR_ISSUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://|www\.|#\d+").unwrap());

// Includes Python compound statements because tree-sitter-python hoists a body-opening comment
// out of `block`, making it that statement's sibling rather than its child. Kinds a grammar
// lacks simply resolve to nothing.
const STATEMENT_CONTAINERS: &[&str] = &[
    "source_file",
    "module",
    "program",
    "block",
    "statement_list",
    "statement_block",
    "declaration_list",
    "field_declaration_list",
    "class_body",
    "interface_body",
    "switch_case",
    "switch_default",
    "expression_case",
    "default_case",
    "type_case",
    "communication_case",
    "function_definition",
    "class_definition",
    "if_statement",
    "elif_clause",
    "else_clause",
    "for_statement",
    "while_statement",
    "try_statement",
    "except_clause",
    "finally_clause",
    "with_statement",
    "match_statement",
    "case_clause",
];

/// Scans each container's children ONCE from a collected `Vec`. Never `next_named_sibling`,
/// `prev_named_sibling` or `parent`: each costs O(index-in-parent) in tree-sitter, which turned
/// a run of n sibling comments into O(n^2). A comment block's span `[i, j]` is found by
/// extending `j` forward; the loop then jumps straight to `j + 1`.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for parent in ctx.nodes(STATEMENT_CONTAINERS) {
        // Godoc mandates a comment on every exported identifier at file scope, so any comment
        // there is redundant-but-undeletable rather than slop -- skip the whole scope at once.
        if ctx.lang == Lang::Go && parent.kind() == "source_file" {
            continue;
        }
        let scope = Scope {
            kind: parent.kind(),
            python_class_body: ctx.lang == Lang::Python
                && (parent.kind() == "class_definition"
                    || (parent.kind() == "block"
                        && parent
                            .parent()
                            .is_some_and(|gp| gp.kind() == "class_definition"))),
        };
        let mut cursor = parent.walk();
        let kids: Vec<Node> = parent.named_children(&mut cursor).collect();
        let mut i = 0;
        while i < kids.len() {
            if !is_comment_kind(ctx.lang, kids[i].kind())
                || context::is_doc_comment(ctx.lang, ctx.node_text(&kids[i]))
            {
                i += 1;
                continue;
            }
            let mut j = i;
            while j + 1 < kids.len() {
                let next = kids[j + 1];
                let continues = is_comment_kind(ctx.lang, next.kind())
                    && next.start_position().row == kids[j].end_position().row + 1
                    && !context::is_doc_comment(ctx.lang, ctx.node_text(&next))
                    && is_leading(ctx, kids[j])
                    && is_leading(ctx, next);
                if !continues {
                    break;
                }
                j += 1;
            }
            if let Some((line, col)) = evaluate_block(ctx, &kids, i, j, scope) {
                out.push(Diagnostic::at_fix(rule, ctx, line, col, MSG, FIX));
            }
            i = j + 1;
        }
    }
}

/// What `check` already knows about the container being scanned. Threaded down so the anchor
/// logic never calls `Node::parent()`, which is O(index-in-parent) and so quadratic over a run
/// of trailing comments (each one is its own block).
#[derive(Clone, Copy)]
struct Scope<'a> {
    kind: &'a str,
    python_class_body: bool,
}

fn evaluate_block(
    ctx: &LintContext,
    kids: &[Node],
    i: usize,
    j: usize,
    scope: Scope,
) -> Option<(usize, usize)> {
    let lines: Vec<String> = kids[i..=j]
        .iter()
        .map(|k| normalize_body(ctx.node_text(k)))
        .collect();
    // A bare `//` line inside the block (`//\n// Receiving frames\n//`) is banner framing.
    if lines.iter().any(String::is_empty) {
        return None;
    }
    let body_raw = lines.join(" ");
    let body = body_raw.to_lowercase();
    let (anchor, anchor_is_compound) = find_anchor(ctx, kids, i, j, scope)?;
    let raw_head = ctx.node_text(&kids[i]).trim_start().to_lowercase();
    if should_skip(&body, &anchor, &raw_head) || names_other_identifier(&body_raw, &anchor) {
        return None;
    }
    // An attribute-assignment comment is doc by convention (Sphinx reads `#:` there) only for a
    // noun phrase; an imperative ("set the size") forfeits the exemption. Only the first word
    // decides, so "the return status" keeps it.
    let imperative = tokenize(&body)
        .first()
        .is_some_and(|w| CODE_VERBS.contains(w));
    if is_attribute_assignment(&anchor) && !imperative {
        return None;
    }

    let code_tokens: HashSet<String> = tokenize(&anchor).into_iter().collect();
    let content_words: Vec<String> = tokenize(&body)
        .into_iter()
        .filter(|w| !STOPWORDS.contains(w.as_str()))
        .collect();
    // A one-word comment over a compound block is a section label (body unseen); over a single
    // statement it's Steidl's "<=2 words" case -- their survey voted to delete it 70% of the
    // time, so it's kept flaggable.
    if content_words.is_empty() || (anchor_is_compound && content_words.len() < 2) {
        return None;
    }

    let literal = |w: &String| code_tokens.contains(w.as_str());
    let excused = |w: &String| {
        CODE_NOUNS.contains(w.as_str())
            || if anchor_is_compound {
                &*CONSTRUCT_VERBS
            } else {
                &*CODE_VERBS
            }
            .contains(w.as_str())
    };
    let all_explained = content_words.iter().all(|w| literal(w) || excused(w));
    let matched = content_words.iter().filter(|w| literal(w)).count();
    // Steidl/Hummel/Jurgens' threshold: c_coeff > 0.5, over half the words reused from code,
    // flags it. Excused verbs drop from the denominator per issue #6; excused nouns don't.
    let denominator = content_words
        .iter()
        .filter(|w| literal(w) || !CODE_VERBS.contains(w.as_str()))
        .count();
    let coherent = matched * 2 > denominator;

    (all_explained && coherent).then(|| ctx.pos(&kids[i]))
}

/// Anchors the block `kids[i..=j]` to exactly one statement: trailing (code before it, same row)
/// or leading (the statement starting the row right after the block). `None` when neither
/// applies. The bool says the anchor is a compound statement (its header opens a body we can't
/// see, so only that header line is returned); a multi-line simple statement is returned whole.
fn find_anchor(
    ctx: &LintContext,
    kids: &[Node],
    i: usize,
    j: usize,
    scope: Scope,
) -> Option<(String, bool)> {
    let head = kids[i];
    let tail = kids[j];
    let is_trailing_position = i > 0 && {
        let prev = kids[i - 1];
        !is_comment_kind(ctx.lang, prev.kind())
            && prev.end_position().row == head.start_position().row
    };
    if is_trailing_position {
        // A trailing comment on a Go struct field or Python class attribute is that member's
        // doc by convention (godoc; Sphinx) -- undeletable. Rust/TS have `///`/`/** */` for
        // that, so their plain `//` fields are judged normally.
        let in_body = match scope.kind {
            "block" | "statement_block" | "statement_list" => !scope.python_class_body,
            "field_declaration_list" => ctx.lang == Lang::Rust,
            "class_body" | "interface_body" => matches!(ctx.lang, Lang::Ts | Lang::Tsx),
            _ => false,
        };
        if !in_body {
            return None;
        }
        // The actual previous statement's own last line, not a slice of the raw physical row --
        // a row can carry more than one statement (`parse(x); increment(y); // parse x` anchors
        // to `increment(y);` alone, never to the whole line).
        let prev_text = ctx.node_text(&kids[i - 1]);
        return Some((prev_text.rsplit('\n').next()?.to_string(), false));
    }
    if !is_leading(ctx, head) {
        // Shares its row with code but isn't glued to a real trailing statement (e.g.
        // `} else { /* ... */`) -- it annotates that row, never falls through to the next one.
        return None;
    }

    let raw_next = *kids.get(j + 1)?;
    if is_comment_kind(ctx.lang, raw_next.kind())
        || raw_next.start_position().row != tail.end_position().row + 1
    {
        return None; // another comment, or a blank line -> section header, not an anchor
    }
    let next = first_real_statement(ctx.lang, raw_next);
    if !is_statement_like(ctx.lang, next.kind()) {
        return None; // Go struct field, match arm, attribute, ... -- not a statement to restate
    }
    if is_definition_doc_anchor(ctx.lang, next, scope) {
        return None; // a const/module-level/class-attribute declaration is a doc by convention
    }
    // `// Exception types` over the first of several consts on consecutive lines labels the
    // run, not that one declaration.
    let heads_a_run = DECLARATION_KINDS.contains(&next.kind())
        && kids.get(j + 2).is_some_and(|sib| {
            sib.kind() == next.kind() && sib.start_position().row == next.end_position().row + 1
        });
    if heads_a_run {
        return None;
    }
    let text = ctx.node_text(&next);
    let first_line = text.split('\n').next()?;
    // A decorator/attribute line is about the item under it, not the decorator itself (Python
    // folds `@deco` into the def's own node, so this is a text check, not a kind check).
    if first_line.starts_with('@') || first_line.starts_with("#[") || first_line.starts_with("#!") {
        return None;
    }
    // A block header is judged alone since its body is invisible; a >10-row "simple" statement
    // gets the same treatment -- full text would let its vocabulary trivially explain any comment.
    let rows = next.end_position().row - next.start_position().row;
    let compound = rows > 0 && (rows > 10 || first_line.trim_end().ends_with(['{', ':']));
    let anchor = if compound { first_line } else { text };
    Some((anchor.to_string(), compound))
}

const DECLARATION_KINDS: &[&str] = &[
    "const_item",
    "static_item",
    "field_declaration",
    "public_field_definition",
    "property_signature",
];

/// A word shaped like an identifier (`F_ADD_SEALS`, `parseConfig`, `MTU`) is a reference; when
/// the anchor doesn't carry it whole, the comment points at something else and its tokens
/// matching the code piecewise (`f`, `add`, `seal`) is coincidence.
fn names_other_identifier(body_raw: &str, anchor: &str) -> bool {
    let anchor_idents: Vec<Vec<String>> = anchor
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty())
        .map(ident_tokens)
        .collect();
    body_raw
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_'))
        .filter(|w| w.len() > 1 && looks_like_identifier(w))
        .map(ident_tokens)
        .filter(|needle| !needle.is_empty())
        .any(|needle| {
            !anchor_idents.iter().any(|toks| {
                toks.windows(needle.len())
                    .any(|win| win == needle.as_slice())
            })
        })
}

/// Token-boundary presence, not substring: `TAG` is not "in" `voltage`, but `MTU` is in
/// `lost_mtu_probe` and `parseConfig` is in `parse_config`.
fn ident_tokens(ident: &str) -> Vec<String> {
    ident
        .split('_')
        .filter(|p| !p.is_empty())
        .flat_map(split_camel)
        .map(|t| t.to_lowercase())
        .collect()
}

fn looks_like_identifier(w: &str) -> bool {
    let all_caps = w.len() >= 3
        && w.chars().any(|c| c.is_ascii_alphabetic())
        && w.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    let mixed = w.chars().any(|c| c.is_ascii_lowercase())
        && w.chars().skip(1).any(|c| c.is_ascii_uppercase());
    w.contains('_') || all_caps || mixed
}

/// `self.size = 0  # file size` in an `__init__` (or `this.x = ...` in a constructor) is the
/// attribute's documentation by convention (Sphinx even reads `#:` there), not a restatement.
/// Walks past the attribute name instead of scanning for the first `=`, so `self.count -= 1` and
/// `self.size != other.size` (both contain a bare `=` that isn't an assignment) aren't exempted.
fn is_attribute_assignment(code: &str) -> bool {
    let code = code.trim_start();
    let Some(rest) = code
        .strip_prefix("self.")
        .or_else(|| code.strip_prefix("this."))
    else {
        return false;
    };
    let after_name =
        rest.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    let mut chars = after_name.trim_start().chars();
    matches!(chars.next(), Some('=')) && !matches!(chars.next(), Some('='))
}

/// Struct fields, table/dict entries, match arms, enum variants, and attributes aren't the kind
/// of statement a comment restates -- only real statements/declarations/definitions/items count.
/// A Go struct field's leading comment is its godoc; Rust and TS fields are deletable-comment
/// territory like any statement (their doc syntax is `///` / `/** */`, which `check` skips).
fn is_statement_like(lang: Lang, kind: &str) -> bool {
    match kind {
        "attribute_item" | "inner_attribute_item" => false,
        "field_declaration" => lang == Lang::Rust,
        "public_field_definition" | "property_signature" => matches!(lang, Lang::Ts | Lang::Tsx),
        _ => {
            kind.ends_with("_statement")
                || kind.ends_with("_declaration")
                || kind.ends_with("_definition")
                || kind.ends_with("_item")
        }
    }
}

/// Python module- and class-level assignments carry their documentation as a plain `#` comment
/// by convention (there is no other syntax for it), so a restating one is undeletable, the same
/// argument as for doc comments. Rust and TS have doc syntax for consts and top-level
/// declarations; a plain `//` there is deletable and is judged like any other.
fn is_definition_doc_anchor(lang: Lang, node: Node, scope: Scope) -> bool {
    lang == Lang::Python
        && node.kind() == "expression_statement"
        && (scope.kind == "module" || scope.python_class_body)
}

/// A comment that's the very first thing in a block can end up as the sibling of a wrapper node
/// (Python's `block`, Go's `statement_list`) instead of the real statement inside it, because
/// tree-sitter can't make it the first token of a body that starts with an actual statement. The
/// wrapper's first named child always starts at the exact same position, so drilling into it
/// doesn't change which source line "the anchor" is.
fn first_real_statement(lang: Lang, node: Node) -> Node {
    let mut n = node;
    while !is_statement_like(lang, n.kind()) {
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
    if body.contains(CLARIFICATION_SYMBOLS) || body.contains("()") || has_quote_apostrophe(body) {
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
    if body.contains("n't") || body.contains("n\u{2019}t") || URL_OR_ISSUE_RE.is_match(body) {
        return true;
    }
    is_symbol_dense(anchor) || REGEX_MARKERS.iter().any(|m| anchor.contains(m))
}

/// An apostrophe that isn't wedged between two letters (`don't`, `file's`) is a quote around a
/// literal (`'x'`, `'\n'`), which makes the comment a clarification.
fn has_quote_apostrophe(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    (0..chars.len()).any(|i| {
        matches!(chars[i], '\'' | '\u{2019}')
            && !(i > 0
                && chars[i - 1].is_alphabetic()
                && i + 1 < chars.len()
                && chars[i + 1].is_alphabetic())
    })
}

fn is_comment_kind(lang: Lang, kind: &str) -> bool {
    match lang {
        Lang::Rust => kind == "line_comment" || kind == "block_comment",
        Lang::Ts | Lang::Tsx | Lang::Python | Lang::Go => kind == "comment",
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => false,
    }
}

/// True when nothing but whitespace precedes `node` on its own start row (i.e. it isn't a
/// trailing comment glued to code). `start_position().column` is a byte offset within the row
/// for UTF-8 input, so the line start is O(1) arithmetic -- no backward scan over the row.
fn is_leading(ctx: &LintContext, node: Node) -> bool {
    let line_start = node.start_byte() - node.start_position().column;
    ctx.source[line_start..node.start_byte()]
        .chars()
        .all(char::is_whitespace)
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

/// Split on non-alphanumerics, split camelCase, lowercase, stem. Shared by both sides of the
/// comparison (comment body and anchor code line) so identifiers compare equal regardless of
/// naming convention or inflection.
fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .flat_map(split_camel)
        .map(|t| stem(&t.to_lowercase()))
        .collect()
}

/// Steidl et al. match comment and code words at Levenshtein distance < 2, which is what lets
/// `parsed` meet `parse`. A suffix strip covers the same inflections (`-s`, `-ed`, `-ing`, final
/// `-e`) without the quadratic pairwise compare, and can't accidentally unify `set` with `get`.
fn stem(w: &str) -> String {
    let w = depluralize(w);
    let w = ["ing", "ed"]
        .iter()
        .find_map(|suf| w.strip_suffix(suf).filter(|s| s.len() >= 3))
        .unwrap_or(&w);
    w.strip_suffix('e')
        .filter(|s| s.len() >= 3)
        .unwrap_or(w)
        .to_string()
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
    if w.len() >= 3
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
    use crate::lang::ts_language;
    use tree_sitter::Parser;

    fn run(lang: Lang, src: &str) -> Vec<Diagnostic> {
        let mut parser = Parser::new();
        parser.set_language(&ts_language(lang)).unwrap();
        let tree = parser.parse(src, None).unwrap();
        let (comments, strings, index) = context::extract(&tree, src, lang);
        let ctx = LintContext {
            display_path: "test".into(),
            source: src,
            index: Some(&index),
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

    #[test]
    fn compound_anchor_excuses_only_construct_verbs() {
        assert!(run(
            Lang::Go,
            "package main\nfunc f(x *int) {\n\t// print x\n\tif x == nil {\n\t\treturn\n\t}\n}"
        )
        .is_empty());
        assert_eq!(
            run(
                Lang::Go,
                "package main\nfunc f(items []int) {\n\t// loop over the items\n\tfor _, item := range items {\n\t\tuse(item)\n\t}\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn single_typed_word_skipped() {
        assert!(run(Lang::Rust, "fn f() {\n    // MAX_DATA\n    max_data();\n}").is_empty());
    }

    #[test]
    fn quoted_clarification_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // \"path\" \"name\" \"value\"\n    let path_name_value = 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn banner_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // --- increment counter ---\n    counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn const_item_anchor_skipped() {
        assert!(run(
            Lang::Rust,
            "// Clock types\npub const CLOCK_REALTIME: i32 = 0;"
        )
        .is_empty());
    }

    #[test]
    fn python_class_attribute_trailing_doc_skipped() {
        assert!(run(
            Lang::Python,
            "class TarFile:\n    extraction_filter = None    # the default filter for extraction"
        )
        .is_empty());
    }

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
        assert_eq!(tokenize("parseConfig"), vec!["pars", "config"]);
        assert_eq!(tokenize("HTTPServer"), vec!["http", "server"]);
    }

    #[test]
    fn tokenize_splits_snake_case_and_strips_plurals() {
        assert_eq!(tokenize("parse_configs"), vec!["pars", "config"]);
        assert_eq!(tokenize("i"), vec!["i"]);
        assert_eq!(tokenize("class"), vec!["class"]); // "ss" ending is not a plural
    }

    /// The inflections Steidl et al.'s Levenshtein < 2 match would unify, plus `-ing`, all land
    /// on one stem; `set`/`get` (also distance 1) stay apart.
    #[test]
    fn stem_unifies_inflections_of_one_word() {
        for (a, b) in [
            ("parsed", "parse"),
            ("parsing", "parse"),
            ("matches", "matched"),
            ("entries", "entry"),
            ("sorted", "sorts"),
            ("closing", "close"),
        ] {
            assert_eq!(stem(a), stem(b), "{a} vs {b}");
        }
        assert_ne!(stem("set"), stem("get"));
        assert_ne!(stem("ring"), stem("r"));
    }

    #[test]
    fn inflected_comment_word_matches_code_identifier() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // parsed the header\n    let header = parse_header(input);\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn go_struct_field_comment_is_godoc() {
        assert!(run(
            Lang::Go,
            "package main\ntype Func struct {\n\t// Parent of a closure\n\tClosureParent *Func\n}"
        )
        .is_empty());
    }

    /// Rust and TS have `///` and `/** */` for field docs, so a plain comment there is as
    /// deletable as one in a function body; a Go field's `//` is its godoc.
    #[test]
    fn rust_and_ts_plain_field_comments_are_judged() {
        assert_eq!(
            run(
                Lang::Rust,
                "struct Hello {\n    // A random value for the inner hello.\n    inner_hello_random: Random,\n\n    count: u32, // the count\n}"
            )
            .len(),
            2
        );
        assert_eq!(
            run(
                Lang::Ts,
                "interface User {\n  // the user name\n  userName: string;\n}\nclass A {\n  // the item count\n  itemCount = 0;\n}"
            )
            .len(),
            2
        );
        assert!(run(
            Lang::Rust,
            "struct Hello {\n    /// A random value for the inner hello.\n    inner_hello_random: Random,\n}"
        )
        .is_empty());
    }

    /// Round 4, #4: `is_attribute_assignment` must not treat `-=`/`!=` (or any operator ending
    /// in `=`) as a plain assignment -- only a lone `=` after the attribute name counts.
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
                "def f(self):\n    # decrement the count\n    self.count -= 1\n"
            )
            .len(),
            1
        );
        assert_eq!(
            run(
                Lang::Python,
                "def f(self, other):\n    # check the sizes\n    self.size != other.size\n"
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

    // -- round 4 (perf + correctness review) --------------------------------------------------

    #[test]
    fn rust_and_ts_top_level_plain_comments_are_judged() {
        assert_eq!(
            run(Lang::Ts, "// the api version\nconst API_VERSION = 2;").len(),
            1
        );
        assert_eq!(
            run(
                Lang::Rust,
                "// the max retries\nconst MAX_RETRIES: u32 = 3;"
            )
            .len(),
            1
        );
        assert!(run(
            Lang::Rust,
            "/// the max retries\nconst MAX_RETRIES: u32 = 3;"
        )
        .is_empty());
    }

    #[test]
    fn attribute_assignment_doc_needs_a_noun_phrase() {
        assert!(run(
            Lang::Python,
            "class A:\n    def __init__(self, size):\n        # the file size\n        self.size = size"
        )
        .is_empty());
        assert_eq!(
            run(
                Lang::Python,
                "class A:\n    def __init__(self, size):\n        # set the size\n        self.size = size"
            )
            .len(),
            1
        );
    }

    #[test]
    fn identifier_naming_something_else_is_a_reference() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // for use with F_ADD_SEALS\n    let f_seal_grow = 4;\n}"
        )
        .is_empty());
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // the F_SEAL_GROW value\n    let f_seal_grow = 4;\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn identifier_shapes_and_token_boundary_presence() {
        assert!(looks_like_identifier("MTU"));
        assert!(looks_like_identifier("parseConfig"));
        assert!(looks_like_identifier("f_seal"));
        assert!(!looks_like_identifier("Some"));
        assert!(!looks_like_identifier("the"));
        assert!(!names_other_identifier("the MTU probe", "lost_mtu_probe()"));
        assert!(!names_other_identifier(
            "see parseConfig",
            "parse_config(x)"
        ));
        assert!(names_other_identifier("the TAG value", "let voltage = 1;"));
    }

    #[test]
    fn curly_apostrophe_counts_like_the_ascii_one() {
        let src = "fn f() {\n    // the file\u{2019}s name\n    let file_name = g();\n    // the \u{2018}x\u{2019} flag\n    let flag = x;\n    // don\u{2019}t clear the flag\n    clear(flag);\n}";
        assert_eq!(run(Lang::Rust, src).len(), 1);
    }

    #[test]
    fn attribute_doc_survives_a_verb_used_as_a_noun() {
        assert!(run(
            Lang::Python,
            "class T:\n    def __init__(self, status):\n        self.status = status  # the return status"
        )
        .is_empty());
    }

    /// Every trailing comment is its own block, so a per-block `Node::parent()` call would make
    /// this quadratic the way the leading-comment run once was.
    #[test]
    fn twenty_thousand_trailing_comments_run_fast() {
        let body: String = (0..20_000)
            .map(|i| format!("    a{i} += 1; // step {i}\n"))
            .collect();
        let src = format!("fn f() {{\n{body}}}\n");
        let start = std::time::Instant::now();
        let d = run(Lang::Rust, &src);
        assert!(d.is_empty());
        assert!(start.elapsed().as_secs() < 10, "{:?}", start.elapsed());
    }

    #[test]
    fn empty_comment_line_marks_a_banner() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    //\n    // Receiving frames\n    //\n    receiving_frames();\n}"
        )
        .is_empty());
    }

    #[test]
    fn group_header_over_a_run_of_declarations_not_flagged() {
        assert!(run(
            Lang::Rust,
            "// Exception types\nconst EXCEPT_A: ExceptionType = 0;\nconst EXCEPT_B: ExceptionType = 1;\n"
        )
        .is_empty());
        assert_eq!(
            run(
                Lang::Rust,
                "// Exception type\nconst EXCEPT_A: ExceptionType = 0;\n\nfn f() {}\n"
            )
            .len(),
            1
        );
    }

    #[test]
    fn empty_parens_mark_a_clarification() {
        assert!(run(
            Lang::Go,
            "package main\nfunc f() {\n\t// func panicrangestate()\n\tobj = NewFunc(\"panicrangestate\")\n}"
        )
        .is_empty());
    }

    #[test]
    fn python_module_level_assignment_not_flagged() {
        assert!(run(Lang::Python, "# the default config\nDEFAULT_CONFIG = {}").is_empty());
    }

    #[test]
    fn url_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // see https://docs.rs/parse\n    parse(x);\n}"
        )
        .is_empty());
    }

    #[test]
    fn issue_ref_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // parse per #123\n    parse(x);\n}"
        )
        .is_empty());
    }

    #[test]
    fn apostrophe_contraction_skipped() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // don't increment the counter\n    counter += 1;\n}"
        )
        .is_empty());
    }

    #[test]
    fn exactly_twelve_words_flags_thirteen_does_not() {
        let anchor =
            "let set_get_init_loop_check_call_create_make_build_define_declare_assign = 1;";
        assert_eq!(
            run(
                Lang::Rust,
                &format!(
                    "fn f() {{\n    // set get init loop check call create make build define declare assign\n    {anchor}\n}}"
                )
            )
            .len(),
            1
        );
        assert!(run(
            Lang::Rust,
            &format!(
                "fn f() {{\n    // set get init loop check call create make build define declare assign extra\n    {anchor}\n}}"
            )
        )
        .is_empty());
    }

    #[test]
    fn rust_block_comment_star_stripping_flags() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    /*\n     * increment the\n     * counter\n     */\n    counter += 1;\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn merged_block_diagnostic_reports_first_comment_line() {
        let diags = run(
            Lang::Rust,
            "fn f() {\n    // increment\n    // the counter\n    counter += 1;\n}",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 2);
    }

    /// Round 4, #3: the trailing anchor is the actual previous statement, never a slice of the
    /// raw physical row -- a row can carry more than one statement.
    #[test]
    fn trailing_anchor_uses_previous_statement_only() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    parse(x);\n    increment(y); // parse x\n}"
        )
        .is_empty());
        assert_eq!(
            run(Lang::Rust, "fn f() {\n    increment(y); // increment y\n}").len(),
            1
        );
    }

    /// Replaces the old `single_word_comment_skipped`, which exited on the typed-word check
    /// before ever reaching the one-this-names: 4 typed words, all but "counter" a stopword.
    #[test]
    fn one_literal_word_over_a_simple_statement_flagged() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // the an a counter\n    counter += 1;\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn one_content_word_over_a_compound_statement_is_a_label() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // the counter\n    if counter > 0 {\n        g();\n    }\n}"
        )
        .is_empty());
    }

    #[test]
    fn condition_words_do_not_excuse_a_restatement() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // retry when locked\n    retry_when_locked();\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn parentheses_and_possessive_apostrophe_are_prose() {
        let src = "fn f() {\n    // strip the prefix (leading)\n    strip_leading_prefix(x);\n    // the file's name\n    let file_name = g();\n}";
        assert_eq!(run(Lang::Rust, src).len(), 2);
    }

    #[test]
    fn quoted_literal_apostrophe_is_a_clarification() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // the 'x' flag\n    let flag = x;\n}"
        )
        .is_empty());
    }

    #[test]
    fn wrapped_simple_statement_is_judged_whole() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // sum the values\n    let total: u32 = items\n        .values()\n        .sum();\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn long_literal_is_judged_on_its_header_only() {
        let rows = (0..12).map(|i| format!("    {i},\n")).collect::<String>();
        let src =
            format!("fn f() {{\n    // the lookup table\n    let table = [\n{rows}    ];\n}}");
        assert!(run(Lang::Rust, &src).is_empty());
        let src = "fn f() {\n    // the lookup table\n    let table = [\n        1,\n        2,\n    ];\n}";
        assert!(run(Lang::Rust, src).is_empty());
        let src = "fn f() {\n    // the lookup table\n    let table = [\n        lookup,\n        2,\n    ];\n}";
        assert_eq!(run(Lang::Rust, src).len(), 1);
    }

    #[test]
    fn compound_statement_is_judged_on_its_header_only() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // sum the values\n    for v in items {\n        total += v.values();\n    }\n}"
        )
        .is_empty());
    }

    #[test]
    fn verbs_only_comment_over_matching_call_flagged() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f() {\n    // parse and validate\n    parse(x);\n}"
            )
            .len(),
            1
        );
    }

    /// Replaces the old `lexicon_only_comment_not_flagged`, which exited on the 1-content-word
    /// check before ever reaching the coherence ratio it was meant to exercise.
    #[test]
    fn lexicon_only_comment_fails_coherence() {
        assert!(run(
            Lang::Rust,
            "fn f() {\n    // initialize and setup\n    let x = Foo::new();\n}"
        )
        .is_empty());
    }

    /// Per-comment `next_named_sibling` calls made a run of n sibling comments O(n^2): 26s for
    /// 40k lines in release. The merged block exceeds the 12-word cap, so 0 findings is right;
    /// the bound is loose because CI runners are slow, and the quadratic version takes minutes.
    #[test]
    fn twenty_thousand_consecutive_comments_runs_fast() {
        let mut src = String::from("fn f() {\n");
        for _ in 0..20_000 {
            src.push_str("    // increment the counter\n");
        }
        src.push_str("    counter += 1;\n}");
        let start = std::time::Instant::now();
        let diags = run(Lang::Rust, &src);
        let elapsed = start.elapsed();
        assert_eq!(diags.len(), 0);
        assert!(elapsed.as_secs() < 10, "took {elapsed:?}");
    }
}
