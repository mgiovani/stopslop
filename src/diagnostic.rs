use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Tier {
    A,
    B,
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
        }
    }
    pub fn sort_key(&self) -> (&str, usize, usize, &str) {
        (self.path.as_str(), self.line, self.col, self.code)
    }
}
