//! Named rule groups. Every rule code belongs to exactly one group, and a group name is accepted
//! anywhere a rule code or code prefix is (`--select`, `--ignore`, and the `select`/`ignore` keys
//! in `stopslop.toml`). `SLOP0NN` numbering is chronological, not thematic, so numeric prefixes
//! alone can't express "just the rhetoric rules" -- that's what these names are for.

/// (group name, member codes). The `groups_partition_every_rule` test below enforces that this
/// table stays exhaustive and non-overlapping, so adding a rule without grouping it fails CI.
pub static GROUPS: &[(&str, &[&str])] = &[
    // Mechanical leftovers from a generation session: chat turns, tool tokens, unfilled slots.
    (
        "artifact",
        &[
            "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP011", "SLOP012", "SLOP013", "SLOP044",
        ],
    ),
    // Structural code smells: swallowed errors, escaped types, stubs, speculative abstraction.
    (
        "structure",
        &[
            "SLOP005", "SLOP006", "SLOP007", "SLOP008", "SLOP009", "SLOP010", "SLOP039", "SLOP040",
        ],
    ),
    // Code that rebuilds something the standard library or the platform already provides.
    ("stdlib", &["SLOP037", "SLOP038"]),
    // Formulaic rhetorical shapes: clichés, staged reveals, manufactured significance.
    (
        "rhetoric",
        &[
            "SLOP014", "SLOP017", "SLOP022", "SLOP023", "SLOP024", "SLOP026", "SLOP029", "SLOP030",
            "SLOP031", "SLOP035", "SLOP036",
        ],
    ),
    // Words that cost the reader something and return nothing: hedging, filler, padding.
    (
        "verbosity",
        &[
            "SLOP015", "SLOP016", "SLOP027", "SLOP028", "SLOP032", "SLOP033", "SLOP034", "SLOP041",
            "SLOP042", "SLOP043",
        ],
    ),
    // Claims with no checkable source behind them.
    ("sourcing", &["SLOP025"]),
    // Typographic and Markdown affectations.
    ("format", &["SLOP018", "SLOP019", "SLOP020", "SLOP021"]),
    // Embedded image metadata that names how or by what an image was made.
    ("provenance", &["SLOP045", "SLOP046", "SLOP047"]),
];

/// The group a code belongs to, for `--list-rules`. `SLOP9NN` codes are user-defined
/// (`crate::custom`), special-cased here the same way `ALL` is special-cased in `expand` below --
/// they can never join the `GROUPS` table itself because `groups_partition_every_rule` requires
/// every member to also be a `RULES` entry, and custom codes aren't.
pub fn group_of(code: &str) -> &'static str {
    if code.starts_with("SLOP9") {
        return "custom";
    }
    GROUPS
        .iter()
        .find(|(_, codes)| codes.contains(&code))
        .map(|(name, _)| *name)
        .unwrap_or("ungrouped")
}

/// Replaces every group name in `pats` with that group's member codes; non-group patterns pass
/// through untouched, so codes and prefixes keep working exactly as before. `ALL` is a reserved
/// selector (not a `GROUPS` entry -- that table's partition test requires every code to live in
/// exactly one group, and `ALL` deliberately spans all of them) that expands to every registered
/// rule code.
pub fn expand(pats: &[String]) -> Vec<String> {
    pats.iter()
        .flat_map(|p| {
            if p == "ALL" {
                return crate::registry::RULES
                    .iter()
                    .map(|r| r.code.to_string())
                    .collect::<Vec<_>>();
            }
            match GROUPS.iter().find(|(name, _)| name == p) {
                Some((_, codes)) => codes.iter().map(|c| c.to_string()).collect(),
                None => vec![p.clone()],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RULES;
    use std::collections::HashSet;

    /// Exhaustive and non-overlapping: every registered rule sits in exactly one group. A new rule
    /// that nobody grouped, or a code pasted into two groups, fails here rather than silently
    /// escaping `--select <group>`.
    #[test]
    fn groups_partition_every_rule() {
        let mut seen: HashSet<&str> = HashSet::new();
        for (name, codes) in GROUPS {
            for code in *codes {
                assert!(
                    seen.insert(code),
                    "{code} appears in more than one group ({name})"
                );
                assert!(
                    RULES.iter().any(|r| r.code == *code),
                    "group {name} lists unknown rule {code}"
                );
            }
        }
        for r in RULES {
            assert!(seen.contains(r.code), "rule {} is in no group", r.code);
        }
    }

    #[test]
    fn expand_replaces_group_names_and_passes_codes_through() {
        assert_eq!(expand(&["sourcing".to_string()]), vec!["SLOP025"]);
        assert_eq!(expand(&["SLOP001".to_string()]), vec!["SLOP001"]);
        assert_eq!(expand(&["SLOP".to_string()]), vec!["SLOP"]);
    }

    #[test]
    fn group_of_slop9_prefix_is_custom() {
        assert_eq!(group_of("SLOP900"), "custom");
        assert_eq!(group_of("SLOP999"), "custom");
    }

    #[test]
    fn expand_all_returns_every_rule_code() {
        let codes = expand(&["ALL".to_string()]);
        assert_eq!(codes.len(), RULES.len());
        for r in RULES {
            assert!(codes.contains(&r.code.to_string()));
        }
    }
}
