/// Score a query against text using prefix, word-boundary, substring, then
/// ordered-subsequence matching.
pub(crate) fn fuzzy_score(query: &str, text: &str) -> i32 {
    let text_lower = text.to_lowercase();

    if text_lower.starts_with(query) {
        return 100;
    }

    if text_lower
        .split_whitespace()
        .any(|word| word.starts_with(query))
    {
        return 75;
    }

    if text_lower.contains(query) {
        return 25;
    }

    let mut text_chars = text_lower.chars().peekable();
    let mut matched = 0;
    let query_len = query.chars().count();
    for query_char in query.chars() {
        while let Some(&text_char) = text_chars.peek() {
            text_chars.next();
            if text_char == query_char {
                matched += 1;
                break;
            }
        }
    }
    if matched == query_len { 10 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn prefix_matches_outrank_subsequence_matches() {
        assert!(fuzzy_score("cap", "capture to file") > fuzzy_score("cap", "clipboard action"));
    }

    #[test]
    fn word_boundary_matches_outrank_plain_substrings() {
        assert!(fuzzy_score("bar", "status bar") > fuzzy_score("bar", "crowbar"));
    }
}
