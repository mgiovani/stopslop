use crate::{context::TextNode, diagnostic::Diagnostic};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// A suppression's rule scope. `None` = every rule ("ai-slop-ignore" bare); `Some(codes)` =
/// only those codes, after group names (`verbosity`) are expanded via `crate::groups::expand`.
type Scope = Option<HashSet<String>>;

/// Matches `ai-slop-ignore` / `ai-slop-ignore-file`, with an optional `: CODE,CODE ...` or
/// `: group` suffix. The scope character class excludes comment delimiters (`-->`, `*/`, `\n`),
/// so it naturally stops there without needing to know which comment style it's inside.
///
/// Anchored: the directive must OPEN the comment body. Matching it anywhere in the text made
/// this project's own source emit eleven dead-suppression warnings, every one from a comment
/// that merely *names* the token while documenting the feature. Prose gets the same protection
/// one layer up -- `prose::scan_ignore_comments` already drops HTML comments inside code spans.
static DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^ai-slop-ignore(-file)?(?::[ \t]*([A-Za-z0-9_,\t ]*))?").unwrap()
});

/// Strips a comment's opening delimiter so the directive can be anchored to what follows it.
/// Longest-first ordering matters: `///` must be tried before `//`, `/**` before `/*`.
pub(crate) fn comment_body(text: &str) -> &str {
    let text = text.trim_start();
    for delim in ["<!--", "///", "//!", "//", "/**", "/*!", "/*", "#"] {
        if let Some(rest) = text.strip_prefix(delim) {
            return rest.trim_start();
        }
    }
    text
}

struct Suppression {
    comment_line: usize, // comment's own start line, for the dead-suppression message
    target_lines: Vec<usize>, // lines it can absorb findings on; unused when file_wide
    file_wide: bool,
    scope: Scope,
}

fn parse_scope(raw: Option<&str>) -> Scope {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let tokens: Vec<String> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    Some(crate::groups::expand(&tokens).into_iter().collect())
}

fn parse(comments: &[TextNode]) -> Vec<Suppression> {
    comments
        .iter()
        .filter_map(|c| {
            let caps = DIRECTIVE.captures(comment_body(c.text))?;
            let file_wide = caps.get(1).is_some();
            let scope = parse_scope(caps.get(2).map(|m| m.as_str()));
            let target_lines = if file_wide {
                Vec::new()
            } else {
                // Comment may span multiple lines (block comment); "below" means below its
                // closing delimiter, not one past its start line.
                let end_line = c.line + c.text.matches('\n').count();
                vec![c.line, end_line + 1]
            };
            Some(Suppression {
                comment_line: c.line,
                target_lines,
                file_wide,
                scope,
            })
        })
        .collect()
}

fn scope_matches(scope: &Scope, code: &str) -> bool {
    match scope {
        None => true,
        Some(codes) => codes.contains(code),
    }
}

fn union_scope(a: Scope, b: Scope) -> Scope {
    match (a, b) {
        (None, _) | (_, None) => None,
        (Some(x), Some(y)) => Some(x.union(&y).cloned().collect()),
    }
}

/// A line with multiple ignore comments unions their scopes; a bare one anywhere on the line
/// wins as "all" (`union_scope`'s `None` short-circuit). Same shape for file-wide.
fn aggregate(suppressions: &[Suppression]) -> (Option<Scope>, HashMap<usize, Scope>) {
    let mut file_wide: Option<Scope> = None;
    let mut lines: HashMap<usize, Scope> = HashMap::new();
    for s in suppressions {
        if s.file_wide {
            file_wide = Some(match file_wide.take() {
                None => s.scope.clone(),
                Some(existing) => union_scope(existing, s.scope.clone()),
            });
        } else {
            for &line in &s.target_lines {
                let existing = lines.remove(&line).unwrap_or_else(|| Some(HashSet::new()));
                lines.insert(line, union_scope(existing, s.scope.clone()));
            }
        }
    }
    (file_wide, lines)
}

fn describe_scope(scope: &Scope) -> String {
    match scope {
        None => "all rules".to_string(),
        Some(codes) => {
            let mut codes: Vec<&str> = codes.iter().map(String::as_str).collect();
            codes.sort_unstable();
            codes.join(", ")
        }
    }
}

// ponytail: these warnings are eprintln! rather than real diagnostics because surfacing them in
// JSON/SARIF output needs a rule code of their own (a "dead suppression" lint) -- deferred until
// something actually consumes structured output for this.
/// True when `diags` contains nothing this suppression actually absorbs: no diagnostic both
/// lands on one of its target lines (or the whole file, when `file_wide`) and matches its scope.
fn is_dead(s: &Suppression, diags: &[Diagnostic]) -> bool {
    !diags.iter().any(|d| {
        let on_target = s.file_wide || s.target_lines.contains(&d.line);
        on_target && scope_matches(&s.scope, d.code)
    })
}

fn report_dead(suppressions: &[Suppression], diags: &[Diagnostic], display_path: &str) {
    for s in suppressions {
        if is_dead(s, diags) {
            let directive = if s.file_wide {
                "ai-slop-ignore-file"
            } else {
                "ai-slop-ignore"
            };
            eprintln!(
                "stopslop: warning: {display_path}:{}: {directive} ({}) suppressed nothing",
                s.comment_line,
                describe_scope(&s.scope),
            );
        }
    }
}

pub fn apply(diags: &mut Vec<Diagnostic>, comments: &[TextNode], display_path: &str) {
    let suppressions = parse(comments);
    report_dead(&suppressions, diags, display_path);

    let (file_wide, lines) = aggregate(&suppressions);
    diags.retain(|d| {
        let file_hit = file_wide.as_ref().is_some_and(|s| scope_matches(s, d.code));
        let line_hit = lines.get(&d.line).is_some_and(|s| scope_matches(s, d.code));
        !file_hit && !line_hit
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Diagnostic, Tier};

    fn diag(line: usize, code: &'static str) -> Diagnostic {
        Diagnostic {
            code,
            name: "test",
            tier: Tier::A,
            path: "f.ts".into(),
            line,
            col: 1,
            message: "test".into(),
            fix: None,
        }
    }

    fn node(text: &'static str, line: usize) -> TextNode<'static> {
        TextNode {
            text,
            start_byte: 0,
            end_byte: text.len(),
            line,
            col: 1,
            is_doc: false,
        }
    }

    /// A comment that merely names the directive while documenting it is not a directive. This
    /// project's own source is full of such comments; before anchoring, each one both silently
    /// suppressed its line and then warned that it had suppressed nothing.
    #[test]
    fn directive_must_open_the_comment_to_count() {
        for text in [
            "/// A suppression's scope. `None` = every rule (\"ai-slop-ignore\" bare).",
            "// dogfooding would self-flag these; `// ai-slop-ignore` is the escape hatch",
            "<!-- documenting the `ai-slop-ignore` syntax, not using it -->",
        ] {
            let mut diags = vec![diag(1, "SLOP004")];
            apply(&mut diags, &[node(text, 1)], "f.rs");
            assert_eq!(diags.len(), 1, "mention should not suppress: {text:?}");
        }
    }

    /// The flip side: the delimiter still gets stripped, so real directives keep working in
    /// every comment style the supported languages use.
    #[test]
    fn directive_is_recognized_after_each_comment_delimiter() {
        for text in [
            "// ai-slop-ignore",
            "///ai-slop-ignore",
            "# ai-slop-ignore",
            "/* ai-slop-ignore */",
            "<!-- ai-slop-ignore -->",
        ] {
            let mut diags = vec![diag(1, "SLOP004")];
            apply(&mut diags, &[node(text, 1)], "f.rs");
            assert!(diags.is_empty(), "should suppress: {text:?}");
        }
    }

    /// Multi-line block comment starting at line 2, ending at line 4 (`\n` x2), must
    /// suppress a finding on line 5 (the line below its closing `*/`), not line 3.
    #[test]
    fn multiline_block_comment_suppresses_line_after_its_end() {
        let comment = node("/* ai-slop-ignore\n   multi-line note\n*/", 2);
        let mut diags = vec![diag(5, "SLOP004")];
        apply(&mut diags, &[comment], "f.ts");
        assert!(
            diags.is_empty(),
            "line below a multi-line comment's end should be suppressed"
        );
    }

    #[test]
    fn bare_suppresses_all_rules_on_the_line() {
        let comment = node("// ai-slop-ignore", 3);
        let mut diags = vec![diag(3, "SLOP004"), diag(3, "SLOP018")];
        apply(&mut diags, &[comment], "f.ts");
        assert!(diags.is_empty());
    }

    #[test]
    fn scoped_suppresses_only_named_codes() {
        let comment = node("// ai-slop-ignore: SLOP004,SLOP018", 3);
        let mut diags = vec![diag(3, "SLOP004"), diag(3, "SLOP025")];
        apply(&mut diags, &[comment], "f.ts");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP025");
    }

    #[test]
    fn scoped_accepts_space_separated_codes() {
        let comment = node("// ai-slop-ignore: SLOP004 SLOP018", 3);
        let mut diags = vec![diag(3, "SLOP004"), diag(3, "SLOP018")];
        apply(&mut diags, &[comment], "f.ts");
        assert!(diags.is_empty());
    }

    #[test]
    fn scoped_accepts_group_names() {
        // "sourcing" is SLOP025 only (see groups.rs).
        let comment = node("// ai-slop-ignore: sourcing", 3);
        let mut diags = vec![diag(3, "SLOP025"), diag(3, "SLOP004")];
        apply(&mut diags, &[comment], "f.ts");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP004");
    }

    #[test]
    fn scoped_file_wide_only_drops_named_code() {
        let comment = node("// ai-slop-ignore-file: SLOP018", 1);
        let mut diags = vec![diag(50, "SLOP018"), diag(99, "SLOP004")];
        apply(&mut diags, &[comment], "f.ts");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP004");
    }

    #[test]
    fn dead_suppression_is_detected() {
        let suppressions = parse(&[node("// ai-slop-ignore: SLOP018", 3)]);
        let diags = [diag(3, "SLOP004")]; // wrong code -> the ignore absorbs nothing
        assert!(is_dead(&suppressions[0], &diags));
    }

    #[test]
    fn live_suppression_is_not_dead() {
        let suppressions = parse(&[node("// ai-slop-ignore: SLOP018", 3)]);
        let diags = [diag(3, "SLOP018")]; // matching code and line -> absorbed
        assert!(!is_dead(&suppressions[0], &diags));
    }
}
