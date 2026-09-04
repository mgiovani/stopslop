use std::path::Path;
use tree_sitter::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Html,
    /// One `Lang` for four container formats (PNG/JPEG/WebP, gated by `IMAGE_LANGS`) because the
    /// tell a rule reads is the embedded-metadata signal (a generator string, a ComfyUI workflow
    /// graph, a C2PA claim), not the container byte layout. A rule that must distinguish reads
    /// `ImageDoc::format`, resolved from the magic bytes by `image::ImageDoc::parse`, the same
    /// way a code rule reads `ctx.lang` only when one branch needs it.
    Image,
}

pub const CODE_LANGS: &[Lang] = &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust];
pub const PROSE_LANGS: &[Lang] = &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst, Lang::Html];
pub const IMAGE_LANGS: &[Lang] = &[Lang::Image];

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
            "html" | "htm" => Some(Lang::Html),
            "png" | "jpg" | "jpeg" | "webp" => Some(Lang::Image),
            _ => None,
        }
    }

    /// Prose langs take the `ProseDoc` path in `engine::lint_file` instead of `context::extract`.
    /// The Markdown family never touches tree-sitter; HTML parses with tree-sitter-html inside
    /// `ProseDoc::parse_html` to find its visible text.
    pub fn is_prose(self) -> bool {
        matches!(
            self,
            Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst | Lang::Html
        )
    }

    /// Images take neither the AST path (`context::extract`) nor the `ProseDoc` path: they carry
    /// no text stream at all, so `engine::lint_image` builds a `LintContext` straight from
    /// `image::ImageDoc::parse` instead.
    pub fn is_image(self) -> bool {
        matches!(self, Lang::Image)
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
        Lang::Html => tree_sitter_html::LANGUAGE.into(),
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst | Lang::Image => {
            unreachable!("markdown-family prose langs and images bypass tree-sitter")
        }
    }
}

/// Natural-language axis, orthogonal to `Lang` (file syntax): a `.md` file's `Lang` is always
/// `Md` whether its prose is English or Portuguese. `natlangs` on a `RuleDef` means "this rule's
/// lexicon is validated by a fixture in that language" -- a rule with no natural-language
/// lexicon (AST shape, punctuation, statistics) declares every language instead of narrowing.
/// v1 ships one Portuguese lexicon, tuned on Brazilian text; `pt-PT` resolves to the same
/// `PtBr` variant as a best-effort alias rather than a separate lexicon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NatLang {
    En,
    PtBr,
}

pub const ALL_NATLANGS: &[NatLang] = &[NatLang::En, NatLang::PtBr];

impl NatLang {
    /// Matches a BCP-47-ish tag by its primary subtag alone, case-insensitively, accepting `-`
    /// or `_` as the subtag separator (`en-US`, `en_GB`, `pt-BR`, `pt-PT`, `pt_br` all resolve).
    /// Anything outside `en`/`pt` is `None` rather than a guess, so config validation can turn
    /// it into a startup error naming the supported tags instead of silently picking one.
    pub fn from_tag(tag: &str) -> Option<NatLang> {
        let primary = tag.split(['-', '_']).next()?;
        match primary.to_ascii_lowercase().as_str() {
            "en" => Some(NatLang::En),
            "pt" => Some(NatLang::PtBr),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            NatLang::En => "en",
            NatLang::PtBr => "pt-BR",
        }
    }
}

#[cfg(test)]
mod natlang_tests {
    use super::*;

    #[test]
    fn from_tag_matches_primary_subtag_case_insensitively() {
        for tag in ["en", "EN", "en-US", "en_GB", "En-us"] {
            assert_eq!(NatLang::from_tag(tag), Some(NatLang::En), "{tag}");
        }
        for tag in ["pt", "PT", "pt-BR", "pt-PT", "pt_br"] {
            assert_eq!(NatLang::from_tag(tag), Some(NatLang::PtBr), "{tag}");
        }
    }

    #[test]
    fn from_tag_rejects_unknown_or_empty_tags() {
        assert_eq!(NatLang::from_tag("fr"), None);
        assert_eq!(NatLang::from_tag("es-MX"), None);
        assert_eq!(NatLang::from_tag(""), None);
    }

    #[test]
    fn label_matches_display_form() {
        assert_eq!(NatLang::En.label(), "en");
        assert_eq!(NatLang::PtBr.label(), "pt-BR");
    }
}
