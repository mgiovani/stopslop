fn normalize_account(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-account");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_profile(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-profile");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_session(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-session");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_invoice(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-invoice");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_payment(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-payment");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_address(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-address");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_contact(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-contact");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_channel(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-channel");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_message(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-message");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_summary(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-summary");
    }
    value.replace(char::is_whitespace, "-")
}

fn normalize_receipt(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "-" {
        return String::from("unnamed-receipt");
    }
    value.replace(char::is_whitespace, "-")
}
// expect-line: 1 SLOP045
