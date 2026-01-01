use pango::prelude::*;
use std::collections::HashSet;
use std::sync::OnceLock;

pub(crate) fn resolve_help_font_family(family_list: &str) -> String {
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
        if help_font_families().contains(&key) {
            return candidate.to_string();
        }
    }
    fallback.unwrap_or("Sans").to_string()
}

pub(crate) fn help_font_families() -> &'static HashSet<String> {
    static CACHE: OnceLock<HashSet<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let font_map = pangocairo::FontMap::default();
        font_map
            .list_families()
            .into_iter()
            .map(|family| family.name().to_ascii_lowercase())
            .collect()
    })
}
