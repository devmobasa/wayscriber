#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchQuery {
    raw: String,
    tokens: Vec<String>,
}

impl SearchQuery {
    pub(crate) fn new(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let tokens = raw
            .split_whitespace()
            .map(normalize)
            .filter(|token| !token.is_empty())
            .collect();
        Self { raw, tokens }
    }

    pub(crate) fn raw(&self) -> &str {
        &self.raw
    }

    pub(crate) fn has_raw_input(&self) -> bool {
        !self.raw.is_empty()
    }

    pub(crate) fn is_active(&self) -> bool {
        !self.tokens.is_empty()
    }

    pub(crate) fn matches_text(&self, value: &str) -> bool {
        self.matches_parts([value])
    }

    pub(crate) fn matches_parts<'a>(&self, parts: impl IntoIterator<Item = &'a str>) -> bool {
        if !self.is_active() {
            return true;
        }
        let haystack = parts
            .into_iter()
            .map(normalize)
            .collect::<Vec<_>>()
            .join(" ");
        self.tokens.iter().all(|token| haystack.contains(token))
    }
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self::new("")
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '#' {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::SearchQuery;

    #[test]
    fn inactive_query_matches_everything() {
        assert!(SearchQuery::new("  ").matches_text("Boards"));
    }

    #[test]
    fn raw_input_state_is_separate_from_active_tokens() {
        let punctuation = SearchQuery::new("/");
        assert!(punctuation.has_raw_input());
        assert!(!punctuation.is_active());

        let empty = SearchQuery::new("");
        assert!(!empty.has_raw_input());
        assert!(!empty.is_active());
    }

    #[test]
    fn matches_all_tokens_across_parts() {
        let query = SearchQuery::new("pdf label");
        assert!(query.matches_parts(["Export PDF", "Show page labels"]));
        assert!(!query.matches_parts(["Export PDF", "Filename template"]));
    }

    #[test]
    fn punctuation_does_not_block_matching() {
        let query = SearchQuery::new("render profiles");
        assert!(query.matches_text("Render Profiles"));
        assert!(SearchQuery::new("ctrl f").matches_text("Ctrl+F"));
    }
}
