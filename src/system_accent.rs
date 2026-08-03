//! Startup resolution of the configured accent color (`[ui] accent_color`).
//!
//! `"system"` reads the desktop accent from the xdg-desktop-portal Settings
//! interface (`org.freedesktop.appearance` / `accent-color`) once at
//! startup, before constructing the owned runtime theme. There is no live
//! subscription: the daemon spawns a fresh overlay
//! process per activation, so a changed desktop accent lands on the next
//! toggle.

use log::debug;

use crate::config::{AccentColor, AccentColorMode};
use crate::ui::theme;

/// Resolves `[ui] accent_color` to the accent root the theme should carry,
/// or `None` for the built-in accent. The caller uses the result to construct
/// the root-owned [`theme::Theme`].
pub(crate) fn resolve_configured_accent(accent_color: &AccentColor) -> Option<theme::Rgb> {
    match accent_color.mode() {
        AccentColorMode::Default => None,
        AccentColorMode::Custom(color) => {
            // Accent tokens carry their own alphas, so only the RGB root is
            // taken from the configured color.
            Some((color.r, color.g, color.b))
        }
        AccentColorMode::System => match read_system_accent() {
            Some(root) => {
                log::info!(
                    "Using the system accent color from the settings portal: \
                     rgb({:.3}, {:.3}, {:.3})",
                    root.0,
                    root.1,
                    root.2
                );
                Some(root)
            }
            None => {
                debug!("No system accent color; keeping the built-in accent");
                None
            }
        },
    }
}

#[cfg(all(feature = "dbus", not(test)))]
fn read_system_accent() -> Option<theme::Rgb> {
    // Startup calls this from sync context before any runtime is entered;
    // this guard is defensive, since the private runtime below cannot be
    // entered from inside another one.
    if tokio::runtime::Handle::try_current().is_ok() {
        debug!("Skipping the system accent read inside an async context");
        return None;
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            debug!("Could not create the system-accent portal runtime: {err}");
            return None;
        }
    };
    runtime.block_on(async {
        match tokio::time::timeout(portal::READ_TIMEOUT, portal::read_accent()).await {
            Ok(accent) => accent,
            Err(_) => {
                debug!("System accent read timed out");
                None
            }
        }
    })
}

/// Unit tests must not depend on the host desktop's portal; the resolution and
/// decoding logic is tested purely instead.
#[cfg(all(feature = "dbus", test))]
fn read_system_accent() -> Option<theme::Rgb> {
    None
}

#[cfg(not(feature = "dbus"))]
fn read_system_accent() -> Option<theme::Rgb> {
    debug!("System accent color unavailable (built without D-Bus support)");
    None
}

// Test builds route `read_system_accent` to the stub above, leaving the
// portal plumbing unused there; keep compiling it so tests still type-check
// the D-Bus path.
#[cfg_attr(test, allow(dead_code))]
#[cfg(feature = "dbus")]
mod portal {
    //! Settings-portal read of `org.freedesktop.appearance` `accent-color`.

    use log::debug;
    use zbus::proxy;
    use zbus::zvariant::{OwnedValue, Value};

    use crate::ui::theme::Rgb;

    /// How long startup waits for the settings portal before falling back
    /// to the built-in accent (same order as the capture portal probe).
    pub(super) const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(400);

    const ACCENT_NAMESPACE: &str = "org.freedesktop.appearance";
    const ACCENT_KEY: &str = "accent-color";

    /// D-Bus proxy for the xdg-desktop-portal Settings interface.
    #[proxy(
        interface = "org.freedesktop.portal.Settings",
        default_service = "org.freedesktop.portal.Desktop",
        default_path = "/org/freedesktop/portal/desktop"
    )]
    trait Settings {
        /// Reads a single setting (Settings interface version 2+).
        #[zbus(name = "ReadOne")]
        async fn read_one(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;

        /// Version 1 read; wraps the value in one more variant layer.
        #[zbus(name = "Read")]
        async fn read(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;
    }

    pub(super) async fn read_accent() -> Option<Rgb> {
        let connection = match zbus::Connection::session().await {
            Ok(connection) => connection,
            Err(err) => {
                debug!("No session bus for the system accent read: {err}");
                return None;
            }
        };
        let settings = match SettingsProxy::new(&connection).await {
            Ok(settings) => settings,
            Err(err) => {
                debug!("Could not create the system accent settings proxy: {err}");
                return None;
            }
        };
        let value = match settings.read_one(ACCENT_NAMESPACE, ACCENT_KEY).await {
            Ok(value) => value,
            Err(err) => {
                debug!("Settings.ReadOne failed ({err}); trying the v1 Read");
                match settings.read(ACCENT_NAMESPACE, ACCENT_KEY).await {
                    Ok(value) => value,
                    Err(err) => {
                        debug!("Settings.Read failed: {err}");
                        return None;
                    }
                }
            }
        };
        accent_from_value(&value)
    }

    /// Decodes the portal's `(ddd)` accent structure, unwrapping nested
    /// variant layers (`Read` double-wraps, `ReadOne` does not).
    /// Out-of-range or non-finite channels mean "no preference" per the
    /// portal spec, not a clamped color.
    fn accent_from_value(value: &Value<'_>) -> Option<Rgb> {
        match value {
            Value::Value(inner) => accent_from_value(inner),
            Value::Structure(structure) => {
                let fields = structure.fields();
                if fields.len() != 3 {
                    return None;
                }
                let mut channels = [0.0f64; 3];
                for (slot, field) in channels.iter_mut().zip(fields) {
                    match field {
                        Value::F64(channel)
                            if channel.is_finite() && (0.0..=1.0).contains(channel) =>
                        {
                            *slot = *channel;
                        }
                        _ => return None,
                    }
                }
                Some((channels[0], channels[1], channels[2]))
            }
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use zbus::zvariant::Structure;

        fn accent_structure(r: f64, g: f64, b: f64) -> Value<'static> {
            Value::Structure(Structure::from((r, g, b)))
        }

        #[test]
        fn accent_from_value_decodes_a_ddd_structure() {
            let value = accent_structure(0.2, 0.5, 0.9);
            assert_eq!(accent_from_value(&value), Some((0.2, 0.5, 0.9)));
        }

        #[test]
        fn accent_from_value_unwraps_nested_variant_layers() {
            // The v1 `Read` answer: a variant inside the reply variant.
            let value = Value::Value(Box::new(Value::Value(Box::new(accent_structure(
                1.0, 0.0, 0.5,
            )))));
            assert_eq!(accent_from_value(&value), Some((1.0, 0.0, 0.5)));
        }

        #[test]
        fn accent_from_value_rejects_no_preference_sentinels() {
            // Out-of-range channels are the spec's "no preference" signal.
            for bad in [
                accent_structure(-1.0, 0.0, 0.0),
                accent_structure(0.0, 1.5, 0.0),
                accent_structure(0.0, 0.0, f64::NAN),
            ] {
                assert_eq!(accent_from_value(&bad), None);
            }
        }

        #[test]
        fn accent_from_value_rejects_other_shapes() {
            assert_eq!(accent_from_value(&Value::F64(0.5)), None);
            let two_fields = Value::Structure(Structure::from((0.5f64, 0.5f64)));
            assert_eq!(accent_from_value(&two_fields), None);
            let wrong_type = Value::Structure(Structure::from((1u32, 2u32, 3u32)));
            assert_eq!(accent_from_value(&wrong_type), None);
        }
    }
}
