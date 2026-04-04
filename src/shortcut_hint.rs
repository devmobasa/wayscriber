use std::fs;

use crate::systemd_user_service::portal_shortcut_dropin_path;

pub const GNOME_MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
pub const GNOME_MEDIA_KEYS_KEY: &str = "custom-keybindings";
pub const GNOME_CUSTOM_KEYBINDING_SCHEMA: &str =
    "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
pub const GNOME_WAYSCRIBER_KEYBINDING_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/";
pub const PORTAL_SHORTCUT_ENV: &str = "WAYSCRIBER_PORTAL_SHORTCUT";
pub const PORTAL_APP_ID_ENV: &str = "WAYSCRIBER_PORTAL_APP_ID";
pub const PORTAL_SHORTCUT_OPT_IN_ENV: &str = "WAYSCRIBER_ENABLE_PORTAL_SHORTCUTS";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PortalShortcutDropInState {
    pub portal_shortcut_present: bool,
    pub portal_app_id_present: bool,
    pub explicit_portal_opt_in_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutRuntimeInputs {
    pub gnome_desktop: bool,
    pub portal_runtime_supported: bool,
    pub portal_dropin_state: PortalShortcutDropInState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutRuntimeBackend {
    GnomeCustomShortcut,
    PortalGlobalShortcuts,
    Manual,
}

pub const fn portal_runtime_supported() -> bool {
    cfg!(feature = "portal")
}

pub fn gnome_shortcut_schema_with_path() -> String {
    format!("{GNOME_CUSTOM_KEYBINDING_SCHEMA}:{GNOME_WAYSCRIBER_KEYBINDING_PATH}")
}

pub fn is_gnome_desktop(current: &str, session: &str) -> bool {
    let combined = format!("{current};{session}").to_lowercase();
    combined.contains("gnome")
}

pub fn resolve_shortcut_runtime_backend(inputs: ShortcutRuntimeInputs) -> ShortcutRuntimeBackend {
    if inputs.gnome_desktop {
        if inputs.portal_runtime_supported
            && inputs.portal_dropin_state.explicit_portal_opt_in_present
        {
            return ShortcutRuntimeBackend::PortalGlobalShortcuts;
        }
        return ShortcutRuntimeBackend::GnomeCustomShortcut;
    }
    if inputs.portal_runtime_supported {
        ShortcutRuntimeBackend::PortalGlobalShortcuts
    } else {
        ShortcutRuntimeBackend::Manual
    }
}

pub fn current_shortcut_runtime_backend() -> ShortcutRuntimeBackend {
    let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
    resolve_shortcut_runtime_backend(ShortcutRuntimeInputs {
        gnome_desktop: is_gnome_desktop(&current, &session),
        portal_runtime_supported: portal_runtime_supported(),
        portal_dropin_state: read_portal_shortcut_dropin_state(),
    })
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

pub fn read_portal_shortcut_dropin_state() -> PortalShortcutDropInState {
    portal_shortcut_dropin_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|content| parse_portal_shortcut_dropin_state(&content))
        .unwrap_or_default()
}

pub fn parse_portal_shortcut_dropin_state(content: &str) -> PortalShortcutDropInState {
    let mut state = PortalShortcutDropInState::default();
    for line in content.lines() {
        let Some((name, value)) = parse_systemd_environment_assignment(line) else {
            continue;
        };
        match name.as_str() {
            PORTAL_SHORTCUT_ENV => {
                state.portal_shortcut_present = normalize_shortcut_hint(Some(&value)).is_some();
            }
            PORTAL_APP_ID_ENV => {
                state.portal_app_id_present = normalize_shortcut_hint(Some(&value)).is_some();
            }
            PORTAL_SHORTCUT_OPT_IN_ENV => {
                state.explicit_portal_opt_in_present = value.trim() == "1";
            }
            _ => {}
        }
    }
    state
}

pub fn parse_portal_shortcut_from_dropin(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let (name, value) = parse_systemd_environment_assignment(line)?;
        if name != PORTAL_SHORTCUT_ENV {
            return None;
        }
        normalize_shortcut_hint(Some(&value))
    })
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

fn parse_systemd_environment_assignment(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let raw_assignment = trimmed.strip_prefix("Environment=")?;
    let assignment = if let Some(quoted_assignment) = raw_assignment
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        unescape_systemd_env_assignment(quoted_assignment)
    } else {
        raw_assignment.to_string()
    };
    let (name, value) = assignment.split_once('=')?;
    Some((name.trim().to_string(), value.trim().to_string()))
}

fn unescape_systemd_env_assignment(value: &str) -> String {
    let mut unescaped = String::with_capacity(value.len());
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            unescaped.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        unescaped.push(ch);
    }
    if escaped {
        unescaped.push('\\');
    }
    unescaped
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

    #[test]
    fn parse_portal_shortcut_dropin_state_handles_legacy_gnome_dropin() {
        let content = "[Service]\nEnvironment=\"WAYSCRIBER_PORTAL_SHORTCUT=<Super>g\"\nEnvironment=\"WAYSCRIBER_PORTAL_APP_ID=com.devmobasa.wayscriber\"\n";
        assert_eq!(
            parse_portal_shortcut_dropin_state(content),
            PortalShortcutDropInState {
                portal_shortcut_present: true,
                portal_app_id_present: true,
                explicit_portal_opt_in_present: false,
            }
        );
        assert_eq!(
            resolve_shortcut_runtime_backend(ShortcutRuntimeInputs {
                gnome_desktop: true,
                portal_runtime_supported: true,
                portal_dropin_state: parse_portal_shortcut_dropin_state(content),
            }),
            ShortcutRuntimeBackend::GnomeCustomShortcut
        );
    }

    #[test]
    fn parse_portal_shortcut_dropin_state_handles_explicit_opt_in() {
        let content = "[Service]\nEnvironment=\"WAYSCRIBER_ENABLE_PORTAL_SHORTCUTS=1\"\nEnvironment=\"WAYSCRIBER_PORTAL_SHORTCUT=<Ctrl><Shift>g\"\nEnvironment=\"WAYSCRIBER_PORTAL_APP_ID=wayscriber\"\n";
        assert_eq!(
            parse_portal_shortcut_dropin_state(content),
            PortalShortcutDropInState {
                portal_shortcut_present: true,
                portal_app_id_present: true,
                explicit_portal_opt_in_present: true,
            }
        );
        assert_eq!(
            parse_portal_shortcut_from_dropin(content),
            Some("<Ctrl><Shift>g".to_string())
        );
    }

    #[test]
    fn resolve_shortcut_runtime_backend_preserves_non_gnome_portal_default() {
        assert_eq!(
            resolve_shortcut_runtime_backend(ShortcutRuntimeInputs {
                gnome_desktop: false,
                portal_runtime_supported: true,
                portal_dropin_state: PortalShortcutDropInState::default(),
            }),
            ShortcutRuntimeBackend::PortalGlobalShortcuts
        );
        assert_eq!(
            resolve_shortcut_runtime_backend(ShortcutRuntimeInputs {
                gnome_desktop: false,
                portal_runtime_supported: false,
                portal_dropin_state: PortalShortcutDropInState::default(),
            }),
            ShortcutRuntimeBackend::Manual
        );
    }
}
