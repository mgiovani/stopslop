use crate::imports_data::DepIndex; // re-exported via rules::imports_data
use crate::lang::{Lang, NatLang};
use crate::prose::ProseDoc;
use std::collections::HashMap;
use tree_sitter::{LanguageRef, Node, Tree, TreeCursor};

#[derive(Debug, Clone)]
pub struct TextNode<'a> {
    pub text: &'a str, // exact source slice of the node (incl. // # /* delimiters or quotes)
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,  // 1-based start line
    pub col: usize,   // 1-based start col
    pub is_doc: bool, // Rust /// //! /** /*! ; TS /** JSDoc ; Python docstring
}

pub struct LintContext<'a> {
    pub display_path: String,
    pub source: &'a str,
    pub index: Option<&'a NodeIndex<'a>>, // None for prose langs (no tree)
    pub lang: Lang,
    pub comments: &'a [TextNode<'a>],
    pub strings: &'a [TextNode<'a>],
    pub is_test_path: bool,
    pub is_stub_file: bool,              // .pyi
    pub deps: Option<&'a DepIndex>,      // Some only under --check-imports
    pub prose: Option<&'a ProseDoc<'a>>, // Some only for prose langs (see lang::Lang::is_prose)
    /// Natural languages the document is assumed to contain, resolved once from config (default:
    /// every supported language). A rule is gated out when its `natlangs` shares nothing with
    /// this set (engine::lint_file, engine::lint_prose); a no-op under the default.
    pub natlangs: &'a [NatLang],
}

impl<'a> LintContext<'a> {
    /// Every named node whose kind is in `kinds`, in the tree's pre-order. Anonymous tokens
    /// (keywords, punctuation) are not indexed; empty for prose langs.
    pub fn nodes(&self, kinds: &[&str]) -> Vec<Node<'a>> {
        let Some(index) = self.index else {
            return Vec::new();
        };
        let mut positions: Vec<usize> = kinds
            .iter()
            .filter_map(|k| index.kind_id(k))
            .filter_map(|id| index.by_kind.get(&id))
            .flatten()
            .copied()
            .collect();
        positions.sort_unstable();
        positions.dedup();
        positions.into_iter().map(|i| index.all[i]).collect()
    }
    pub fn pos(&self, node: &Node) -> (usize, usize) {
        let p = node.start_position();
        (p.row + 1, p.column + 1)
    }
    pub fn node_text(&self, node: &Node) -> &'a str {
        &self.source[node.byte_range()]
    }
    pub fn in_comment_or_string(&self, byte: usize) -> bool {
        self.comments
            .iter()
            .chain(self.strings.iter())
            .any(|n| byte >= n.start_byte && byte < n.end_byte)
    }
}

/// Every named node of a tree in pre-order, plus each kind's positions in that order. Built
/// once per file by `extract`; AST rules query it through `LintContext::nodes` instead of
/// re-walking the tree, so a file costs one traversal however many rules run (issue #8: a
/// TypeScript file with every rule on used to get 11). Positions rather than nodes per kind so
/// a multi-kind query can merge back into pre-order. Anonymous tokens are skipped: they are
/// ~45% of all nodes and no rule has ever queried one.
pub struct NodeIndex<'t> {
    lang: LanguageRef<'t>,
    all: Vec<Node<'t>>,
    by_kind: HashMap<u16, Vec<usize>>, // kind id -> positions in `all`
}

impl NodeIndex<'_> {
    /// Keyed by `Node::kind_id` rather than `Node::kind`: the id is one FFI read per node, the
    /// string is that plus a strlen, a UTF-8 check and a longer hash. `None` for a name that is
    /// not a named kind of this grammar (tree-sitter reserves id 0 for "not found").
    fn kind_id(&self, kind: &str) -> Option<u16> {
        match self.lang.id_for_node_kind(kind, true) {
            0 => None,
            id => Some(id),
        }
    }
}

/// The single traversal: fills the comment/string vecs (kinds + is_doc rules per §4a table) and
/// the node index.
pub fn extract<'t, 'a>(
    tree: &'t Tree,
    source: &'a str,
    lang: Lang,
) -> (Vec<TextNode<'a>>, Vec<TextNode<'a>>, NodeIndex<'t>) {
    let (comment_kinds, string_kinds): (&[&str], &[&str]) = match lang {
        Lang::Ts | Lang::Tsx => (&["comment"], &["string", "template_string"]),
        Lang::Python => (&["comment"], &["string"]),
        Lang::Go => (
            &["comment"],
            &["interpreted_string_literal", "raw_string_literal"],
        ),
        Lang::Rust => (
            &["line_comment", "block_comment"],
            &["string_literal", "raw_string_literal"],
        ),
        // Never reached: prose langs bypass this extraction path entirely (engine::lint_prose).
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst | Lang::Html => (&[], &[]),
    };

    let mut index = NodeIndex {
        lang: tree.language(),
        all: Vec::new(),
        by_kind: HashMap::new(),
    };
    let comment_ids: Vec<u16> = comment_kinds
        .iter()
        .filter_map(|k| index.kind_id(k))
        .collect();
    let string_ids: Vec<u16> = string_kinds
        .iter()
        .filter_map(|k| index.kind_id(k))
        .collect();

    let mut comments = Vec::new();
    let mut strings = Vec::new();
    let mut c = tree.walk();
    // Node/TreeCursor borrows `tree` (lifetime 't), independent of `source`'s lifetime 'a. Tree
    // byte offsets apply directly to `source` since it's the exact parsed text, so the two
    // lifetimes don't need to unify.
    walk_tree(&mut c, &mut |node| {
        if !node.is_named() {
            return true;
        }
        let id = node.kind_id();
        index.by_kind.entry(id).or_default().push(index.all.len());
        index.all.push(node);
        if comment_ids.contains(&id) {
            comments.push(make_text_node(
                node,
                source,
                is_doc_comment(lang, &source[node.byte_range()]),
            ));
        } else if string_ids.contains(&id) {
            strings.push(make_text_node(node, source, is_doc_string(lang, node)));
        }
        true
    });
    (comments, strings, index)
}

/// Pre-order DFS over a `TreeCursor`, visiting every node exactly once. `f` returns whether to
/// descend into the node's children; `prose::parse_html` uses `false` to skip `<pre>` subtrees.
///
/// Iterative on purpose: the recursive version cost one stack frame per nesting level and blew
/// the stack (SIGABRT, not a clean exit code) on ~5k-deep bracket nesting -- 10 KB of generated
/// or minified source. The cursor starts at the root, so climbing past it ends the walk.
pub(crate) fn walk_tree<'t>(c: &mut TreeCursor<'t>, f: &mut impl FnMut(Node<'t>) -> bool) {
    loop {
        if f(c.node()) && c.goto_first_child() {
            continue;
        }
        while !c.goto_next_sibling() {
            if !c.goto_parent() {
                return;
            }
        }
    }
}

fn make_text_node<'t, 'a>(node: Node<'t>, source: &'a str, is_doc: bool) -> TextNode<'a> {
    let p = node.start_position();
    TextNode {
        text: &source[node.byte_range()],
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        line: p.row + 1,
        col: p.column + 1,
        is_doc,
    }
}

/// is_doc(comment) per §4a: Ts/Tsx `/**`; Python never (docstrings are strings); Go never;
/// Rust `///` `//!` `/**` `/*!`.
pub(crate) fn is_doc_comment(lang: Lang, text: &str) -> bool {
    match lang {
        Lang::Ts | Lang::Tsx => text.starts_with("/**"),
        Lang::Python | Lang::Go => false,
        Lang::Rust => {
            text.starts_with("///")
                || text.starts_with("//!")
                || text.starts_with("/**")
                || text.starts_with("/*!")
        }
        // Never reached: prose langs never call extract().
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst | Lang::Html => false,
    }
}

/// is_doc(string) per §4a: only Python, via the classic "expression_statement parent"
/// docstring heuristic. `// ponytail: good enough for placeholder/fence exemption.`
fn is_doc_string(lang: Lang, node: Node) -> bool {
    match lang {
        Lang::Python => node
            .parent()
            .map(|p| p.kind() == "expression_statement")
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use std::collections::BTreeSet;
    use tree_sitter::Parser;

    /// The index must answer exactly what a kind-filtered pre-order walk over the named nodes
    /// would, per kind and for several kinds at once, in every grammar. This is the assumption
    /// every AST rule's `ctx.nodes(...)` query rests on, including that `kind_id` and
    /// `id_for_node_kind` agree for aliased symbols. The TS and Python samples each carry a
    /// name that is both a named kind and a keyword (`number` literal vs. type, `lambda`
    /// expression vs. keyword) so the named-only lookup is exercised where it matters.
    #[test]
    fn nodes_query_matches_a_kind_filtered_preorder_walk() {
        let samples = [
            (Lang::Ts, "const f = (a: number) => { try { g(42) } catch (e) {} };\n"),
            (Lang::Python, "import os\nf = lambda: 0\nclass A(B):\n    def f(self):\n        try:\n            pass\n        except Exception:\n            pass\n"),
            (Lang::Go, "package p\nimport \"fmt\"\nfunc f() { if err != nil { } }\n"),
            (Lang::Rust, "use std::io;\nfn f() -> i32 { match g() { Err(_) => {} Ok(_) => {} } }\n"),
        ];
        for (lang, src) in samples {
            let mut p = Parser::new();
            p.set_language(&crate::lang::ts_language(lang)).unwrap();
            let tree = p.parse(src, None).unwrap();
            let (comments, strings, index) = extract(&tree, src, lang);
            let ctx = LintContext {
                display_path: "t".into(),
                source: src,
                index: Some(&index),
                lang,
                comments: &comments,
                strings: &strings,
                is_test_path: false,
                is_stub_file: false,
                deps: None,
                prose: None,
                natlangs: crate::lang::ALL_NATLANGS,
            };
            let mut preorder = Vec::new();
            walk_tree(&mut tree.walk(), &mut |n| {
                if n.is_named() {
                    preorder.push(n);
                }
                true
            });
            let kinds: Vec<&str> = preorder
                .iter()
                .map(|n| n.kind())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            for &k in &kinds {
                let expected: Vec<Node> =
                    preorder.iter().copied().filter(|n| n.kind() == k).collect();
                assert_eq!(ctx.nodes(&[k]), expected, "{lang:?} kind {k:?}");
            }
            assert_eq!(ctx.nodes(&kinds), preorder, "{lang:?} every kind at once");
            assert!(ctx.nodes(&["no_such_kind"]).is_empty());
        }
    }

    #[test]
    fn nodes_query_is_empty_for_prose_ctx() {
        let ctx = LintContext {
            display_path: "t.md".into(),
            source: "",
            index: None,
            lang: Lang::Md,
            comments: &[],
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            natlangs: crate::lang::ALL_NATLANGS,
        };
        assert!(ctx.nodes(&["paragraph"]).is_empty());
    }

    /// The recursive `walk_tree` aborted the process (stack overflow, exit 134) at ~5k nesting
    /// levels -- 10 KB of generated source. Test threads get a smaller stack than main, so a
    /// regression here takes the whole test run down with it.
    #[test]
    fn deeply_nested_source_does_not_overflow_the_stack() {
        let src = format!("const x = {}1{};\n", "[".repeat(20_000), "]".repeat(20_000));
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(Lang::Ts)).unwrap();
        let tree = p.parse(&src, None).unwrap();
        let mut visited = 0usize;
        let mut c = tree.walk();
        walk_tree(&mut c, &mut |_| {
            visited += 1;
            true
        });
        assert!(visited > 20_000, "visited {visited} nodes");
    }
}
