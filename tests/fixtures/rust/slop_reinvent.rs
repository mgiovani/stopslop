use regex::Regex;

fn make_validator() -> Regex {
    Regex::new(r"[^\s@]+@[^\s@]+").unwrap() // expect: SLOP037
}
