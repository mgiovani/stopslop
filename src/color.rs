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
        r"(?i-u)#[0-9a-f]{{8}}(?-u:\b)|#[0-9a-f]{{6}}(?-u:\b)|#[0-9a-f]{{3,4}}(?-u:\b)|(?-u:\b)(?:rgb|hsl)a?\((?u:[^()]*)\)|(?-u:\b)(?:{names}|transparent)(?-u:\b)"
    ))
    .unwrap()
});

/// Parses one CSS color token: hex (3/4/6/8 digits), `rgb()`/`rgba()`, `hsl()`/`hsla()` in the
/// comma or space syntax, or a short named-color table. `None` for anything else (`var()`,
/// `currentColor`, `oklch()`).
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
    NAMED
        .iter()
        .find(|(n, _)| *n == t)
        .map(|&(_, [r, g, b])| Rgba { r, g, b, a: 1.0 })
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
    match p.strip_suffix('%') {
        Some(v) => v.parse::<f32>().ok().map(|v| v / 100.0 * scale),
        None => p.parse::<f32>().ok(),
    }
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
    let h = p[0].trim_end_matches("deg").parse::<f32>().ok()?;
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
}
