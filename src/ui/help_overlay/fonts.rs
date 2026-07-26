use pango::prelude::*;
use std::collections::HashSet;

pub(crate) fn resolve_help_font_family(family_list: &str) -> String {
    let available = help_font_families();
    resolve_help_font_family_from(family_list, &available)
}

fn resolve_help_font_family_from(family_list: &str, available: &HashSet<String>) -> String {
    let mut fallback = None;
    for raw in family_list.split(',') {
        let candidate = raw.trim();
        if candidate.is_empty() {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(candidate);
        }
        let key = candidate.to_ascii_lowercase();
        if available.contains(&key) {
            return candidate.to_string();
        }
    }
    fallback.unwrap_or("Sans").to_string()
}

fn help_font_families() -> HashSet<String> {
    let font_map = pangocairo::FontMap::default();
    font_map
        .list_families()
        .into_iter()
        .map(|family| family.name().to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn resolver_selects_the_first_available_family_case_insensitively() {
        let fonts = available(&["noto sans", "sans"]);

        assert_eq!(
            resolve_help_font_family_from("Missing, Noto Sans, Sans", &fonts),
            "Noto Sans"
        );
    }

    #[test]
    fn resolver_preserves_the_first_named_fallback() {
        let fonts = available(&[]);

        assert_eq!(
            resolve_help_font_family_from("  Preferred , Backup ", &fonts),
            "Preferred"
        );
    }

    #[test]
    fn resolver_uses_sans_when_the_family_list_is_empty() {
        let fonts = available(&[]);

        assert_eq!(resolve_help_font_family_from(" , ", &fonts), "Sans");
    }
}
