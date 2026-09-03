//! Identifier comparison shared by parsing, metadata lookup, and SQL generation.

/// Compares logical identifiers using the compiler's case-insensitive rule.
pub(super) fn names_equal(left: &str, right: &str) -> bool {
    if left.is_ascii() && right.is_ascii() {
        return left.eq_ignore_ascii_case(right);
    }
    left.chars()
        .flat_map(char::to_lowercase)
        .eq(right.chars().flat_map(char::to_lowercase))
}

/// Produces the hash-map key corresponding to [`names_equal`].
pub(super) fn folded_name(value: &str) -> String {
    if value.is_ascii() {
        value.to_ascii_lowercase()
    } else {
        value.chars().flat_map(char::to_lowercase).collect()
    }
}
