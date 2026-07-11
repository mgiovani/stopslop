use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP001",
    name: "Elision / \"rest unchanged\" comment",
    tier: Tier::A,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    default_on: true,
    path_gated: false,
    check,
};

static RE_A: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?im)^\s*(?://|#|\*+)\s*\.\.\.?\s*(rest|existing|other|remaining|unchanged|keep)\b",
    )
    .unwrap()
});
static RE_B: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\.\.\.\s*(existing code|rest of|unchanged|other methods|remaining)").unwrap()
});

/// True iff everything from the start of `start_byte`'s line up to `start_byte` is whitespace,
/// i.e. the node is the first non-whitespace thing on its line (kills trailing-after-code FPs
/// and TS `...rest` spread, which is code with no comment node at all).
fn is_first_on_line(source: &str, start_byte: usize) -> bool {
    let line_start = source[..start_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    source[line_start..start_byte]
        .chars()
        .all(char::is_whitespace)
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for c in ctx.comments {
        if c.is_doc || !is_first_on_line(ctx.source, c.start_byte) {
            continue;
        }
        if RE_A.is_match(c.text) || RE_B.is_match(c.text) {
            out.push(Diagnostic::at(
                rule,
                ctx,
                c.line,
                c.col,
                "elision comment may have replaced real code",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rest_unchanged() {
        assert!(RE_A.is_match("// ... rest of code unchanged"));
        assert!(RE_A.is_match("# ... existing code"));
        assert!(RE_B.is_match("// ... other methods unchanged"));
    }

    #[test]
    fn does_not_match_prose() {
        assert!(!RE_A.is_match("// process the rest of the data"));
        assert!(!RE_B.is_match("// process the rest of the data"));
    }

    #[test]
    fn first_on_line_detection() {
        let src = "let x = 1; // ...rest\n// ...rest\n";
        // trailing-after-code comment starts at the position right after "let x = 1; "
        let trailing_start = src.find("// ...rest").unwrap();
        assert!(!is_first_on_line(src, trailing_start));
        let leading_start = src.rfind("// ...rest").unwrap();
        assert!(is_first_on_line(src, leading_start));
    }
}
