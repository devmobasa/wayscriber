#[cfg(feature = "tray")]
use crate::env_vars::{XDG_CURRENT_DESKTOP_ENV, XDG_SESSION_DESKTOP_ENV};
#[cfg(feature = "tray")]
use crate::shortcut_hint::{
    GNOME_MEDIA_KEYS_KEY, GNOME_MEDIA_KEYS_SCHEMA, PORTAL_SHORTCUT_ENV, ShortcutRuntimeBackend,
    current_shortcut_runtime_backend, gnome_shortcut_schema_with_path, is_gnome_desktop,
    normalize_shortcut_hint, resolve_toggle_shortcut_hint,
};
#[cfg(feature = "tray")]
use std::env;
#[cfg(feature = "tray")]
use std::ffi::OsStr;
#[cfg(feature = "tray")]
use std::time::Duration;

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
    let current = env::var(XDG_CURRENT_DESKTOP_ENV).unwrap_or_default();
    let session = env::var(XDG_SESSION_DESKTOP_ENV).unwrap_or_default();
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
    let arguments = gsettings_get_arguments(schema, key);
    let output = match crate::process_broker::current().and_then(|broker| {
        broker.run(
            crate::process_broker::HelperKind::Gsettings,
            OsStr::new("gsettings"),
            arguments,
            Vec::new(),
            Duration::from_secs(3),
            64 * 1024,
        )
    }) {
        Ok(output) => output,
        Err(err) => {
            log::warn!(
                "Failed to query the GNOME shortcut hint through the process broker: {err:#}"
            );
            return None;
        }
    };
    if output.timed_out {
        log::warn!("Timed out while querying the GNOME shortcut hint with gsettings");
        return None;
    }
    if output.status != 0 {
        log::warn!(
            "gsettings could not read the GNOME shortcut hint (status {})",
            output.status
        );
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(feature = "tray")]
fn gsettings_get_arguments<'a>(schema: &'a str, key: &'a str) -> [&'a OsStr; 3] {
    [OsStr::new("get"), OsStr::new(schema), OsStr::new(key)]
}

#[cfg(all(test, feature = "tray"))]
mod tests {
    use super::*;

    #[test]
    fn gsettings_shortcut_query_has_an_explicit_read_only_argv() {
        assert_eq!(
            gsettings_get_arguments("org.example.settings", "binding"),
            [
                OsStr::new("get"),
                OsStr::new("org.example.settings"),
                OsStr::new("binding"),
            ]
        );
    }
}
