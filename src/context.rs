use crate::imports_data::DepIndex; // re-exported via rules::imports_data
use crate::lang::Lang;
use crate::prose::ProseDoc;
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
    pub tree: Option<&'a Tree>,
    pub lang: Lang,
    pub comments: &'a [TextNode<'a>],
    pub strings: &'a [TextNode<'a>],
    pub is_test_path: bool,
    pub is_stub_file: bool,              // .pyi
    pub deps: Option<&'a DepIndex>,      // Some only under --check-imports
    pub prose: Option<&'a ProseDoc<'a>>, // Some only for prose langs (see lang::Lang::is_prose)
}

impl<'a> LintContext<'a> {
    /// One DFS over the whole tree; callers `match node.kind()`. (See ponytail note below.)
    /// No-op for prose langs (`tree` is `None`).
    pub fn walk(&self, mut f: impl FnMut(Node<'a>)) {
        let Some(tree) = self.tree else { return };
        let mut c: TreeCursor<'a> = tree.walk();
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
        // Never reached: prose langs bypass this extraction path entirely (engine::lint_prose).
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => (&[], &[]),
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

/// Shared pre-order DFS over a `TreeCursor`, visiting every node exactly once. Used by both
/// `LintContext::walk` (rules) and `extract` (comment/string classification).
///
/// Iterative on purpose: the recursive version cost one stack frame per nesting level and blew
/// the stack (SIGABRT, not a clean exit code) on ~5k-deep bracket nesting -- 10 KB of generated
/// or minified source. Both callers start the cursor at the root, so climbing past it ends the
/// walk.
fn walk_tree<'t>(c: &mut TreeCursor<'t>, f: &mut impl FnMut(Node<'t>)) {
    loop {
        f(c.node());
        if c.goto_first_child() {
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
        // Never reached: prose langs never call extract().
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => false,
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
    use tree_sitter::Parser;

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
        walk_tree(&mut c, &mut |_| visited += 1);
        assert!(visited > 20_000, "visited {visited} nodes");
    }
}
