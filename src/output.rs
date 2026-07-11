use crate::{
    cli::Format,
    diagnostic::{Diagnostic, Tier},
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
    }
}

fn emit_text(diags: &[Diagnostic], w: &mut impl Write) -> std::io::Result<()> {
    for d in diags {
        writeln!(
            w,
            "{}:{}:{} {} {}",
            d.path, d.line, d.col, d.code, d.message
        )?;
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
                    "https://github.com/mgiovani/ai-stop-slop/blob/main/README.md#{}",
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
            serde_json::json!({
                "ruleId": d.code,
                "level": level,
                "message": { "text": d.message },
                "locations": [ {
                    "physicalLocation": {
                        "artifactLocation": { "uri": d.path },
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
                    "informationUri": "https://github.com/mgiovani/ai-stop-slop",
                    "rules": rules,
                }
            },
            "results": results,
        } ],
    });
    serde_json::to_writer_pretty(&mut *w, &doc)?;
    writeln!(w)
}
