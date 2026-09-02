use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP007",
    name: "Type-escape (`as any`/`as unknown`/`@ts-ignore`)",
    tier: Tier::A,
    langs: &[Lang::Ts, Lang::Tsx],
    default_on: true,
    path_gated: false,
    check,
};

// `@ts-ignore`/`@ts-nocheck` directives. The `regex` crate has no look-around, so a trailing
// `[code]` (e.g. `@ts-ignore[2322]`, a scoped/intentional suppression) is excluded below by
// checking the text right after the match instead of a negative lookahead.
static TS_DIRECTIVE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*//\s*@ts-(ignore|nocheck)(?-u:\b)").unwrap());

fn is_blanket_directive(text: &str) -> bool {
    let Some(m) = TS_DIRECTIVE_RE.find(text) else {
        return false;
    };
    !text[m.end()..].trim_start().starts_with('[')
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["as_expression"]) {
        // as_expression named children are [expression, type] (no fields; see node-types.json).
        let Some(ty) = node.named_child(1) else {
            continue;
        };
        if ty.kind() != "predefined_type" {
            continue; // `as const`, `as Record<..>`, etc. — not an escape kind
        }
        let text = ctx.node_text(&ty);
        // Only the INNER cast of `x as unknown as T` counts: its parent is itself an
        // as_expression (the outer `as T`). A standalone `x as unknown` is not an escape.
        let is_chained = node.parent().is_some_and(|p| p.kind() == "as_expression");
        let msg = if text == "any" {
            Some("`as any` disables type checking")
        } else if text == "unknown" && is_chained {
            Some("`as unknown` cast bypasses type checking")
        } else {
            None
        };
        if let Some(msg) = msg {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, msg));
        }
    }

    for c in ctx.comments {
        if c.text.starts_with("//") && is_blanket_directive(c.text) {
            out.push(Diagnostic::at(
                rule,
                ctx,
                c.line,
                c.col,
                "@ts-ignore / @ts-nocheck suppresses type errors",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_blanket_directive;

    #[test]
    fn directive_regex_flags_ignore_and_nocheck() {
        assert!(is_blanket_directive("// @ts-ignore"));
        assert!(is_blanket_directive("// @ts-nocheck"));
        assert!(is_blanket_directive("//@ts-ignore"));
        assert!(is_blanket_directive("// @ts-ignore expect: SLOP007"));
    }

    #[test]
    fn directive_regex_spares_expect_error_and_coded_ignore() {
        assert!(!is_blanket_directive(
            "// @ts-expect-error – intentional shim"
        ));
        assert!(!is_blanket_directive("// @ts-ignore[2322] – safe coercion"));
    }
}
