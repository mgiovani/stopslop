use std::path::Path;
use tree_sitter::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ts,
    Tsx,
    Python,
    Go,
    Rust,
    Md,
    Mdx,
    Txt,
    Rst,
}

impl Lang {
    pub fn from_path(p: &Path) -> Option<Lang> {
        match p.extension()?.to_str()? {
            "ts" | "mts" | "cts" => Some(Lang::Ts),
            "tsx" => Some(Lang::Tsx),
            "py" | "pyi" => Some(Lang::Python),
            "go" => Some(Lang::Go),
            "rs" => Some(Lang::Rust),
            "md" | "markdown" => Some(Lang::Md),
            "mdx" => Some(Lang::Mdx),
            "txt" | "text" => Some(Lang::Txt),
            "rst" => Some(Lang::Rst),
            _ => None,
        }
    }

    /// Prose langs bypass tree-sitter entirely (see engine::lint_file).
    pub fn is_prose(self) -> bool {
        matches!(self, Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst)
    }
}

/// THE ONLY place that touches grammar-crate symbols. If Cargo resolves tree-sitter 0.21
/// instead of 0.23, this fn and the `set_language(&..)` call in engine.rs are the ONLY edits
/// (0.21: replace `X::LANGUAGE.into()` with `X::language()`, and pass by value).
pub fn ts_language(lang: Lang) -> Language {
    match lang {
        Lang::Ts => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => {
            unreachable!("prose langs bypass tree-sitter")
        }
    }
}
