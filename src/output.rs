use crate::{
    cli::Format,
    diagnostic::{Diagnostic, Tier},
    paths::strip_dot_slash,
    registry::RULES,
};
use std::collections::HashSet;
use std::io::Write;

pub fn emit(
    format: Format,
    diags: &[Diagnostic],
    enabled: &HashSet<&str>,
    w: &mut impl Write,
) -> std::io::Result<()> {
    match format {
        Format::Text => emit_text(diags, w),
        Format::Json => emit_json(diags, w),
        Format::Sarif => emit_sarif(diags, enabled, w),
        Format::Markdown => emit_markdown(diags, w),
    }
}

/// Tier A blocks a build and Tier B does not, so a flat list buries the distinction that decides
/// whether anyone has to act. Groups under one heading per tier, heaviest first.
fn emit_markdown(diags: &[Diagnostic], w: &mut impl Write) -> std::io::Result<()> {
    if diags.is_empty() {
        return writeln!(w, "No stopslop findings.");
    }

    for (tier, heading) in [
        (Tier::A, "Tier A -- these fail the build"),
        (Tier::B, "Tier B -- advisory, they do not fail the build"),
    ] {
        let group: Vec<_> = diags.iter().filter(|d| d.tier == tier).collect();
        if group.is_empty() {
            continue;
        }
        writeln!(w, "### {heading} ({})\n", group.len())?;
        for d in group {
            let loc = format!("{}:{}:{}", strip_dot_slash(&d.path), d.line, d.col);
            writeln!(w, "- `{loc}` **{}** {}", d.code, d.message)?;
            if let Some(fix) = &d.fix {
                writeln!(w, "  - fix: {fix}")?;
            }
        }
        writeln!(w)?;
    }
    Ok(())
}

fn emit_text(diags: &[Diagnostic], w: &mut impl Write) -> std::io::Result<()> {
    for d in diags {
        writeln!(
            w,
            "{}:{}:{} {} {}",
            d.path, d.line, d.col, d.code, d.message
        )?;
        if let Some(fix) = &d.fix {
            writeln!(w, "    fix: {fix}")?;
        }
    }
    Ok(())
}

fn emit_json(diags: &[Diagnostic], w: &mut impl Write) -> std::io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, diags)?;
    writeln!(w)
}

fn emit_sarif(
    diags: &[Diagnostic],
    enabled: &HashSet<&str>,
    w: &mut impl Write,
) -> std::io::Result<()> {
    let rules: Vec<_> = RULES
        .iter()
        .filter(|r| enabled.contains(r.code))
        .map(|r| {
            serde_json::json!({
                "id": r.code,
                "shortDescription": { "text": r.name },
                "helpUri": format!(
                    "https://github.com/mgiovani/stopslop/blob/main/README.md#{}",
                    r.code.to_lowercase()
                ),
            })
        })
        .collect();

    let results: Vec<_> = diags
        .iter()
        .map(|d| {
            let level = match d.tier {
                Tier::A => "error",
                Tier::B => "warning",
            };
            let text = match &d.fix {
                Some(fix) => format!("{} (fix: {fix})", d.message),
                None => d.message.clone(),
            };
            serde_json::json!({
                "ruleId": d.code,
                "level": level,
                "message": { "text": text },
                "locations": [ {
                    "physicalLocation": {
                        "artifactLocation": { "uri": strip_dot_slash(&d.path) },
                        "region": { "startLine": d.line, "startColumn": d.col },
                    }
                } ],
            })
        })
        .collect();

    let doc = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [ {
            "tool": {
                "driver": {
                    "name": "stopslop",
                    "informationUri": "https://github.com/mgiovani/stopslop",
                    "rules": rules,
                }
            },
            "results": results,
        } ],
    });
    serde_json::to_writer_pretty(&mut *w, &doc)?;
    writeln!(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sarif_uri_is_repo_root_relative() {
        let diags = vec![Diagnostic {
            code: "SLOP018",
            name: "test",
            tier: Tier::B,
            path: "./README.md".to_string(),
            line: 1,
            col: 1,
            message: "test".into(),
            fix: None,
        }];
        let mut out = Vec::new();
        emit_sarif(&diags, &HashSet::new(), &mut out).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            doc["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
                ["uri"],
            "README.md"
        );
    }

    fn diag(code: &'static str, tier: Tier, path: &str) -> Diagnostic {
        Diagnostic {
            code,
            name: "test",
            tier,
            path: path.to_string(),
            line: 1,
            col: 2,
            message: "msg".into(),
            fix: None,
        }
    }

    fn markdown_of(diags: &[Diagnostic]) -> String {
        let mut out = Vec::new();
        emit_markdown(diags, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn markdown_groups_by_tier_with_blocking_first() {
        let body = markdown_of(&[
            diag("SLOP018", Tier::B, "./a.md"),
            diag("SLOP001", Tier::A, "./b.rs"),
        ]);
        let a = body.find("Tier A").expect("Tier A heading");
        let b = body.find("Tier B").expect("Tier B heading");
        assert!(a < b, "blocking tier must come first:\n{body}");
        assert!(body.contains("`b.rs:1:2` **SLOP001**"), "{body}");
        assert!(body.contains("`a.md:1:2` **SLOP018**"), "{body}");
    }

    /// A tier with no findings must not print an empty heading -- a "Tier A" header over nothing
    /// reads as a build failure.
    #[test]
    fn markdown_omits_a_tier_with_no_findings() {
        let body = markdown_of(&[diag("SLOP018", Tier::B, "./a.md")]);
        assert!(!body.contains("Tier A"), "{body}");
        assert!(body.contains("Tier B"), "{body}");
    }

    #[test]
    fn markdown_of_nothing_is_one_line() {
        assert_eq!(markdown_of(&[]), "No stopslop findings.\n");
    }
}
