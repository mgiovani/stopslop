use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::{first_byte_per_line, ProseDoc};
use crate::prose_words::REASONING_CHAIN_FRAGMENT;
use crate::registry::RuleDef;
use crate::rules::fragmentation;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP011",
    name: "Assistant-response residue in prose",
    tier: Tier::A,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// Groups A/B/E (self-ID / knowledge-cutoff disclaimer / speculative gap-filling, refusal
/// boilerplate, reviewer-submission leakage). Anchored anywhere in the masked prose stream --
/// this exact phrasing has no legitimate reason to appear in finished, edited prose. The
/// speculative-gap-filling family below (`maintains a low profile`, `not publicly available`, ...)
/// is the shape a model falls back on when it has no real biographical fact to report.
static RE_ANYWHERE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)as an? (ai|large) language model(?-u:\b)|(?-u:\b)as an ai (assistant|model)(?-u:\b)|(?-u:\b)as of my (last|latest|most recent) (knowledge|training) (update|cutoff|data)(?-u:\b)|(?-u:\b)(up to|as of) my last training update(?-u:\b)|(?-u:\b)my (knowledge|training) cutoff(?-u:\b)|(?-u:\b)I (do not|don'?t) have (access to|the ability to browse) (real-?time|the internet|current)(?-u:\b)|(?-u:\b)I (cannot|can'?t) browse the internet(?-u:\b)|(?-u:\b)while specific details (are|remain) (limited|scarce|unavailable)(?-u:\b)|(?-u:\b)in the (provided|available) (search results|sources)(?-u:\b)|(?-u:\b)based on (the )?available information(?-u:\b)|(?-u:\b)I'?m (sorry|unable)[, ].{0,40}(?-u:\b)(cannot|can'?t|unable to) (assist|help|provide|generate)(?-u:\b)|(?-u:\b)I cannot generate content that(?-u:\b)|(?-u:\b)I'?m unable to assist with that request(?-u:\b)|(?-u:\b)reviewer note(?-u:\b)|(?-u:\b)i hope this message finds you well(?-u:\b)|(?-u:\b)thank you for your review(?-u:\b)|(?-u:\b)please find (our|the) revised(?-u:\b)|(?-u:\b)we remain committed to creating content that aligns with(?-u:\b)|(?-u:\b)maintains? a low profile(?-u:\b)|(?-u:\b)keeps? (his|her|their) personal (life|details) private(?-u:\b)|(?-u:\b)prefers? to stay out of the spotlight(?-u:\b)|(?-u:\b)likely (grew up|studied|began|started)(?-u:\b)|(?-u:\b)not publicly available(?-u:\b)").unwrap()
});

/// Portuguese self-ID / knowledge-cutoff / refusal panel, mirroring `RE_ANYWHERE`'s groups A/B/E.
/// The article before "inteligência artificial"/"modelo de linguagem" is required: a corpus check
/// found "conhecidas como inteligência artificial forte" ("known as strong AI", a description,
/// not self-ID). The refusal alternative needs an apology in front AND a request-shaped object
/// after the verb, like the English `I'm sorry ... cannot assist`: a bare apology followed by
/// unrelated text ("Desculpe, mas não posso ajudar hoje, estou de férias") is ordinary human
/// correspondence, not a chat refusal. Tier A, so nothing here is a phrase a human would write in
/// a finished document. ASCII-only panel, so `(?-u:\b)` is safe (issue #21); `atualiza[çc][ãa]o`
/// ends on the literal `o` after the class, so the trailing boundary still lands on ASCII.
static RE_ANYWHERE_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)como um modelo de linguagem(?-u:\b)|(?-u:\b)como uma intelig[êe]ncia artificial(?-u:\b)|(?-u:\b)como um assistente (virtual|de ia)(?-u:\b)|(?-u:\b)n[ãa]o tenho acesso a (dados|informa[çc][õo]es) em tempo real(?-u:\b)|(?-u:\b)minha data de corte(?-u:\b)|(?-u:\b)meus dados de treinamento(?-u:\b)|(?-u:\b)at[ée] a minha [úu]ltima atualiza[çc][ãa]o(?-u:\b)|(?-u:\b)(desculpe|infelizmente|sinto muito)[, ][^.!?\n]{0,40}n[ãa]o (posso|consigo) (ajudar com|atender|fornecer|gerar|criar) (isso|essa solicita[çc][ãa]o|esse pedido|esse tipo de conte[úu]do)(?-u:\b)").unwrap()
});

/// Reasoning-chain leakage: chain-of-thought scaffolding left behind in a deliverable ("let's
/// think about this", "breaking this down", ...). Matched anywhere, same as `RE_ANYWHERE` above --
/// this phrasing has no legitimate reason to survive into finished prose either. Shared with
/// `rules::preamble` (SLOP002, code comments) via `prose_words::REASONING_CHAIN_FRAGMENT` so the
/// phrase list can't drift between the two consumers. `step 1:` is the one member that isn't
/// shared -- it has a legitimate reading, so it needs position context (see `RE_NUMBERED_STEP`).
static RE_REASONING_CHAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)(?-u:\b)(?:{REASONING_CHAIN_FRAGMENT})")).unwrap());

/// Portuguese reasoning-chain leakage. "deixa eu pensar/verificar" and "vamos por partes" are
/// dropped: both are ordinary human asides in runbooks and teaching material ("Antes de sair,
/// deixa eu verificar os logs.", "Vamos por partes: primeiro o build."), not chain-of-thought
/// residue. ASCII-only panel, so `(?-u:\b)` is safe and keeps the lazy DFA (issue #21).
static RE_REASONING_CHAIN_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)vamos pensar (passo a passo|sobre isso)(?-u:\b)").unwrap()
});

/// `Step N:` -- NOT line-initial. Numbered procedural headings and bold lead-ins (`## Step 1:
/// Detect the framework`, `**Step 1: Mine the conversation.**`, `- Step 1: open the file`) are
/// standard technical writing, not chat residue; every hit of the unanchored form across a
/// 130-document corpus was one of those. The residue reading is the phrase surfacing *inside* a
/// sentence ("...so, step 1: we parse the args"), where no author would number a step.
///
/// A markdown structure marker is *required* (`+`, not `*`): the exemption is for `Step N:` that
/// heads a section or list item, so a bare line-initial `Step 1: parse the config` -- which heads
/// nothing -- stays residue. `(?m)` makes `^` a line anchor. In HTML the tag that made it a
/// heading or list item is blanked, so `ProseDoc::block_initial` is the equivalent test there.
static RE_NUMBERED_STEP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]{0,3}(?:#{1,6}[ \t]+|[-*+][ \t]+|\d+\.[ \t]+|\*\*)+step \d+:").unwrap()
});

/// The residue form: `Step N:` with real text before it on the same line.
///
/// `Passo N:`/`Etapa N:` has no Portuguese twin here: a corpus pass found 34 hits, all ordinary
/// structure -- `Passo 1:` lines inside `•`-bulleted lists (a marker this rule's structural
/// exemption above doesn't recognize) and backward references (`do passo 4:`, `no passo 6:`). Unlike `step 1:`, `passo`/`etapa` are common enough nouns in Portuguese runbooks that the
/// mid-sentence reading isn't reliably chat residue.
static RE_MIDLINE_STEP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?-u:\b)step \d+:").unwrap());

/// Acknowledgment loops ("you're asking about X", "to answer your question, ..."). PARAGRAPH-
/// INITIAL only, checked separately below via `fragmentation::paragraph_blocks`: a wrapped
/// continuation line that happens to start with this phrasing mid-paragraph is ordinary English,
/// but the very first line of a paragraph is where a chat-turn acknowledgment actually lands.
static RE_ACK_LOOP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:you(?:'|\u{2019})re asking (?:about|for)|to (?:answer|address) your question)(?-u:\b)",
    )
    .unwrap()
});

/// Portuguese paragraph-initial acknowledgment loop, same anchor discipline as `RE_ACK_LOOP`.
/// Only the two "your question" forms: "conforme solicitado" and "como você pediu" open ordinary
/// human emails and PR descriptions, too common for a Tier A panel.
static RE_ACK_LOOP_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:para responder|respondendo) [àa] sua pergunta(?-u:\b)").unwrap()
});

/// Group C (conversational openers). LINE-INITIAL anchor only: a paragraph that legitimately
/// opens with "Sure, ..." mid-document is ordinary English; residue only when it's literally
/// the first thing on the line (chat-turn register bleeding through).
static RE_OPENER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]*(certainly|sure|absolutely|of course|great question|excellent question)[!,.]|^[ \t]*(you'?re absolutely right|that'?s an excellent (?:point|question)|happy to help)(?-u:\b)").unwrap()
});

/// Portuguese line-initial openers, Group C's shape. `Ótima pergunta!` lives here rather than in
/// SLOP022 because it is a chat reflex, not a rhetorical setup. Unlike `RE_OPENER`, the
/// interjections take `!` only: a corpus check found line-initial "Claro, ..." as an ordinary
/// connector in translated docs, and "Com certeza, ..." / "Certamente, ..." open human paragraphs
/// the same way.
static RE_OPENER_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]*(claro|com certeza|certamente|sem problemas)!|^[ \t]*([óo]tima|excelente) pergunta[!,]").unwrap()
});

/// Group D (closers). END-OF-LINE/end-of-paragraph anchor: "feel free to reach out to a
/// maintainer" mid-sentence is normal CONTRIBUTING-doc prose; only the chat-closer form --
/// the phrase trailing off the end of the line -- is residue.
static RE_CLOSER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)(?-u:\b)(i hope this helps|hope this helps|let me know if you (need|have|'?d like)|feel free to (reach out|ask)|don'?t hesitate to ask|would you like me to|is there anything else)(?-u:\b)(?:[!.]|\s*$)").unwrap()
});

/// Portuguese closers, same end-of-line/end-of-paragraph anchor as `RE_CLOSER`. `fico à
/// disposição` and `qualquer dúvida, é só chamar/avisar/perguntar` are dropped: both are ordinary
/// Brazilian email/README sign-offs, not chat residue (a corpus check found real developer docs
/// closing with them). ASCII-only panel, so `(?-u:\b)` is safe (issue #21).
static RE_CLOSER_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)(?-u:\b)(espero (?:que (?:isso |isto )?)?(?:ajude|tenha ajudado)|me avise se precisar de (mais )?(alguma coisa|algo|ajuda)|se precisar de mais alguma coisa)(?-u:\b)(?:[!.]|\s*$)").unwrap()
});

/// Paragraph-initial acknowledgment-loop bytes, parameterized over the language-specific regex.
/// HTML has no blank-line paragraphs; there "paragraph-initial" means opening a block element.
fn ack_loop_bytes(doc: &ProseDoc, re: &Regex) -> Vec<usize> {
    if doc.block_starts.is_empty() {
        fragmentation::paragraph_blocks(doc)
            .iter()
            .filter(|b| re.is_match(&b.text))
            .map(|b| b.first_byte)
            .collect()
    } else {
        doc.block_starts
            .iter()
            .filter_map(|&bs| {
                let rest = &doc.masked[bs..];
                let trimmed = rest.trim_start();
                re.is_match(trimmed)
                    .then(|| bs + (rest.len() - trimmed.len()))
            })
            .collect()
    }
}

/// Scope: headings in scope, frontmatter in scope, URLs/link text in scope -- only code (already
/// blanked in `doc.masked`) is excluded. One diagnostic per matching line: track the first
/// (minimum-byte) match per line across all groups (the anywhere/opener/closer regexes plus the
/// paragraph-initial acknowledgment-loop check), then emit once per line in line order. Each
/// language's panel only runs when `ctx.natlangs` enables it; the default enables both (union).
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let en = ctx.natlangs.contains(&NatLang::En);
    let pt = ctx.natlangs.contains(&NatLang::PtBr);

    let mut anywhere_bytes: Vec<usize> = Vec::new();
    if en {
        anywhere_bytes.extend(
            [
                &*RE_ANYWHERE,
                &*RE_REASONING_CHAIN,
                &*RE_OPENER,
                &*RE_CLOSER,
            ]
            .into_iter()
            .flat_map(|re| re.find_iter(&doc.masked).map(|m| m.start())),
        );
    }
    if pt {
        anywhere_bytes.extend(
            [
                &*RE_ANYWHERE_PT_BR,
                &*RE_REASONING_CHAIN_PT_BR,
                &*RE_OPENER_PT_BR,
                &*RE_CLOSER_PT_BR,
            ]
            .into_iter()
            .flat_map(|re| re.find_iter(&doc.masked).map(|m| m.start())),
        );
    }

    let mut step_bytes = Vec::new();
    if en {
        let structural_ends: std::collections::HashSet<usize> = RE_NUMBERED_STEP
            .find_iter(&doc.masked)
            .map(|m| m.end())
            .collect();
        step_bytes.extend(
            RE_MIDLINE_STEP
                .find_iter(&doc.masked)
                .filter(|m| !structural_ends.contains(&m.end()) && !doc.block_initial(m.start()))
                .map(|m| m.start()),
        );
    }

    let mut ack_bytes = Vec::new();
    if en {
        ack_bytes.extend(ack_loop_bytes(doc, &RE_ACK_LOOP));
    }
    if pt {
        ack_bytes.extend(ack_loop_bytes(doc, &RE_ACK_LOOP_PT_BR));
    }

    let by_line = first_byte_per_line(
        doc,
        anywhere_bytes
            .into_iter()
            .chain(ack_bytes)
            .chain(step_bytes),
    );
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "unedited assistant-response residue in prose",
            "delete the sentence",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_in(ProseDoc::parse(src), src, Lang::Md)
    }

    fn diagnostics_for_html(src: &str) -> Vec<Diagnostic> {
        diagnostics_in(ProseDoc::parse_html(src), src, Lang::Html)
    }

    fn diagnostics_in<'a>(doc: ProseDoc<'a>, src: &'a str, lang: Lang) -> Vec<Diagnostic> {
        diagnostics_in_natlangs(doc, src, lang, crate::lang::ALL_NATLANGS)
    }

    fn diagnostics_in_natlangs<'a>(
        doc: ProseDoc<'a>,
        src: &'a str,
        lang: Lang,
        natlangs: &'static [NatLang],
    ) -> Vec<Diagnostic> {
        let ctx = LintContext {
            display_path: "test.md".to_string(),
            source: src,
            index: None,
            lang,
            comments: &doc.ignore_comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(&doc),
            natlangs,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn html_step_heading_and_list_item_are_structure() {
        let src = "<h3>Step 1: Install</h3>\n<ul><li>Step 2: run it</li></ul>\n<p>so, step 3: we parse.</p>\n";
        let diags = diagnostics_for_html(src);
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].line, 3);
    }

    #[test]
    fn html_ack_loop_only_when_it_opens_a_block() {
        assert_eq!(
            diagnostics_for_html("<p>You're asking about caching.</p>\n").len(),
            1
        );
        assert!(diagnostics_for_html("<p>Text. You're asking about caching.</p>\n").is_empty());
    }

    /// Every `Step N:` hit across a 130-document corpus was one of these -- numbered procedural
    /// writing, not chat residue.
    #[test]
    fn numbered_step_as_structure_is_not_residue() {
        for src in [
            "## Step 1: Detect the framework\n",
            "### Step 1: Load Eval Cases\n",
            "**Step 1: Mine the conversation.** Read what the user already said.\n",
            "- Step 1: open the file\n",
            "1. Step 1: open the file\n",
            "- **Step 2: Verify.** Re-run the suite.\n",
            "# Step 1: Navigate to form\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "structure flagged as residue: {src:?}"
            );
        }
    }

    #[test]
    fn numbered_step_mid_sentence_is_still_residue() {
        let diags =
            diagnostics_for("The parser is straightforward, so step 1: we read the args.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP011");
    }

    #[test]
    fn flags_self_id_disclaimer() {
        let diags =
            diagnostics_for("As an AI language model, I don't have access to real-time data.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP011");
    }

    #[test]
    fn flags_refusal_boilerplate() {
        let diags = diagnostics_for("I'm sorry, but I cannot provide that here.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_reviewer_leakage() {
        let diags = diagnostics_for("Reviewer note: please check the config.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_line_initial_opener() {
        let diags = diagnostics_for("Certainly! Here is the updated table.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_mid_sentence_opener() {
        let diags = diagnostics_for(
            "Our release process is smooth, and sure enough, tests catch regressions.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_end_of_line_closer() {
        let diags = diagnostics_for("If anything's unclear, feel free to reach out!\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_mid_sentence_closer() {
        let diags = diagnostics_for(
            "Feel free to reach out to a maintainer before starting a large change.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn diagnostic_carries_a_fix_hint() {
        let diags = diagnostics_for("Reviewer note: please check the config.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].fix.as_deref(), Some("delete the sentence"));
    }

    #[test]
    fn flags_new_opener_markers() {
        let diags = diagnostics_for(
            "Excellent question! Let's look at the logs.\n\nThat's an excellent point about retries.\n\nHappy to help with the migration.\n",
        );
        assert_eq!(diags.len(), 3);
        assert!(diags.iter().all(|d| d.code == "SLOP011"));
    }

    #[test]
    fn flags_speculative_gap_filling_markers() {
        let cases = [
            "The author maintains a low profile outside of work.\n",
            "She keeps her personal life private from the press.\n",
            "He prefers to stay out of the spotlight entirely.\n",
            "The engineer likely grew up in the same region as the team.\n",
            "A verified birth date is not publicly available for this person.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn flags_reasoning_chain_leakage_markers() {
        let cases = [
            "Let's think about this differently before shipping the fix.\n",
            "Let\u{2019}s think about this differently before shipping the fix.\n",
            "Let me think about the right way to phrase this.\n",
            "Thinking through this carefully before writing the final answer.\n",
            "Step 1: parse the config file into a struct.\n",
            "Breaking this down into smaller pieces makes it easier to review.\n",
            "First, let's consider what happens when the queue backs up.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn flags_paragraph_initial_acknowledgment_loop() {
        let cases = [
            "You're asking about the retry budget, so here is how it works.\n",
            "You\u{2019}re asking for a walkthrough of the deploy pipeline.\n",
            "To answer your question, the timeout defaults to thirty seconds.\n",
            "To address your question directly, caching is enabled by default.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn ignores_mid_paragraph_acknowledgment_loop() {
        // Same phrasing, but not the first line of its paragraph -- ordinary English, not a
        // chat-turn acknowledgment bleeding through.
        let src = "The config file controls most of the runtime behavior.\nYou're asking about the retry budget specifically here.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_code_fence() {
        let diags = diagnostics_for(
            "Body text.\n```\nAs an AI language model, I have no opinions.\n```\nMore text here.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn dedupes_multiple_matches_per_line() {
        let diags = diagnostics_for(
            "As an AI language model, I don't have access to real-time information.\n",
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_pt_br_self_id_disclaimer() {
        let diags = diagnostics_for(
            "Como um modelo de linguagem, não tenho acesso a dados em tempo real.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP011");
    }

    #[test]
    fn flags_pt_br_refusal_boilerplate() {
        let diags = diagnostics_for("Desculpe, mas não posso ajudar com essa solicitação.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP011");
        assert!(diagnostics_for("Não posso ajudar com essa mudança hoje.\n").is_empty());
        assert!(diagnostics_for(
            "Desculpe, mas não posso ajudar hoje, estou de férias até segunda.\n"
        )
        .is_empty());
    }

    #[test]
    fn flags_pt_br_opener_com_certeza() {
        let diags = diagnostics_for("Com certeza! Aqui está o script atualizado.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP011");
    }

    #[test]
    fn flags_pt_br_closer_variants() {
        let cases = [
            "Espero que ajude.\n",
            "Espero que isso tenha ajudado!\n",
            "Qualquer coisa, se precisar de mais alguma coisa.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn flags_pt_br_self_id_variants() {
        let cases = [
            "Falo como uma inteligência artificial treinada para ajudar times.\n",
            "Ajo como um assistente virtual dedicado a tarefas de suporte.\n",
            "Isso não estava nos meus dados de treinamento originais.\n",
            "Não tenho essa informação até a minha última atualização.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn flags_pt_br_opener_variants() {
        let cases = [
            "Certamente! Vou revisar o código agora.\n",
            "Sem problemas! Ajusto o script já.\n",
            "Ótima pergunta! Vamos ver a resposta.\n",
            "Excelente pergunta, vou responder em detalhes.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn ignores_pt_br_ordinary_sign_offs_and_asides() {
        for src in [
            "Fico à disposição.\n",
            "Qualquer dúvida, é só avisar.\n",
            "Antes de sair, deixa eu verificar os logs.\n",
            "Vamos por partes: primeiro o build.\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "unexpectedly flagged: {src:?}"
            );
        }
    }

    #[test]
    fn pt_br_gate_silences_portuguese_panel_when_only_english_selected() {
        let src = "Vamos pensar passo a passo antes de aplicar a correção.\n";
        assert!(
            diagnostics_in_natlangs(ProseDoc::parse(src), src, Lang::Md, &[NatLang::En]).is_empty()
        );
        let src_en = "As an AI language model, I have no opinions.\n";
        assert!(diagnostics_in_natlangs(
            ProseDoc::parse(src_en),
            src_en,
            Lang::Md,
            &[NatLang::PtBr]
        )
        .is_empty());
    }

    #[test]
    fn flags_pt_br_reasoning_chain_leakage() {
        let cases = [
            "Vamos pensar passo a passo antes de propor a correção.\n",
            "Vamos pensar sobre isso antes de propor a correção.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
        }
    }

    #[test]
    fn flags_pt_br_line_initial_opener() {
        let diags = diagnostics_for("Claro! Vamos ajustar o script agora mesmo.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_pt_br_mid_sentence_claro() {
        // "Claro," as an ordinary connector (not a line-initial chat opener) stays silent --
        // the comma form fires on real Portuguese technical prose (see corpus note above).
        assert!(
            diagnostics_for("Claro que a equipe testou tudo antes do lançamento.\n").is_empty()
        );
    }

    #[test]
    fn flags_pt_br_end_of_line_closer() {
        let diags = diagnostics_for("Qualquer coisa, me avise se precisar de mais alguma coisa!\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_pt_br_mid_sentence_closer() {
        let diags = diagnostics_for(
            "Fico à disposição se precisar de mais alguma coisa antes da reunião de amanhã.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_pt_br_paragraph_initial_acknowledgment_loop() {
        let cases = [
            "Para responder à sua pergunta, o timeout padrão é de trinta segundos.\n",
            "Respondendo à sua pergunta, o relatório completo segue anexo.\n",
        ];
        assert!(
            diagnostics_for("Conforme solicitado, o relatório completo segue anexo.\n").is_empty()
        );
        assert!(diagnostics_for("Com certeza, o relatório completo segue anexo.\n").is_empty());
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
        }
    }

    #[test]
    fn ignores_pt_br_mid_paragraph_acknowledgment_loop() {
        let src = "O arquivo de configuração controla a maior parte do comportamento.\nConforme solicitado no início da reunião, o prazo foi mantido.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ignores_plain_sentence_with_passo() {
        assert!(diagnostics_for("Cada passo do processo foi validado com cuidado.\n").is_empty());
    }

    #[test]
    fn ignores_pt_br_midsentence_numbered_step() {
        // `passo 1:` used to be a residue shape, but real documents fire it on ordinary
        // structure (bulleted `Passo 1:` steps outside markdown lists) and backward references
        // (`do passo 4:`), so it was dropped from the panel entirely.
        assert!(
            diagnostics_for("O time revisou o código, então, passo 1: ajustamos o timeout.\n")
                .is_empty()
        );
    }
}
