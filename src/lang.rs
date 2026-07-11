use std::path::Path;
use tree_sitter::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ts,
    Tsx,
    Python,
    Go,
    Rust,
}

impl Lang {
    pub fn from_path(p: &Path) -> Option<Lang> {
        match p.extension()?.to_str()? {
            "ts" | "mts" | "cts" => Some(Lang::Ts),
            "tsx" => Some(Lang::Tsx),
            "py" | "pyi" => Some(Lang::Python),
            "go" => Some(Lang::Go),
            "rs" => Some(Lang::Rust),
            _ => None,
        }
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
    }
}
