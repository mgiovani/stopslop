use crate::{
    cli::Format,
    diagnostic::{Diagnostic, Tier},
    paths::strip_dot_slash,
    registry::RULES,
    walk::Stats,
};
use std::collections::HashSet;
use std::io::Write;

pub fn emit(
    format: Format,
    diags: &[Diagnostic],
    enabled: &HashSet<&str>,
    stats: Option<&Stats>,
    w: &mut impl Write,
) -> std::io::Result<()> {
    match format {
        Format::Text => emit_text(diags, w),
        Format::Json => emit_json(diags, stats, w),
        Format::Sarif => emit_sarif(diags, enabled, stats, w),
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
        (Tier::C, "Tier C -- experimental, opt-in and never gating"),
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

fn emit_json(
    diags: &[Diagnostic],
    stats: Option<&Stats>,
    w: &mut impl Write,
) -> std::io::Result<()> {
    match stats {
        None => serde_json::to_writer_pretty(&mut *w, diags)?,
        Some(stats) => {
            let doc = serde_json::json!({ "findings": diags, "stats": stats });
            serde_json::to_writer_pretty(&mut *w, &doc)?;
        }
    }
    writeln!(w)
}

fn emit_sarif(
    diags: &[Diagnostic],
    enabled: &HashSet<&str>,
    stats: Option<&Stats>,
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
            // SARIF 2.1.0 restricts result.level to none/note/warning/error, so Tier C maps
            // to `note` -- GitHub's `notice` is a workflow-command level and is invalid here.
            let level = match d.tier {
                Tier::A => "error",
                Tier::B => "warning",
                Tier::C => "note",
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

    let mut doc = serde_json::json!({
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
    if let Some(stats) = stats {
        doc["runs"][0]["properties"] = serde_json::json!({ "stats": stats });
    }
    serde_json::to_writer_pretty(&mut *w, &doc)?;
    writeln!(w)
}

pub fn render_stats(s: &Stats) -> String {
    let mut out = String::from("\n");
    for (label, value) in [
        ("files", commas(s.files)),
        ("skipped", commas(s.skipped)),
        ("lines", commas(s.lines)),
        ("wall", format!("{:.3}s", s.wall_secs)),
    ] {
        out.push_str(&format!("  {:<10}{:>10}\n", label, value));
    }
    out.push_str(&format!(
        "  {:<10}{:>10} lines/s\n",
        "rate",
        commas(s.lines_per_sec)
    ));
    out
}

fn commas(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
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
        emit_sarif(&diags, &HashSet::new(), None, &mut out).unwrap();
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

    #[test]
    fn commas_groups_thousands() {
        assert_eq!(commas(0), "0");
        assert_eq!(commas(999), "999");
        assert_eq!(commas(1000), "1,000");
        assert_eq!(commas(15918), "15,918");
        assert_eq!(commas(1234567), "1,234,567");
    }

    #[test]
    fn json_without_stats_is_a_flat_array() {
        let mut out = Vec::new();
        emit_json(&[diag("SLOP018", Tier::B, "./a.md")], None, &mut out).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(doc.is_array());
    }

    #[test]
    fn json_with_stats_wraps_findings_and_stats() {
        let stats = Stats {
            files: 3,
            skipped: 1,
            lines: 42,
            wall_secs: 0.1,
            lines_per_sec: 420,
        };
        let mut out = Vec::new();
        emit_json(
            &[diag("SLOP018", Tier::B, "./a.md")],
            Some(&stats),
            &mut out,
        )
        .unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(doc["findings"].is_array());
        assert_eq!(doc["stats"]["files"], 3);
        assert_eq!(doc["stats"]["skipped"], 1);
        assert_eq!(doc["stats"]["lines"], 42);
    }

    /// SARIF 2.1.0 restricts `result.level` to none/note/warning/error. A value outside that
    /// set (GitHub's workflow-command `notice`, say) is silently dropped by code scanning, so
    /// the mapping is pinned here rather than left to the next reader to re-derive.
    #[test]
    fn sarif_level_uses_only_spec_valid_values() {
        let diags = [
            diag("SLOP001", Tier::A, "./a.rs"),
            diag("SLOP018", Tier::B, "./b.md"),
            diag("SLOP045", Tier::C, "./c.rs"),
        ];
        let mut out = Vec::new();
        emit_sarif(&diags, &HashSet::new(), None, &mut out).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let levels: Vec<&str> = doc["runs"][0]["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["level"].as_str().unwrap())
            .collect();
        assert_eq!(levels, ["error", "warning", "note"]);
        for level in levels {
            assert!(
                ["none", "note", "warning", "error"].contains(&level),
                "{level} is not a SARIF 2.1.0 result.level"
            );
        }
    }

    #[test]
    fn sarif_with_stats_lands_in_run_properties() {
        let stats = Stats {
            files: 3,
            skipped: 1,
            lines: 42,
            wall_secs: 0.1,
            lines_per_sec: 420,
        };
        let mut with = Vec::new();
        emit_sarif(&[], &HashSet::new(), Some(&stats), &mut with).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&with).unwrap();
        assert_eq!(doc["runs"][0]["properties"]["stats"]["lines"], 42);

        let mut without = Vec::new();
        emit_sarif(&[], &HashSet::new(), None, &mut without).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&without).unwrap();
        assert!(doc["runs"][0].get("properties").is_none());
    }

    #[test]
    fn text_output_ignores_stats() {
        let diags = [diag("SLOP018", Tier::B, "./a.md")];
        let stats = Stats {
            files: 3,
            skipped: 1,
            lines: 42,
            wall_secs: 0.1,
            lines_per_sec: 420,
        };
        let mut with = Vec::new();
        emit(
            Format::Text,
            &diags,
            &HashSet::new(),
            Some(&stats),
            &mut with,
        )
        .unwrap();
        let mut without = Vec::new();
        emit(Format::Text, &diags, &HashSet::new(), None, &mut without).unwrap();
        assert_eq!(with, without);
    }

    #[test]
    fn render_stats_layout() {
        let stats = Stats {
            files: 230,
            skipped: 12,
            lines: 15918,
            wall_secs: 0.092,
            lines_per_sec: 172241,
        };
        assert_eq!(
            render_stats(&stats),
            "\n  files            230\n  skipped           12\n  lines         15,918\n  wall          0.092s\n  rate         172,241 lines/s\n"
        );
    }
}
