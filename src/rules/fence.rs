use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP003",
    name: "Stray markdown code fence in source",
    tier: Tier::A,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    default_on: true,
    path_gated: false,
    check,
};

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*```[A-Za-z0-9_+-]*\s*$").unwrap());

// Deliberately not ctx.in_comment_or_string(byte): grammars mis-lex a bare ``` as a same-line
// string start, so that check would call the fence itself "in a string" and miss it
// (vscode#295126). Require the enclosing node to contain the FULL line instead.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let mut byte_offset = 0usize;
    for (idx, line) in ctx.source.split('\n').enumerate() {
        if RE.is_match(line) {
            let indent = line.find('`').unwrap();
            let line_end = byte_offset + line.len();
            let enclosed = ctx
                .comments
                .iter()
                .chain(ctx.strings.iter())
                .any(|n| n.start_byte <= byte_offset && n.end_byte >= line_end);
            if !enclosed {
                out.push(Diagnostic::at(
                    rule,
                    ctx,
                    idx + 1,
                    indent + 1,
                    "stray markdown code fence in source file",
                ));
            }
        }
        byte_offset += line.len() + 1; // +1 for the '\n' consumed by split
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_bare_fence() {
        assert!(RE.is_match("```"));
        assert!(RE.is_match("```javascript"));
        assert!(RE.is_match("  ```python  "));
    }

    #[test]
    fn does_not_match_prefixed_fence() {
        assert!(!RE.is_match("// ```"));
        assert!(!RE.is_match("const x = `template`;"));
    }
}
