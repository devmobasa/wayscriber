pub const GNOME_MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
pub const GNOME_MEDIA_KEYS_KEY: &str = "custom-keybindings";
pub const GNOME_CUSTOM_KEYBINDING_SCHEMA: &str =
    "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
pub const GNOME_WAYSCRIBER_KEYBINDING_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/";

pub fn gnome_shortcut_schema_with_path() -> String {
    format!("{GNOME_CUSTOM_KEYBINDING_SCHEMA}:{GNOME_WAYSCRIBER_KEYBINDING_PATH}")
}

pub fn is_gnome_desktop(current: &str, session: &str) -> bool {
    let combined = format!("{current};{session}").to_lowercase();
    combined.contains("gnome")
}

pub fn normalize_shortcut_hint(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

pub fn normalize_binding_hint(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("disabled") {
        return None;
    }
    Some(trimmed.to_string())
}

pub fn parse_gsettings_string_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "''" {
        return None;
    }
    let unquoted = trimmed
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .unwrap_or(trimmed);
    if unquoted.is_empty() {
        return None;
    }
    Some(unquoted.replace("\\'", "'").replace("\\\\", "\\"))
}

pub fn parse_gsettings_path_list(raw: &str) -> Result<Vec<String>, String> {
    let trimmed = raw.trim();
    let list_literal = trimmed.strip_prefix("@as ").map_or(trimmed, str::trim);
    if !list_literal.starts_with('[') || !list_literal.ends_with(']') {
        return Err(format!(
            "Unexpected gsettings list format: `{}`",
            raw.trim()
        ));
    }
    let inner = list_literal[1..list_literal.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }

    let mut values = Vec::new();
    for chunk in inner.split(',') {
        let value = chunk.trim().trim_matches('\'').trim_matches('"').trim();
        if value.is_empty() {
            continue;
        }
        values.push(value.to_string());
    }
    Ok(values)
}

pub fn gnome_effective_shortcut(custom_keybindings_raw: &str, binding_raw: &str) -> Option<String> {
    let configured_paths = parse_gsettings_path_list(custom_keybindings_raw).ok()?;
    if !configured_paths
        .iter()
        .any(|path| path == GNOME_WAYSCRIBER_KEYBINDING_PATH)
    {
        return None;
    }
    let binding = parse_gsettings_string_value(binding_raw)?;
    normalize_binding_hint(Some(binding.as_str()))
}

pub fn resolve_toggle_shortcut_hint(
    portal_shortcut_env: Option<&str>,
    gnome_desktop: bool,
    gnome_custom_keybindings_raw: Option<&str>,
    gnome_binding_raw: Option<&str>,
) -> Option<String> {
    if let Some(portal_shortcut) = normalize_shortcut_hint(portal_shortcut_env) {
        return Some(portal_shortcut);
    }
    if !gnome_desktop {
        return None;
    }
    let custom_keybindings = gnome_custom_keybindings_raw?;
    let binding = gnome_binding_raw?;
    gnome_effective_shortcut(custom_keybindings, binding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_shortcut_hint_trims_and_rejects_empty() {
        assert_eq!(
            normalize_shortcut_hint(Some("  <Ctrl><Shift>g ")),
            Some("<Ctrl><Shift>g".to_string())
        );
        assert_eq!(normalize_shortcut_hint(Some("   ")), None);
        assert_eq!(normalize_shortcut_hint(None), None);
    }

    #[test]
    fn normalize_binding_hint_rejects_disabled_case_insensitive() {
        assert_eq!(normalize_binding_hint(Some("disabled")), None);
        assert_eq!(normalize_binding_hint(Some("DiSaBlEd")), None);
        assert_eq!(
            normalize_binding_hint(Some(" <Super>g ")),
            Some("<Super>g".to_string())
        );
    }

    #[test]
    fn parse_gsettings_path_list_handles_variants() {
        assert_eq!(
            parse_gsettings_path_list("@as []").unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            parse_gsettings_path_list("[]").unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            parse_gsettings_path_list("['/org/one/', '/org/two/']").unwrap(),
            vec!["/org/one/".to_string(), "/org/two/".to_string()]
        );
    }

    #[test]
    fn gnome_effective_shortcut_requires_registered_path() {
        assert_eq!(
            gnome_effective_shortcut(
                "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/other/']",
                "'<Super>g'",
            ),
            None
        );
    }

    #[test]
    fn gnome_effective_shortcut_rejects_disabled_or_empty_binding() {
        let paths = "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/']";
        assert_eq!(gnome_effective_shortcut(paths, "'disabled'"), None);
        assert_eq!(gnome_effective_shortcut(paths, "'  DISABLED  '"), None);
        assert_eq!(gnome_effective_shortcut(paths, "''"), None);
    }

    #[test]
    fn gnome_effective_shortcut_accepts_valid_binding() {
        let paths = "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/']";
        assert_eq!(
            gnome_effective_shortcut(paths, "'<Super>g'"),
            Some("<Super>g".to_string())
        );
    }

    #[test]
    fn resolve_toggle_shortcut_hint_prefers_portal_env() {
        assert_eq!(
            resolve_toggle_shortcut_hint(
                Some("  Super+G  "),
                true,
                Some(
                    "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/']"
                ),
                Some("'<Super>x'"),
            ),
            Some("Super+G".to_string())
        );
    }

    #[test]
    fn resolve_toggle_shortcut_hint_rejects_non_gnome_fallback() {
        assert_eq!(
            resolve_toggle_shortcut_hint(
                None,
                false,
                Some(
                    "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/']"
                ),
                Some("'<Super>g'"),
            ),
            None
        );
    }

    #[test]
    fn resolve_toggle_shortcut_hint_rejects_blank_portal_env() {
        assert_eq!(
            resolve_toggle_shortcut_hint(
                Some("   "),
                false,
                Some(
                    "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/']"
                ),
                Some("'<Super>g'"),
            ),
            None
        );
    }
}
