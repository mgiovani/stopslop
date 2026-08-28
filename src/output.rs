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
    }
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
}
