pub mod gradient_text;
pub mod purple_gradient;
pub mod violet_hex;

use crate::style::Decl;

pub(crate) fn is_gradient(value: &str) -> bool {
    value.contains("gradient(")
}

/// `-webkit-background-clip` and `background-clip` name the same property.
pub(crate) fn unprefixed(property: &str) -> &str {
    property.strip_prefix("-webkit-").unwrap_or(property)
}

pub(crate) fn is_gradient_background(d: &Decl) -> bool {
    matches!(unprefixed(&d.property), "background" | "background-image") && is_gradient(d.value)
}
