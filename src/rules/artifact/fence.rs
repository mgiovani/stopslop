use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, CODE_LANGS};
use crate::registry::RuleDef;
use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP003",
    name: "Stray markdown code fence in source",
    tier: Tier::A,
    langs: CODE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*```[A-Za-z0-9_+-]*\s*$").unwrap());

/// Source with every fence-shaped line blanked, byte for byte, so the parser sees the code inside
/// a pasted reply. Offsets and line numbers match the original.
pub(crate) fn blank_fence_lines(source: &str) -> Cow<'_, str> {
    if !source.contains("```") {
        return Cow::Borrowed(source);
    }
    let blanked = source
        .split('\n')
        .map(|line| {
            if RE.is_match(line) {
                line.chars()
                    .flat_map(|c| (0..c.len_utf8()).map(|_| ' '))
                    .collect()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join("\n");
    Cow::Owned(blanked)
}

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
    fn blanking_preserves_bytes_and_lines() {
        let src = "```ts\nlet é = 1;\n\u{a0}```\u{a0}\n```";
        let out = blank_fence_lines(src);
        assert_eq!(out.len(), src.len());
        assert_eq!(out.matches('\n').count(), src.matches('\n').count());
        assert_eq!(out.lines().nth(1), Some("let é = 1;"));
        assert!(!out.contains('`'));
    }

    #[test]
    fn blanking_borrows_when_no_fence() {
        assert!(matches!(
            blank_fence_lines("const x = `a`;"),
            Cow::Borrowed(_)
        ));
        assert_eq!(blank_fence_lines("x = 1 // ```"), "x = 1 // ```");
    }

    #[test]
    fn does_not_match_prefixed_fence() {
        assert!(!RE.is_match("// ```"));
        assert!(!RE.is_match("const x = `template`;"));
    }
}
