const HEX: &[u8; 16] = b"0123456789abcdef";

/// Escape a name component into a backend-safe, collision-free segment.
///
/// The renderer hex-encodes UTF-8 bytes instead of sanitizing punctuation. That
/// keeps `foo-bar`, `foo_bar`, and `foo/bar` distinct on every platform.
pub fn escape_segment(value: &str) -> String {
    let mut escaped = String::with_capacity(1 + value.len() * 2);
    escaped.push('x');
    for byte in value.bytes() {
        escaped.push(HEX[(byte >> 4) as usize] as char);
        escaped.push(HEX[(byte & 0x0f) as usize] as char);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_utf8_bytes() {
        assert_eq!(escape_segment("app/main"), "x6170702f6d61696e");
        assert_eq!(escape_segment("Δ"), "xce94");
    }

    #[test]
    fn avoids_sanitization_collisions() {
        assert_ne!(escape_segment("foo-bar"), escape_segment("foo_bar"));
        assert_ne!(escape_segment("foo/bar"), escape_segment("foo_bar"));
    }
}
