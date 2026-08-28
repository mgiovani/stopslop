use serde::Serialize;

/// Severity ONLY. `A` exits 1 (blocks CI); `B` is advisory and never affects the exit code.
/// Independent of `RuleDef::default_on` -- a rule can be Tier B (advisory) and still on by
/// default (a judgment-call rule you want surfaced but never gating), so tier and default-on
/// are two separate axes, not a 1:1 pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Tier {
    A,
    B,
}

impl Tier {
    /// Shared by `[[custom-rule]] tier` and the `fail-on-tier` setting so the accepted spellings
    /// can't drift apart between them.
    pub fn parse(s: &str) -> Option<Tier> {
        match s {
            "A" => Some(Tier::A),
            "B" => Some(Tier::B),
            _ => None,
        }
    }

    /// True when `self` is at least as severe as `floor`. A is the more severe tier, so
    /// `fail-on-tier = "B"` admits both tiers and `"A"` admits only A.
    pub fn at_least_as_severe_as(self, floor: Tier) -> bool {
        matches!((self, floor), (Tier::A, _) | (Tier::B, Tier::B))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: &'static str, // "SLOP001"
    pub name: &'static str, // human name
    pub tier: Tier,
    pub path: String, // display path: relative to cwd, forward-slash
    pub line: usize,  // 1-based
    pub col: usize,   // 1-based
    pub message: String,
    /// Concrete "write this instead" hint, when the rule knows a specific replacement
    /// (`leverage` -> `use`, a hand-rolled clone -> `structuredClone`). Density rules that
    /// flag a document-wide pattern rather than one substitutable span leave this `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Diagnostic {
    pub fn at(
        rule: &crate::registry::RuleDef,
        ctx: &crate::context::LintContext,
        line: usize,
        col: usize,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            code: rule.code,
            name: rule.name,
            tier: rule.tier,
            path: ctx.display_path.clone(),
            line,
            col,
            message: message.into(),
            fix: None,
        }
    }
    /// Same as `at`, plus a concrete replacement hint rendered under the finding.
    pub fn at_fix(
        rule: &crate::registry::RuleDef,
        ctx: &crate::context::LintContext,
        line: usize,
        col: usize,
        message: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Diagnostic {
            fix: Some(fix.into()),
            ..Diagnostic::at(rule, ctx, line, col, message)
        }
    }
    pub fn sort_key(&self) -> (&str, usize, usize, &str) {
        (self.path.as_str(), self.line, self.col, self.code)
    }
}
