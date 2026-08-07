//! Renderer-neutral label formatting for session names and paths.
//!
//! Both adapters share session-extension stripping. The built-in Session
//! popover also uses the fixed character-count truncators below, while GTK
//! delegates visible ellipsization to Pango.

/// Middle-ellipsize so both the head and the distinguishing tail survive.
/// Tail truncation made e.g. two different "lecture-05-…" files render
/// identically in the recents list.
pub fn truncate_middle(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(1).max(2);
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let mut truncated: String = value.chars().take(head).collect();
    truncated.push('…');
    truncated.extend(value.chars().skip(count - tail));
    truncated
}

/// Keep the tail of a path — the leading directories are the least
/// informative part of a session path.
pub fn truncate_start(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let mut truncated = String::from("…");
    truncated.extend(value.chars().skip(count - keep));
    truncated
}

/// Drop the constant session-file extension in list rows; it costs the
/// characters that distinguish one session from another.
pub fn strip_session_extension(value: &str) -> &str {
    value
        .strip_suffix(".wayscriber-session")
        .or_else(|| value.strip_suffix(".wayscriber"))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_middle_keeps_head_and_tail() {
        assert_eq!(truncate_middle("short", 10), "short");
        let truncated = truncate_middle("lecture-05-quantum-mechanics-part-two", 20);
        assert_eq!(truncated.chars().count(), 20);
        assert!(truncated.starts_with("lecture-05"));
        assert!(truncated.ends_with("part-two"));
        assert!(truncated.contains('…'));
    }

    #[test]
    fn truncate_start_keeps_path_tail() {
        assert_eq!(truncate_start("/tmp/x", 10), "/tmp/x");
        let truncated = truncate_start("/home/user/.local/share/wayscriber/sessions", 20);
        assert_eq!(truncated.chars().count(), 20);
        assert!(truncated.starts_with('…'));
        assert!(truncated.ends_with("wayscriber/sessions"));
    }

    #[test]
    fn strip_session_extension_drops_known_suffixes() {
        assert_eq!(
            strip_session_extension("lecture.wayscriber-session"),
            "lecture"
        );
        assert_eq!(strip_session_extension("lecture.wayscriber"), "lecture");
        assert_eq!(strip_session_extension("lecture.txt"), "lecture.txt");
    }
}
