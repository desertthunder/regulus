/// Remove one surrounding pair of double quotes from source text.
///
/// This is intended for parser-provided literal source strings.
///
/// It leaves unmatched quotes and inner quotes untouched.
pub fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_one_surrounding_quote_pair() {
        assert_eq!(unquote("\"gleam/io\""), "gleam/io");
    }

    #[test]
    fn leaves_unmatched_or_extra_boundary_quotes() {
        assert_eq!(unquote("\"gleam/io"), "\"gleam/io");
        assert_eq!(unquote("gleam/io\""), "gleam/io\"");
        assert_eq!(unquote("\"\"quoted\"\""), "\"quoted\"");
    }
}
