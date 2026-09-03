use std::sync::LazyLock;

use regex::Regex;

use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, NatLang};
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP009",
    name: "Placeholder / sample credential value",
    tier: Tier::A,
    // Every code lang's string literals, plus HTML attribute values (`ctx.strings` carries
    // `name="value"` there, see `ProseDoc::attr_values`).
    langs: &[
        Lang::Ts,
        Lang::Tsx,
        Lang::Python,
        Lang::Go,
        Lang::Rust,
        Lang::Html,
    ],
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: true,
    check,
};

const MESSAGE: &str = "hardcoded sample/credential value";

// This file's own pattern below spells out several of the trigger tokens it describes, so
// dogfooding stopslop against its own src/ would self-flag it.
static RE_CI: LazyLock<Regex> = LazyLock::new(|| {
    // ai-slop-ignore
    Regex::new(r"(?i)(?-u:\b)YOUR_[A-Z0-9_]+|<your[ -][^>]*>|example\.(com|org|net)|123[- ]?456[- ]?7890|John Doe|Jane Doe|foo@bar\.|user@example\.|change[_ ]?me").unwrap()
});
// The credential patterns are prefixes and character classes, never a literal sample value, so
// this one needs no suppression -- SLOP009 has nothing to match here.
static RE_CS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"sk-[A-Za-z0-9]{16,}|AKIA[0-9A-Z]{12,}|ghp_[A-Za-z0-9]{20,}|xox[baprs]-").unwrap()
});
/// HTML attributes: the four placeholder-image generators, and an `alt` whose whole value is a
/// generic word (AccessGuru and A11YN both report generic alt text as the recurring defect in
/// generated markup). `picsum.photos` serves real photographs for demos and is not a placeholder;
/// `alt=""` is the correct markup for a decorative image and never matches. Each string is one
/// whole `name="value"` attribute (`ProseDoc::attr_values`), so `^alt=` cannot hit `data-alt=`.
static RE_HTML: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)via\.placeholder\.com|placehold\.co|placekitten\.com|dummyimage\.com|^alt=["'](?:image|img|photo|picture|placeholder|image description|alt text|description)["']$"#).unwrap()
});
/// Brazilian-Portuguese twin of `RE_CI`. The CPF shapes are the sample values a placeholder
/// leaves behind (a repeated digit, or the textbook sequential `123.456.789-XX`), never a
/// general CPF regex -- this rule names placeholders, not real personal data, so it only matches
/// the handful of values every Brazilian dev recognizes as fake. "Maria Silva" without "da" is a
/// real, ordinary name and is excluded on purpose; requiring "da silva" narrows the match to the
/// specific stock full name. "Fulano/Ciclano/Beltrano" are the Portuguese "John Doe"/"Jane Doe".
static RE_CI_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)SEU_[A-Z0-9_]+|(?-u:\b)SUA_[A-Z0-9_]+|<(?:seu|sua)[ -][^>]*>|exemplo\.(?:com|org|net)|(?:jo[ãa]o|maria|jos[ée]) da silva(?-u:\b)|(?-u:\b)(?:fulan|ciclan|beltran)[oa](?-u:\b)|usu[áa]rio@exemplo\.|(?:mude|altere|troque)[_ -]?(?:me|aqui|isso)(?-u:\b)|(?-u:\b)(?:0{3}\.0{3}\.0{3}-00|1{3}\.1{3}\.1{3}-11|2{3}\.2{3}\.2{3}-22|3{3}\.3{3}\.3{3}-33|4{3}\.4{3}\.4{3}-44|5{3}\.5{3}\.5{3}-55|6{3}\.6{3}\.6{3}-66|7{3}\.7{3}\.7{3}-77|8{3}\.8{3}\.8{3}-88|9{3}\.9{3}\.9{3}-99|123\.456\.789-\d{2}|123456789\d{2})(?-u:\b)").unwrap()
});
/// Brazilian-Portuguese twin of `RE_HTML`'s generic-`alt` half. Applied at the same
/// `ProseDoc::attr_values` path as `RE_HTML`.
static RE_HTML_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^alt=["'](?:imagem|foto|fotografia|figura|ilustra[çc][ãa]o|descri[çc][ãa]o(?: da imagem)?|texto alternativo)["']$"#).unwrap()
});

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let en = ctx.natlangs.contains(&NatLang::En);
    let pt = ctx.natlangs.contains(&NatLang::PtBr);
    for s in ctx.strings {
        if s.is_doc {
            continue;
        }
        let hit = (en && (RE_CI.is_match(s.text) || RE_HTML.is_match(s.text)))
            || (pt && (RE_CI_PT_BR.is_match(s.text) || RE_HTML_PT_BR.is_match(s.text)))
            || RE_CS.is_match(s.text);
        if hit {
            out.push(Diagnostic::at(rule, ctx, s.line, s.col, MESSAGE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_your_api_key() {
        assert!(RE_CI.is_match("YOUR_API_KEY")); // ai-slop-ignore
    }

    #[test]
    fn your_inside_a_filename_token_is_not_a_placeholder() {
        assert!(!RE_CI.is_match("/files/Leave_Your_Dog_at_Home_600x.png"));
        assert!(RE_CI.is_match("/files/YOUR_LOGO_HERE.png")); // ai-slop-ignore
    }

    #[test]
    fn flags_example_domain() {
        assert!(RE_CI.is_match("https://example.com/api")); // ai-slop-ignore
    }

    #[test]
    fn flags_john_doe() {
        assert!(RE_CI.is_match("John Doe")); // ai-slop-ignore
    }

    #[test]
    fn flags_changeme() {
        assert!(RE_CI.is_match("change_me")); // ai-slop-ignore
    }

    #[test]
    fn flags_stripe_secret_shape() {
        assert!(RE_CS.is_match("sk-abcdefghijklmnop1234")); // ai-slop-ignore
    }

    #[test]
    fn flags_aws_key_shape() {
        assert!(RE_CS.is_match("AKIAIOSFODNN7EXAMPLE")); // ai-slop-ignore
    }

    #[test]
    fn flags_github_token_shape() {
        assert!(RE_CS.is_match("ghp_abcdefghijklmnopqrstuvwxyz012345")); // ai-slop-ignore
    }

    #[test]
    fn clean_production_url_not_flagged() {
        assert!(
            !RE_CI.is_match("https://api.production.com")
                && !RE_CS.is_match("https://api.production.com")
        );
    }

    #[test]
    fn flags_placeholder_image_host_and_generic_alt() {
        assert!(RE_HTML.is_match("src=\"https://via.placeholder.com/150\"")); // ai-slop-ignore
        assert!(RE_HTML.is_match("src=\"https://placehold.co/600x400\"")); // ai-slop-ignore
        assert!(RE_HTML.is_match("alt=\"image\""));
        assert!(RE_HTML.is_match("alt='Image description'"));
        assert!(RE_HTML.is_match("ALT=\"IMAGE\""));
    }

    #[test]
    fn clean_decorative_descriptive_alt_and_demo_photo_host() {
        assert!(!RE_HTML.is_match("alt=\"\""));
        assert!(!RE_HTML.is_match("alt=\"image of the office lobby\""));
        assert!(!RE_HTML.is_match("src=\"https://picsum.photos/200\""));
        assert!(!RE_HTML.is_match("data-alt=\"image\""));
        assert!(!RE_HTML.is_match("title=\"image\""));
    }

    #[test]
    fn clean_specific_type_cast_like_string_not_flagged() {
        assert!(!RE_CI.is_match("hello world") && !RE_CS.is_match("hello world"));
    }

    #[test]
    fn flags_pt_br_placeholder_tokens() {
        assert!(RE_CI_PT_BR.is_match("SEU_TOKEN_AQUI")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("SUA_CHAVE_API")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("<seu-nome-aqui>")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("contato@exemplo.com")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("João da Silva")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("Maria da Silva")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("Fulano")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("Ciclana")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("usuario@exemplo.com")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("troque_me")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("111.111.111-11")); // ai-slop-ignore
        assert!(RE_CI_PT_BR.is_match("123.456.789-00")); // ai-slop-ignore
    }

    #[test]
    fn maria_silva_without_da_is_a_real_name() {
        // "Maria Silva" -- no "da" between the names -- is an ordinary real name, not the stock
        // placeholder full name.
        assert!(!RE_CI_PT_BR.is_match("Maria Silva"));
    }

    #[test]
    fn real_looking_cpf_is_not_a_placeholder() {
        // Neither a repeated digit nor the textbook sequential value -- a real CPF must not fire,
        // since this panel names sample/placeholder values, not general PII.
        assert!(!RE_CI_PT_BR.is_match("529.982.247-25"));
    }

    #[test]
    fn flags_pt_br_generic_alt() {
        assert!(RE_HTML_PT_BR.is_match("alt=\"imagem\""));
        assert!(RE_HTML_PT_BR.is_match("alt='Foto'"));
        assert!(RE_HTML_PT_BR.is_match("alt=\"descrição da imagem\""));
    }

    #[test]
    fn clean_pt_br_descriptive_alt() {
        assert!(!RE_HTML_PT_BR.is_match("alt=\"foto da equipe reunida no escritório\""));
    }

    fn diagnostics_for_natlangs(src: &str, natlangs: &'static [NatLang]) -> Vec<Diagnostic> {
        use tree_sitter::Parser;
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(Lang::Ts)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, strings, index) = crate::context::extract(&tree, src, Lang::Ts);
        let ctx = LintContext {
            display_path: "test.ts".to_string(),
            source: src,
            index: Some(&index),
            lang: Lang::Ts,
            comments: &comments,
            strings: &strings,
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            natlangs,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn pt_br_gate_silences_portuguese_panel_when_only_english_selected() {
        let src_pt = "const nome = \"João da Silva\";\n"; // ai-slop-ignore
        assert!(diagnostics_for_natlangs(src_pt, &[NatLang::En]).is_empty());

        let src_en = "const key = \"YOUR_API_KEY\";\n"; // ai-slop-ignore
        assert!(diagnostics_for_natlangs(src_en, &[NatLang::PtBr]).is_empty());
    }

    #[test]
    fn credential_shapes_fire_regardless_of_natlangs() {
        let src = "const key = \"sk-abcdefghijklmnop1234\";\n"; // ai-slop-ignore
        assert_eq!(diagnostics_for_natlangs(src, &[NatLang::En]).len(), 1);
        assert_eq!(diagnostics_for_natlangs(src, &[NatLang::PtBr]).len(), 1);
    }

    #[test]
    fn pt_br_panel_fires_through_check_under_default_union() {
        let src = "const nome = \"João da Silva\";\n"; // ai-slop-ignore
        let diags = diagnostics_for_natlangs(src, crate::lang::ALL_NATLANGS);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP009");
    }
}
