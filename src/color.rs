use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

const NAMED: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("white", [255, 255, 255]),
    ("red", [255, 0, 0]),
    ("green", [0, 128, 0]),
    ("blue", [0, 0, 255]),
    ("purple", [128, 0, 128]),
    ("violet", [238, 130, 238]),
    ("indigo", [75, 0, 130]),
    ("magenta", [255, 0, 255]),
    ("fuchsia", [255, 0, 255]),
    ("cyan", [0, 255, 255]),
    ("aqua", [0, 255, 255]),
    ("pink", [255, 192, 203]),
    ("orange", [255, 165, 0]),
    ("yellow", [255, 255, 0]),
    ("gray", [128, 128, 128]),
    ("grey", [128, 128, 128]),
];

static COLOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    let names = NAMED.iter().map(|(n, _)| *n).collect::<Vec<_>>().join("|");
    Regex::new(&format!(
        r"(?i-u)#[0-9a-f]{{8}}(?-u:\b)|#[0-9a-f]{{6}}(?-u:\b)|#[0-9a-f]{{3,4}}(?-u:\b)|(?-u:\b)(?:rgba?|hsla?|oklch|oklab)\((?u:[^()]*)\)|(?-u:\b)(?:{names}|transparent)(?-u:\b)"
    ))
    .unwrap()
});

/// Parses one CSS color token: hex (3/4/6/8 digits), `rgb()`/`rgba()`, `hsl()`/`hsla()` in the
/// comma or space syntax, `oklch()`/`oklab()` (gamut-clipped to sRGB), or a short named-color
/// table. `None` for anything else (`var()`, `currentColor`).
pub fn parse(token: &str) -> Option<Rgba> {
    let t = token.trim().to_ascii_lowercase();
    if let Some(hex) = t.strip_prefix('#') {
        return parse_hex(hex);
    }
    if t == "transparent" {
        return Some(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0.0,
        });
    }
    if let Some(args) = function_args(&t, &["rgba", "rgb"]) {
        return parse_rgb(args);
    }
    if let Some(args) = function_args(&t, &["hsla", "hsl"]) {
        return parse_hsl(args);
    }
    if let Some(args) = function_args(&t, &["oklch"]) {
        return parse_oklch(args);
    }
    if let Some(args) = function_args(&t, &["oklab"]) {
        return parse_oklab(args);
    }
    NAMED
        .iter()
        .find(|(n, _)| *n == t)
        .map(|&(_, [r, g, b])| Rgba { r, g, b, a: 1.0 })
}

/// A shadcn theme value: the bare `H S% L%` triplet its v3-era tokens use (`262.1 83.3% 57.8%`),
/// which is not valid CSS by itself and so is not accepted by `parse`.
pub fn parse_hsl_triplet(value: &str) -> Option<Rgba> {
    let color = value.split('/').next().unwrap_or(value);
    let mut parts = color.split_whitespace();
    let (_, s, l) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || !s.ends_with('%') || !l.ends_with('%') {
        return None;
    }
    parse_hsl(value)
}

/// Violet and purple, the hues a stock "AI" theme picks: HSL hue 245-285 with saturation of at
/// least 0.35. The floor sits above pure blue and Tailwind indigo-500 (hue 239-241 depending on
/// notation) and the ceiling below fuchsia (292) and magenta (300), so one palette stop reads the
/// same in hex and `oklch()`.
pub fn is_violet(c: Rgba) -> bool {
    let (h, s, _) = hsl(c);
    s >= 0.35 && (245.0..=285.0).contains(&h)
}

/// Every color token in `value` with its byte span, in source order. Tokens inside `var()` and
/// `url()` are skipped, and a named color glued to identifier characters (`--indigo`,
/// `purple.png`) is not a color.
pub fn colors_in(value: &str) -> Vec<(usize, usize, Rgba)> {
    let opaque = opaque_spans(value);
    COLOR_RE
        .find_iter(value)
        .filter(|m| !opaque.iter().any(|&(s, e)| m.start() >= s && m.start() < e))
        .filter(|m| m.as_str().starts_with('#') || is_standalone(value, m.start(), m.end()))
        .filter_map(|m| parse(m.as_str()).map(|c| (m.start(), m.end(), c)))
        .collect()
}

fn is_standalone(value: &str, start: usize, end: usize) -> bool {
    let glue = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/');
    !value[..start].chars().next_back().is_some_and(glue)
        && !value[end..].chars().next().is_some_and(glue)
}

/// Spans of every `var(...)` and `url(...)` call, parentheses balanced.
fn opaque_spans(value: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for opener in ["var(", "url("] {
        let mut from = 0;
        while let Some(at) = value[from..].find(opener) {
            let start = from + at;
            let mut depth = 0usize;
            let mut end = value.len();
            for (i, c) in value[start..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            spans.push((start, end));
            from = end;
        }
    }
    spans
}

/// (hue in degrees 0..360, saturation 0..1, lightness 0..1).
pub fn hsl(c: Rgba) -> (f32, f32, f32) {
    let (r, g, b) = (
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok().map(|d| d * 17);
    let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let (r, g, b, a) = match hex.len() {
        3 => (digit(0)?, digit(1)?, digit(2)?, 255),
        4 => (digit(0)?, digit(1)?, digit(2)?, digit(3)?),
        6 => (pair(0)?, pair(2)?, pair(4)?, 255),
        8 => (pair(0)?, pair(2)?, pair(4)?, pair(6)?),
        _ => return None,
    };
    Some(Rgba {
        r,
        g,
        b,
        a: f32::from(a) / 255.0,
    })
}

fn function_args<'t>(t: &'t str, names: &[&str]) -> Option<&'t str> {
    names.iter().find_map(|n| {
        t.strip_prefix(n)
            .and_then(|rest| rest.strip_prefix('('))
            .and_then(|rest| rest.strip_suffix(')'))
    })
}

fn split_args(args: &str) -> Vec<&str> {
    let (color, alpha) = args.split_once('/').unwrap_or((args, ""));
    let mut parts: Vec<&str> = color
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    parts.extend(alpha.split_whitespace());
    parts
}

fn number(p: &str, scale: f32) -> Option<f32> {
    let n = match p.strip_suffix('%') {
        Some(v) => v.parse::<f32>().ok().map(|v| v / 100.0 * scale),
        None if p == "none" => Some(0.0),
        None => p.parse::<f32>().ok(),
    };
    n.filter(|n| n.is_finite())
}

/// A CSS `<hue>` in degrees: unitless, `deg`, `grad`, `rad`, `turn`, or `none`.
fn hue(p: &str) -> Option<f32> {
    let (value, per_unit) = if let Some(v) = p.strip_suffix("deg") {
        (v, 1.0)
    } else if let Some(v) = p.strip_suffix("grad") {
        (v, 0.9)
    } else if let Some(v) = p.strip_suffix("rad") {
        (v, 180.0 / std::f32::consts::PI)
    } else if let Some(v) = p.strip_suffix("turn") {
        (v, 360.0)
    } else {
        (p, 1.0)
    };
    let degrees = if value == "none" {
        0.0
    } else {
        value.parse::<f32>().ok()? * per_unit
    };
    degrees.is_finite().then_some(degrees)
}

fn alpha(parts: &[&str]) -> Option<f32> {
    parts
        .get(3)
        .map_or(Some(1.0), |p| number(p, 1.0))
        .map(|a| a.clamp(0.0, 1.0))
}

fn parse_rgb(args: &str) -> Option<Rgba> {
    let p = split_args(args);
    if p.len() < 3 {
        return None;
    }
    let ch = |i: usize| number(p[i], 255.0).map(|v| v.round().clamp(0.0, 255.0) as u8);
    Some(Rgba {
        r: ch(0)?,
        g: ch(1)?,
        b: ch(2)?,
        a: alpha(&p)?,
    })
}

fn parse_hsl(args: &str) -> Option<Rgba> {
    let p = split_args(args);
    if p.len() < 3 {
        return None;
    }
    let h = hue(p[0])?;
    let s = number(p[1], 1.0)?.clamp(0.0, 1.0);
    let l = number(p[2], 1.0)?.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Some(Rgba {
        r: byte(r),
        g: byte(g),
        b: byte(b),
        a: alpha(&p)?,
    })
}

fn parse_oklch(args: &str) -> Option<Rgba> {
    let p = split_args(args);
    if p.len() < 3 {
        return None;
    }
    let l = number(p[0], 1.0)?;
    let chroma = number(p[1], 0.4)?;
    let h = hue(p[2])?.to_radians();
    Some(oklab_to_rgba(
        l,
        chroma * h.cos(),
        chroma * h.sin(),
        alpha(&p)?,
    ))
}

fn parse_oklab(args: &str) -> Option<Rgba> {
    let p = split_args(args);
    if p.len() < 3 {
        return None;
    }
    Some(oklab_to_rgba(
        number(p[0], 1.0)?,
        number(p[1], 0.4)?,
        number(p[2], 0.4)?,
        alpha(&p)?,
    ))
}

fn oklab_to_rgba(l: f32, a: f32, b: f32, alpha: f32) -> Rgba {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let linear = [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_93 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_4 * s_,
        -0.004_196_086 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ];
    let byte = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let encoded = if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (encoded * 255.0).round() as u8
    };
    Rgba {
        r: byte(linear[0]),
        g: byte(linear[1]),
        b: byte(linear[2]),
        a: alpha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 1.0 }
    }

    #[test]
    fn parses_hex_in_every_length() {
        assert_eq!(parse("#7c3aed"), Some(rgb(0x7c, 0x3a, 0xed)));
        assert_eq!(parse("#FFF"), Some(rgb(255, 255, 255)));
        assert_eq!(parse("#00000000").map(|c| c.a), Some(0.0));
        assert_eq!(parse("#12345"), None);
        assert_eq!(parse("#ggg"), None);
    }

    #[test]
    fn parses_functional_notation_in_both_syntaxes() {
        assert_eq!(parse("rgb(124, 58, 237)"), Some(rgb(124, 58, 237)));
        assert_eq!(parse("rgb(124 58 237 / 50%)").map(|c| c.a), Some(0.5));
        assert_eq!(parse("rgba(0,0,0,.25)").map(|c| c.a), Some(0.25));
        assert_eq!(parse("hsl(0, 100%, 50%)"), Some(rgb(255, 0, 0)));
        assert_eq!(parse("hsl(240deg 100% 50%)"), Some(rgb(0, 0, 255)));
        assert_eq!(parse("rgb(1, 2)"), None);
        assert_eq!(parse("var(--x)"), None);
    }

    #[test]
    fn parses_named_colors_and_transparent() {
        assert_eq!(parse("White"), Some(rgb(255, 255, 255)));
        assert_eq!(parse("transparent").map(|c| c.a), Some(0.0));
        assert_eq!(parse("rebeccapurple"), None);
    }

    #[test]
    fn colors_in_finds_tokens_with_spans_and_skips_non_colors() {
        let v = "linear-gradient(135deg, #667eea 0%, rgba(118, 75, 162, .9) 100%, var(--x))";
        let found = colors_in(v);
        assert_eq!(found.len(), 2);
        assert_eq!(&v[found[0].0..found[0].1], "#667eea");
        assert_eq!(&v[found[1].0..found[1].1], "rgba(118, 75, 162, .9)");
        assert!(colors_in("url(#abcdefg) 12px").is_empty());
    }

    #[test]
    fn hsl_places_violet_in_the_purple_band() {
        let (h, s, _) = hsl(rgb(0x7c, 0x3a, 0xed));
        assert!((255.0..290.0).contains(&h), "{h}");
        assert!(s > 0.8);
        assert_eq!(hsl(rgb(128, 128, 128)).1, 0.0);
    }

    #[test]
    fn identifiers_and_calls_that_hold_color_words_are_not_colors() {
        assert!(colors_in("var(--indigo)").is_empty());
        assert!(colors_in("url(purple.png) no-repeat").is_empty());
        assert!(colors_in("url(#abcabc)").is_empty());
        assert!(colors_in("var(--a, #7c3aed)").is_empty());
        assert!(colors_in("my-purple-box").is_empty());
        assert_eq!(colors_in("red url(blue.png) white").len(), 2);
    }

    #[test]
    fn multiline_and_tab_separated_arguments_parse() {
        assert_eq!(parse("rgb(1,\n\t2,\t3)"), Some(rgb(1, 2, 3)));
    }

    #[test]
    fn oklch_reads_tailwind_v4_palette_values() {
        let violet = parse("oklch(0.606 0.25 292.717)").unwrap();
        let (h, sat, _) = hsl(violet);
        assert!((240.0..290.0).contains(&h) && sat > 0.5, "{h} {sat}");
        let blue = parse("oklch(0.623 0.214 259.815)").unwrap();
        let (h, sat, _) = hsl(blue);
        assert!((205.0..235.0).contains(&h) && sat > 0.5, "{h} {sat}");
        assert_eq!(parse("oklch(1 0 0)"), Some(rgb(255, 255, 255)));
        assert_eq!(parse("oklch(0 0 0 / 50%)").map(|c| c.a), Some(0.5));
        assert_eq!(parse("oklab(0.5 0.1)"), None);
    }

    #[test]
    fn oklab_matches_oklch_and_colors_in_finds_both() {
        let lch = parse("oklch(0.606 0.25 292.717)").unwrap();
        let (a, b) = (
            0.25 * 292.717_f32.to_radians().cos(),
            0.25 * 292.717_f32.to_radians().sin(),
        );
        assert_eq!(parse(&format!("oklab(0.606 {a} {b})")), Some(lch));
        assert_eq!(
            colors_in("linear-gradient(oklch(0.6 0.2 290), oklab(0.6 0.1 -0.1))").len(),
            2
        );
    }

    #[test]
    fn hsl_triplet_is_the_shadcn_token_format() {
        let (h, s, _) = hsl(parse_hsl_triplet("262.1 83.3% 57.8%").unwrap());
        assert!((255.0..270.0).contains(&h) && s > 0.7, "{h} {s}");
        assert_eq!(parse_hsl_triplet("240 5.9% 10%").map(|c| c.a), Some(1.0));
        assert_eq!(parse_hsl_triplet("#7c3aed"), None);
        assert_eq!(parse_hsl_triplet("262 83 57"), None);
        assert_eq!(parse("262.1 83.3% 57.8%"), None);
    }

    #[test]
    fn is_violet_separates_tailwind_violet_from_blue_indigo_and_fuchsia() {
        for hex in [
            "#8b5cf6", "#7c3aed", "#6d28d9", "#a855f7", "#9333ea", "#7e22ce",
        ] {
            assert!(is_violet(parse(hex).unwrap()), "{hex}");
        }
        for hex in [
            "#6366f1", "#4f46e5", "#3b82f6", "#1d4ed8", "#0000ff", "#d946ef", "#c026d3", "#ff00ff",
            "#808080",
        ] {
            assert!(!is_violet(parse(hex).unwrap()), "{hex}");
        }
        for v4 in [
            "oklch(0.606 0.25 292.717)",
            "oklch(0.541 0.281 293.009)",
            "oklch(0.627 0.265 303.9)",
        ] {
            assert!(is_violet(parse(v4).unwrap()), "{v4}");
        }
        for v4 in [
            "oklch(0.585 0.233 277.117)",
            "oklch(0.623 0.214 259.815)",
            "oklch(0.546 0.245 262.881)",
            "oklch(0.667 0.295 322.15)",
        ] {
            assert!(!is_violet(parse(v4).unwrap()), "{v4}");
        }
    }

    #[test]
    fn malformed_and_exotic_angles_and_keywords() {
        assert_eq!(parse("oklch(0.5 0.1 abc)"), None);
        assert_eq!(parse("oklch(0.5 0.1 inf)"), None);
        assert_eq!(parse("oklch(nan 0.1 10)"), None);
        assert_eq!(parse("oklch(-0.5 -0.1 10deg)").map(|c| c.a), Some(1.0));
        assert_eq!(
            parse("oklch(0.606 0.25 0.8turn)"),
            parse("oklch(0.606 0.25 288)")
        );
        assert_eq!(
            parse("oklch(0.606 0.25 288deg)"),
            parse("oklch(0.606 0.25 288)")
        );
        assert_eq!(
            parse("oklch(0.606 0.25 320grad)"),
            parse("oklch(0.606 0.25 288)")
        );
        assert!(parse("oklch(0.606 0.25 5.0265rad)").is_some());
        assert_eq!(parse("oklch(0.5 none none)"), parse("oklch(0.5 0 0)"));
        assert_eq!(parse("hsl(0.5turn 100% 50%)"), parse("hsl(180, 100%, 50%)"));
        assert_eq!(parse("oklab(from var(--x) l a b)"), None);
        assert_eq!(parse("oklch(0.5 0.1)"), None);
    }

    #[test]
    fn hsl_triplet_accepts_an_alpha_tail_and_rejects_commas() {
        assert!(parse_hsl_triplet("262.1 83.3% 57.8% / 0.5").is_some_and(|c| c.a == 0.5));
        assert_eq!(parse_hsl_triplet("262, 83%, 58%"), None);
        assert_eq!(parse_hsl_triplet("262 83% 58% 1"), None);
    }
}
