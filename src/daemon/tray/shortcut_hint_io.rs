#[cfg(feature = "tray")]
use std::env;
#[cfg(feature = "tray")]
use std::process::Command;
#[cfg(feature = "tray")]
use wayscriber::shortcut_hint::{
    GNOME_MEDIA_KEYS_KEY, GNOME_MEDIA_KEYS_SCHEMA, PORTAL_SHORTCUT_ENV, ShortcutRuntimeBackend,
    current_shortcut_runtime_backend, gnome_shortcut_schema_with_path, is_gnome_desktop,
    normalize_shortcut_hint, resolve_toggle_shortcut_hint,
};

#[cfg(feature = "tray")]
pub(super) fn configured_toggle_shortcut_hint() -> Option<String> {
    match current_shortcut_runtime_backend() {
        ShortcutRuntimeBackend::PortalGlobalShortcuts => {
            let portal_shortcut_env = env::var(PORTAL_SHORTCUT_ENV).ok();
            normalize_shortcut_hint(portal_shortcut_env.as_deref())
        }
        ShortcutRuntimeBackend::GnomeCustomShortcut => {
            if !current_desktop_is_gnome() {
                return None;
            }
            let (custom_keybindings_raw, binding_raw) = read_gnome_shortcut_outputs()?;
            resolve_toggle_shortcut_hint(
                None,
                true,
                Some(custom_keybindings_raw.as_str()),
                Some(binding_raw.as_str()),
            )
        }
        ShortcutRuntimeBackend::Manual => None,
    }
}

#[cfg(feature = "tray")]
fn current_desktop_is_gnome() -> bool {
    let current = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
    is_gnome_desktop(&current, &session)
}

#[cfg(feature = "tray")]
fn read_gnome_shortcut_outputs() -> Option<(String, String)> {
    let custom_keybindings_raw =
        read_gsettings_value(GNOME_MEDIA_KEYS_SCHEMA, GNOME_MEDIA_KEYS_KEY)?;
    let schema_with_path = gnome_shortcut_schema_with_path();
    let binding_raw = read_gsettings_value(&schema_with_path, "binding")?;
    Some((custom_keybindings_raw, binding_raw))
}

#[cfg(feature = "tray")]
fn read_gsettings_value(schema: &str, key: &str) -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}
