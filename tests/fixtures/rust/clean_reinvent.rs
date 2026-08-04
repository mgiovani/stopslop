use regex::Regex;

fn make_validator() -> Regex {
    Regex::new(r"^\d{3}-\d{2}-\d{4}$").unwrap()
}
