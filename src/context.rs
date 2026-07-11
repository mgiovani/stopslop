use crate::imports_data::DepIndex; // re-exported via rules::imports_data
use crate::lang::Lang;
use tree_sitter::{Node, Tree, TreeCursor};

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
    pub tree: &'a Tree,
    pub lang: Lang,
    pub comments: &'a [TextNode<'a>],
    pub strings: &'a [TextNode<'a>],
    pub is_test_path: bool,
    pub is_stub_file: bool,         // .pyi
    pub deps: Option<&'a DepIndex>, // Some only under --check-imports
}

impl<'a> LintContext<'a> {
    /// One DFS over the whole tree; callers `match node.kind()`. (See ponytail note below.)
    pub fn walk(&self, mut f: impl FnMut(Node<'a>)) {
        let mut c: TreeCursor<'a> = self.tree.walk();
        walk_tree(&mut c, &mut f);
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

/// Single walk that fills both vecs. Comment/string kinds + is_doc rules per §4a table.
pub fn extract<'a>(
    tree: &Tree,
    source: &'a str,
    lang: Lang,
) -> (Vec<TextNode<'a>>, Vec<TextNode<'a>>) {
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
    };

    let mut comments = Vec::new();
    let mut strings = Vec::new();
    let mut c = tree.walk();
    // Node/TreeCursor borrow from `tree` (lifetime 't), independent of `source`'s lifetime 'a.
    // Byte offsets from the tree apply directly to `source` since it's the exact text that was
    // parsed, so we don't need the two lifetimes to unify — only extracted `&'a str` slices matter.
    walk_tree(&mut c, &mut |node| {
        let kind = node.kind();
        if comment_kinds.contains(&kind) {
            comments.push(make_text_node(
                node,
                source,
                is_doc_comment(lang, node, source),
            ));
        } else if string_kinds.contains(&kind) {
            strings.push(make_text_node(node, source, is_doc_string(lang, node)));
        }
    });
    (comments, strings)
}

/// Shared DFS over a `TreeCursor`, visiting every node exactly once. Used by both
/// `LintContext::walk` (rules) and `extract` (comment/string classification).
fn walk_tree<'t>(c: &mut TreeCursor<'t>, f: &mut impl FnMut(Node<'t>)) {
    f(c.node());
    if c.goto_first_child() {
        loop {
            walk_tree(c, f);
            if !c.goto_next_sibling() {
                break;
            }
        }
        c.goto_parent();
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
fn is_doc_comment(lang: Lang, node: Node, source: &str) -> bool {
    let text = &source[node.byte_range()];
    match lang {
        Lang::Ts | Lang::Tsx => text.starts_with("/**"),
        Lang::Python | Lang::Go => false,
        Lang::Rust => {
            text.starts_with("///")
                || text.starts_with("//!")
                || text.starts_with("/**")
                || text.starts_with("/*!")
        }
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
