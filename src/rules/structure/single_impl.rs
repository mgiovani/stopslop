use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang};
use crate::registry::RuleDef;
use tree_sitter::Node;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP040",
    name: "Single-implementation interface / abstract",
    tier: Tier::B,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python],
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: true,
    check,
};

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    match ctx.lang {
        Lang::Ts | Lang::Tsx => check_ts(rule, ctx, out),
        Lang::Python => check_python(rule, ctx, out),
        Lang::Go | Lang::Rust | Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst | Lang::Html => {} // rule.langs excludes these; never reached
    }
}

fn flag(
    rule: &'static RuleDef,
    ctx: &LintContext,
    def_node: Node,
    name: &str,
    implementor: &str,
    out: &mut Vec<Diagnostic>,
) {
    let (line, col) = ctx.pos(&def_node);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        format!("`{name}` has a single implementation (`{implementor}`)"),
        "inline it; reintroduce the abstraction when a second implementation exists",
    ));
}

// --- TypeScript / TSX ---

fn check_ts(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let mut abstractions: Vec<(&str, Node)> = Vec::new();
    for node in ctx.nodes(&["interface_declaration", "abstract_class_declaration"]) {
        if let Some(name) = node.child_by_field_name("name") {
            abstractions.push((ctx.node_text(&name), node));
        }
    }
    if abstractions.is_empty() {
        return;
    }

    // (abstraction name referenced, name of the class referencing it)
    let mut refs: Vec<(String, &str)> = Vec::new();
    for node in ctx.nodes(&["class_declaration", "abstract_class_declaration"]) {
        let Some(impl_name_node) = node.child_by_field_name("name") else {
            continue;
        };
        let impl_name = ctx.node_text(&impl_name_node);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "class_heritage" {
                continue;
            }
            let mut hc = child.walk();
            for h in child.children(&mut hc) {
                match h.kind() {
                    "extends_clause" => {
                        if let Some(v) = h.named_child(0) {
                            refs.push((ts_base_name(ctx, v), impl_name));
                        }
                    }
                    "implements_clause" => {
                        let mut ic = h.walk();
                        for t in h.named_children(&mut ic) {
                            refs.push((ts_base_name(ctx, t), impl_name));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for (name, def_node) in &abstractions {
        let matches: Vec<&str> = refs
            .iter()
            .filter(|(n, im)| n == name && im != name)
            .map(|(_, im)| *im)
            .collect();
        if let [only] = matches[..] {
            flag(rule, ctx, *def_node, name, only, out);
        }
    }
}

/// Strip generic type arguments (`Foo<Bar>` -> `Foo`) so a parameterized reference still
/// matches the plain abstraction name it instantiates.
fn ts_base_name(ctx: &LintContext, node: Node) -> String {
    let text = ctx.node_text(&node);
    text.split('<').next().unwrap_or(text).trim().to_string()
}

// --- Python ---

fn check_python(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let mut abstractions: Vec<(&str, Node)> = Vec::new();
    for node in ctx.nodes(&["class_definition"]) {
        if python_is_abstraction(ctx, node) {
            if let Some(name) = node.child_by_field_name("name") {
                abstractions.push((ctx.node_text(&name), node));
            }
        }
    }
    if abstractions.is_empty() {
        return;
    }

    let mut refs: Vec<(String, &str)> = Vec::new();
    for node in ctx.nodes(&["class_definition"]) {
        let Some(name_node) = node.child_by_field_name("name") else {
            continue;
        };
        let impl_name = ctx.node_text(&name_node);
        let Some(sc) = node.child_by_field_name("superclasses") else {
            continue;
        };
        let mut cursor = sc.walk();
        for base in sc.named_children(&mut cursor) {
            if base.kind() == "keyword_argument" {
                continue; // e.g. `metaclass=ABCMeta`: not a base-class reference
            }
            refs.push((python_base_name(ctx, base), impl_name));
        }
    }

    for (name, def_node) in &abstractions {
        let matches: Vec<&str> = refs
            .iter()
            .filter(|(n, im)| n == name && im != name)
            .map(|(_, im)| *im)
            .collect();
        if let [only] = matches[..] {
            flag(rule, ctx, *def_node, name, only, out);
        }
    }
}

/// `ABC`/`abc.ABC` base, `metaclass=ABCMeta`, or a directly-declared `@abstractmethod` member.
fn python_is_abstraction(ctx: &LintContext, class_node: Node) -> bool {
    if let Some(sc) = class_node.child_by_field_name("superclasses") {
        if ctx.node_text(&sc).contains("ABC") {
            return true;
        }
    }
    let Some(body) = class_node.child_by_field_name("body") else {
        return false;
    };
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        if child.kind() != "decorated_definition" {
            continue;
        }
        let mut dc = child.walk();
        for d in child.children(&mut dc) {
            if d.kind() == "decorator" && ctx.node_text(&d).contains("abstractmethod") {
                return true;
            }
        }
    }
    false
}

/// `Foo[T]` (a `subscript` node) -> `Foo`, so a generic base still matches the plain name.
fn python_base_name(ctx: &LintContext, node: Node) -> String {
    if node.kind() == "subscript" {
        if let Some(v) = node.child_by_field_name("value") {
            return ctx.node_text(&v).to_string();
        }
    }
    ctx.node_text(&node).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use tree_sitter::Parser;

    fn lint(lang: Lang, src: &str) -> Vec<Diagnostic> {
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(lang)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, strings, index) = context::extract(&tree, src, lang);
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
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    // --- TS ---

    #[test]
    fn ts_interface_single_impl_flagged() {
        let src = "interface Storage { get(k: string): string; }\nclass MemStorage implements Storage { get(k: string) { return k; } }\n";
        let diags = lint(Lang::Ts, src);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "`Storage` has a single implementation (`MemStorage`)"
        );
    }

    #[test]
    fn ts_interface_zero_impls_clean() {
        let src = "interface Storage { get(k: string): string; }\n";
        assert_eq!(lint(Lang::Ts, src).len(), 0);
    }

    #[test]
    fn ts_interface_two_impls_clean() {
        let src = "interface Storage { get(k: string): string; }\nclass A implements Storage { get(k: string) { return k; } }\nclass B implements Storage { get(k: string) { return k; } }\n";
        assert_eq!(lint(Lang::Ts, src).len(), 0);
    }

    #[test]
    fn ts_abstract_class_single_impl_flagged() {
        let src =
            "abstract class Base { abstract run(): void; }\nclass Impl extends Base { run() {} }\n";
        let diags = lint(Lang::Ts, src);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "`Base` has a single implementation (`Impl`)"
        );
    }

    #[test]
    fn ts_generic_implements_matches_base_name() {
        let src = "interface Repo<T> { get(): T; }\nclass UserRepo implements Repo<User> { get() { return null as unknown as User; } }\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    // --- Python ---

    #[test]
    fn python_abc_single_impl_flagged() {
        let src = "from abc import ABC\nclass Storage(ABC):\n    pass\nclass MemStorage(Storage):\n    pass\n";
        let diags = lint(Lang::Python, src);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "`Storage` has a single implementation (`MemStorage`)"
        );
    }

    #[test]
    fn python_abstractmethod_single_impl_flagged() {
        let src = "class Storage:\n    @abstractmethod\n    def get(self):\n        pass\nclass MemStorage(Storage):\n    def get(self):\n        return None\n";
        assert_eq!(lint(Lang::Python, src).len(), 1);
    }

    #[test]
    fn python_zero_impls_clean() {
        let src = "from abc import ABC\nclass Storage(ABC):\n    pass\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_two_impls_clean() {
        let src = "from abc import ABC\nclass Storage(ABC):\n    pass\nclass A(Storage):\n    pass\nclass B(Storage):\n    pass\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_plain_base_not_abstraction_clean() {
        let src = "class Base:\n    pass\nclass Impl(Base):\n    pass\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }
}
