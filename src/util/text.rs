//! Text utility functions.

/// Truncate a string to a maximum length, adding ellipsis if truncated.
pub fn truncate_with_ellipsis(text: &str, max_len: usize) -> String {
    if max_len == 0 {
        return text.to_string();
    }
    let char_count = text.chars().count();
    if char_count <= max_len {
        return text.to_string();
    }
    // Reserve 1 character for the ellipsis
    let truncate_at = max_len.saturating_sub(1);
    let truncated: String = text.chars().take(truncate_at).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate_with_ellipsis("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate_with_ellipsis("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate_with_ellipsis("hello world", 6), "hello…");
    }

    #[test]
    fn test_truncate_zero() {
        assert_eq!(truncate_with_ellipsis("hello", 0), "hello");
    }

    #[test]
    fn test_truncate_one() {
        assert_eq!(truncate_with_ellipsis("hello", 1), "…");
    }
}
